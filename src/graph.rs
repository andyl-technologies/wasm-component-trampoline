use crate::semver::VersionMap;
use crate::{DynInterfaceTrampoline, DynPackageTrampoline};
use derivative::Derivative;
use indexmap::IndexSet;
use semver::Version;
use slab::Slab;
use snafu::{ResultExt, Snafu};
use std::collections::HashMap;
use std::str::FromStr;
use wac_types::{ItemKind, Package};
use wasmtime::{AsContextMut, component};

#[derive(Derivative, Debug)]
#[derivative(Default(bound = ""))]
pub struct CompositionGraph<D, C = ()> {
    types: wac_types::Types,
    packages: Slab<Package>,
    package_map: HashMap<String, VersionMap<PackageId>>,
    exported_interfaces: HashMap<ForeignInterfacePath, InterfaceExport<D, C>>,
    imported_interfaces: HashMap<PackageId, Vec<ForeignInterfacePath>>,
}

impl<D, C> CompositionGraph<D, C> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_package(
        &mut self,
        name: String,
        version: Version,
        bytes: impl Into<Vec<u8>>,
        trampoline: impl DynPackageTrampoline<D, C>,
    ) -> Result<PackageId, AddPackageError>
    where
        C: Clone,
    {
        let package = Package::from_bytes(name.as_str(), Some(&version), bytes, &mut self.types)
            .context(add_package_error::PackageParseSnafu)?;

        let package_id = PackageId {
            id: self.packages.insert(package),
        };

        let version_set = self.package_map.entry(name.to_string()).or_default();

        if let Err((version, _)) = version_set.try_insert(version, package_id) {
            return Err(AddPackageError::DuplicatePackage {
                name: name.to_string(),
                version: version.clone(),
            });
        }

        let package = self.packages.get_mut(package_id.id).unwrap();

        let package_prefix = format!("{}/", package.name());
        let version_suffix = package
            .version()
            .map(|v| format!("@{}", v))
            .unwrap_or("".to_string());

        let exports = &self.types[package.ty()].exports;

        for (export_name, _export_kind) in exports {
            let interface_name = export_name
                .strip_prefix(&package_prefix)
                .and_then(|export_name| export_name.strip_suffix(&version_suffix));

            if let Some(interface_name) = interface_name {
                let path = ForeignInterfacePath {
                    package_name: package.name().to_string(),
                    interface_name: interface_name.to_string(),
                    version: package.version().cloned(),
                };

                let interface_trampoline = InterfaceExport {
                    package: package_id,
                    trampoline: trampoline.interface_trampoline(interface_name),
                };

                if self
                    .exported_interfaces
                    .insert(path.clone(), interface_trampoline)
                    .is_some()
                {
                    // This would be a programming error, since the package name/version tuple is
                    // guaranteed to be unique.
                    panic!("duplicate exported interface key {:?}", path);
                }
            }
        }

        let mut import = |package_id: PackageId, import_name: &str| {
            let import_interface_path = InterfacePath::from_str(import_name).context(
                add_package_error::ImportParseSnafu {
                    interface: import_name.to_string(),
                },
            )?;

            if let Some(import) = import_interface_path.into_foreign() {
                self.imported_interfaces
                    .entry(package_id)
                    .or_default()
                    .push(import);
            }

            Ok(())
        };

        for (package_id, package) in &self.packages {
            let package_id = PackageId { id: package_id };
            let package_ty = &self.types[package.ty()];

            for (_use_name, use_type) in &package_ty.uses {
                let Some(import_name) = &self.types[use_type.interface].id else {
                    continue;
                };

                import(package_id, import_name)?;
            }

            for (import_name, import_kind) in &package_ty.imports {
                if !matches!(import_kind, ItemKind::Instance(_)) {
                    continue;
                }

                import(package_id, import_name)?;
            }
        }

        Ok(package_id)
    }

    pub fn instantiate_package(
        &mut self,
        package: PackageId,
        linker: &mut component::Linker<D>,
        store: impl AsContextMut<Data = D>,
    ) -> Result<(), InstantiatePackageError> {
        let mut package_stack = vec![package];

        let package = self.packages.get(package.id).ok_or_else(|| {
            InstantiatePackageError::PackageNotFound {
                package: PackageId { id: package.id },
            }
        })?;

        let mut load_order = IndexSet::new();

        while let Some(package_id) = package_stack.pop() {
            if load_order.contains(&package_id) {
                continue;
            }
            
            load_order.insert(package_id);
            
            let imports = self
                .imported_interfaces
                .get(&package_id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            for import in imports {
                let version_map = self.package_map.get(&import.package_name).ok_or_else(|| {
                    InstantiatePackageError::MissingPackageImport {
                        name: import.package_name.to_string(),
                    }
                })?;

                let import_package = version_map
                    .get_or_latest(import.version.as_ref())
                    .ok_or_else(|| InstantiatePackageError::CannotResolvePackageVersion {
                        name: import.package_name.to_string(),
                        version: import.version.clone(),
                    })?;

                package_stack.push(*import_package);
            }
        }

        todo!()
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ForeignInterfacePath {
    package_name: String,
    interface_name: String,
    version: Option<Version>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct InterfacePath {
    package_name: Option<String>,
    interface_name: String,
    version: Option<Version>,
}

impl InterfacePath {
    pub fn package_name(&self) -> Option<&str> {
        self.package_name.as_ref().map(|n| n.as_str())
    }

    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    pub fn version(&self) -> Option<&Version> {
        self.version.as_ref()
    }

    pub fn into_foreign(self) -> Option<ForeignInterfacePath> {
        Some(ForeignInterfacePath {
            package_name: self.package_name?,
            interface_name: self.interface_name,
            version: self.version,
        })
    }
}

impl FromStr for InterfacePath {
    type Err = InterfacePathParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parses the following format: "package_name/interface_name@version",
        // where the version specifier is optional.

        let parts: Vec<&str> = s.split('/').collect();

        match parts.len() {
            1 if s.contains('@') => return Err(InterfacePathParseError::FormatError),
            1 => {
                return Ok(Self {
                    package_name: None,
                    interface_name: s.to_string(),
                    version: None,
                });
            }
            2 => (), // Continue below.
            _ => return Err(InterfacePathParseError::FormatError),
        }

        let package_name = parts[0].to_string();

        let interface_parts: Vec<&str> = parts[1].split('@').collect();
        let interface_name = interface_parts[0].to_string();

        let version = if interface_parts.len() == 2 {
            Some(
                Version::parse(interface_parts[1])
                    .context(interface_path_parse_error::VersionParseSnafu)?,
            )
        } else {
            None
        };

        Ok(InterfacePath {
            package_name: Some(package_name),
            interface_name,
            version,
        })
    }
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum InterfacePathParseError {
    FormatError,
    VersionParseError { source: semver::Error },
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct PackageId {
    id: usize,
}

#[derive(Debug)]
struct InterfaceImport {
    interface: InterfacePath,
}

#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
struct InterfaceExport<D, C> {
    package: PackageId,

    #[derivative(Debug = "ignore")]
    trampoline: DynInterfaceTrampoline<D, C>,
}

#[derive(Debug)]
struct PackageLoadOperation {
    package: PackageId,
    dependencies: Vec<PackageId>,
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum AddPackageError {
    DuplicatePackage {
        name: String,
        version: Version,
    },

    PackageParseError {
        source: anyhow::Error,
    },

    ImportParseError {
        interface: String,
        source: InterfacePathParseError,
    },
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum InstantiatePackageError {
    PackageNotFound {
        package: PackageId,
    },
    MissingPackageImport {
        name: String,
    },
    CannotResolvePackageVersion {
        name: String,
        version: Option<Version>,
    },
}
