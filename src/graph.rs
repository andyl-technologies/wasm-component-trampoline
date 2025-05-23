use crate::path::{ForeignInterfacePath, InterfacePath, InterfacePathParseError};
use crate::semver::VersionMap;
use crate::{DynInterfaceTrampoline, DynPackageTrampoline};
use derivative::Derivative;
use indexmap::{IndexMap, IndexSet};
use semver::Version;
use slab::Slab;
use snafu::{ResultExt, Snafu};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use wac_types::{ItemKind, Package};
use wasmtime::component::Component;
use wasmtime::{AsContextMut, component};

#[derive(Derivative, Debug)]
#[derivative(Default(bound = ""))]
pub struct CompositionGraph<D, C: Clone = ()> {
    types: wac_types::Types,
    packages: Slab<Package>,
    package_map: HashMap<String, VersionMap<PackageId>>,
    exported_interfaces: HashMap<ForeignInterfacePath, InterfaceExport<D, C>>,
    imported_interfaces: HashMap<PackageId, Vec<ForeignInterfacePath>>,
}

impl<D, C: Clone> CompositionGraph<D, C> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_package(
        &mut self,
        name: String,
        version: Version,
        bytes: impl Into<Vec<u8>>,
        trampoline: impl DynPackageTrampoline<D, C>,
    ) -> Result<PackageId, AddPackageError> {
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
        let version_suffix = package.version().map_or(String::new(), |v| format!("@{v}"));

        let exports = &self.types[package.ty()].exports;

        for (export_name, _export_kind) in exports {
            let interface_name = export_name
                .strip_prefix(&package_prefix)
                .and_then(|export_name| export_name.strip_suffix(&version_suffix));

            if let Some(interface_name) = interface_name {
                let path = ForeignInterfacePath::new(
                    package.name().to_string(),
                    interface_name.to_string(),
                    package.version().cloned(),
                );

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
                    panic!("duplicate exported interface key {path:?}");
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

    pub async fn instantiate(
        &mut self,
        package: PackageId,
        linker: &mut component::Linker<D>,
        mut store: impl AsContextMut<Data = D>,
        engine: &wasmtime::Engine,
    ) -> Result<(), InstantiateError>
    where
        D: Send + 'static,
        C: Send + Sync + 'static,
    {
        let mut interfaces = IndexMap::<PackageId, Vec<String>>::new();

        let load_order = self
            .package_load_order(package, &mut interfaces)
            .context(instantiate_error::LoadPackageSnafu)?;

        for package in load_order {
            self.instantiate_package(
                package,
                linker,
                &mut store,
                engine,
                interfaces
                    .get(&package)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
            )
            .await
            .context(instantiate_error::InstantiatePackageSnafu { package })?;
        }

        Ok(())
    }

    fn package_load_order(
        &self,
        origin: PackageId,
        interfaces: &mut IndexMap<PackageId, Vec<String>>,
    ) -> Result<impl IntoIterator<Item = PackageId> + 'static, LoadPackageError> {
        let mut package_stack = vec![(origin, 0)];

        let mut load_order = IndexSet::<PackageId>::new();
        let mut load_stack = IndexSet::<PackageId>::new();

        while let Some((package_id, offset)) = package_stack.pop() {
            load_order.extend(load_stack.drain(offset..).rev());

            if let Some(cycle_start) = load_stack.get_index_of(&package_id) {
                let mut cycle = load_stack
                    .iter()
                    .skip(cycle_start)
                    .copied()
                    .collect::<Vec<_>>();

                cycle.push(package_id);

                return Err(LoadPackageError::PackageCycle { cycle });
            }

            if load_order.contains(&package_id) {
                continue;
            }

            load_stack.insert(package_id);

            let imports = self
                .imported_interfaces
                .get(&package_id)
                .map(Vec::as_slice)
                .unwrap_or_default();

            for import in imports {
                let version_map = self.package_map.get(import.package_name()).ok_or_else(|| {
                    LoadPackageError::MissingPackage {
                        package_name: import.package_name().to_string(),
                    }
                })?;

                let import_package =
                    version_map.get_or_latest(import.version()).ok_or_else(|| {
                        LoadPackageError::CannotResolvePackageVersion {
                            name: import.package_name().to_string(),
                            version: import.version().cloned(),
                        }
                    })?;

                package_stack.push((*import_package, load_stack.len()));

                interfaces
                    .entry(*import_package)
                    .or_default()
                    .push(import.interface_name().to_string());
            }
        }

        Ok(load_order.into_iter().chain(load_stack.into_iter().rev()))
    }

    async fn instantiate_package(
        &mut self,
        package: PackageId,
        linker: &mut component::Linker<D>,
        mut store: impl AsContextMut<Data = D>,
        engine: &wasmtime::Engine,
        interfaces: &[String],
    ) -> Result<(), InstantiatePackageError>
    where
        D: Send + 'static,
        C: Send + Sync + 'static,
    {
        let package = self
            .packages
            .get(package.id)
            .ok_or(InstantiatePackageError::PackageNotFound)?;

        let package_ty = &self.types[package.ty()];

        let component = Component::new(engine, package.bytes())
            .context(instantiate_package_error::ComponentSnafu)?;

        let shadow_instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .context(instantiate_package_error::ComponentInstantiationSnafu)?;

        let shadow_instance = Rc::new(shadow_instance);

        for interface_name in interfaces {
            let Some(ItemKind::Instance(interface_id)) = package_ty.exports.get(interface_name)
            else {
                return Err(InstantiatePackageError::MissingInterface {
                    package_name: package.name().to_string(),
                    package_version: package.version().cloned(),
                    interface_name: interface_name.to_string(),
                });
            };

            let interface_path = ForeignInterfacePath::new(
                package.name().to_string(),
                interface_name.to_string(),
                package.version().cloned(),
            );

            let interface_export =
                self.exported_interfaces
                    .get(&interface_path)
                    .ok_or_else(|| InstantiatePackageError::MissingInterface {
                        package_name: package.name().to_string(),
                        package_version: package.version().cloned(),
                        interface_name: interface_name.to_string(),
                    })?;

            let DynInterfaceTrampoline::Async(trampoline) = &interface_export.trampoline else {
                return Err(InstantiatePackageError::InvalidTrampolineSynchronicity);
            };

            let mut front_instance = linker
                .instance(format!("{}/{}", package.name(), interface_name).as_str())
                .context(instantiate_package_error::LinkerInstanceSnafu)?;

            let interface = &self.types[*interface_id];

            for (export_name, export_kind) in &interface.exports {
                let ItemKind::Func(func_id) = export_kind else {
                    continue;
                };

                let shadow_func = shadow_instance
                    .get_func(&mut store, export_name)
                    .ok_or_else(|| InstantiatePackageError::ComponentFuncRetrievalError {
                        func_name: export_name.to_string(),
                    })?;

                let fn_export_name = Arc::new(export_name.to_string());
                let fn_trampoline = trampoline.clone();
                let fn_interface_path = Arc::new(interface_path.clone());
                let fn_ty = Arc::new(self.types[*func_id].clone());

                front_instance
                    .func_new_async(export_name, move |store, arguments, result| {
                        let export_name = fn_export_name.clone();
                        let trampoline = fn_trampoline.clone();
                        let interface_path = fn_interface_path.clone();
                        let ty = fn_ty.clone();

                        Box::new(async move {
                            let _result = trampoline
                                .bounce_async(
                                    shadow_func,
                                    store,
                                    interface_path.as_ref(),
                                    export_name.as_str(),
                                    ty.as_ref(),
                                    arguments,
                                    result,
                                )
                                .await?;
                            Ok(())
                        })
                    })
                    .context(instantiate_package_error::LinkFuncInstantiationSnafu)?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct PackageId {
    id: usize,
}

#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
struct InterfaceExport<D, C: Clone> {
    package: PackageId,

    #[derivative(Debug = "ignore")]
    trampoline: DynInterfaceTrampoline<D, C>,
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
pub enum InstantiateError {
    LoadPackageError {
        source: LoadPackageError,
    },
    InstantiatePackageError {
        package: PackageId,
        source: InstantiatePackageError,
    },
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum LoadPackageError {
    PackageCycle {
        cycle: Vec<PackageId>,
    },
    #[snafu(display("Package {} not found", package_name))]
    MissingPackage {
        package_name: String,
    },
    CannotResolvePackageVersion {
        name: String,
        version: Option<Version>,
    },
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum InstantiatePackageError {
    PackageNotFound,
    ComponentError {
        source: anyhow::Error,
    },
    ComponentInstantiationError {
        source: anyhow::Error,
    },
    LinkerInstanceError {
        source: anyhow::Error,
    },
    ComponentFuncRetrievalError {
        func_name: String,
    },
    LinkFuncInstantiationError {
        source: anyhow::Error,
    },
    InvalidTrampolineSynchronicity,
    #[snafu(display(
        "Missing interface {}@{:?} in package {}",
        interface_name,
        package_version,
        package_name
    ))]
    MissingInterface {
        package_name: String,
        package_version: Option<Version>,
        interface_name: String,
    },
}
