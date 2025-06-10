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
use wac_types::{InterfaceId, ItemKind, Package};
use wasmtime::component::{Component, Instance, LinkerInstance};
use wasmtime::{AsContextMut, component};

/// A graph for composing multiple WebAssembly components into a single linker, while allowing for
/// automatic insertion of "trampoline" functions between cross-component calls.
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
    /// Creates a new empty `CompositionGraph`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a package (component) to the composition graph.
    ///
    /// Components can be added in any order, and dependencies will be resolved at instantiation time.
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

        for (export_name, export_kind) in exports {
            let ItemKind::Instance(interface_id) = export_kind else {
                continue;
            };

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
                    interface: *interface_id,
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

    /// Instantiates a component from the composition graph, resolving all component dependencies.
    ///
    /// Host functions and other resources can be provided through the `linker` argument prior to
    /// instantiation.
    pub fn instantiate(
        &mut self,
        package_id: PackageId,
        linker: &mut component::Linker<D>,
        mut store: impl AsContextMut<Data = D>,
        engine: &wasmtime::Engine,
    ) -> Result<Instance, InstantiateError>
    where
        D: 'static,
        C: Send + Sync + 'static,
    {
        let mut interfaces = IndexMap::<PackageId, IndexSet<String>>::new();

        let load_order = self
            .package_load_order(package_id, &mut interfaces)
            .context(instantiate_error::LoadPackageSnafu)?;

        let package = self
            .packages
            .get(package_id.id)
            .ok_or(InstantiateError::PackageNotFound { id: package_id })?;

        let component = Component::new(engine, package.bytes())
            .context(instantiate_error::ComponentInstantiationSnafu)?;

        for shadow_package_id in load_order.into_iter() {
            if shadow_package_id == package_id {
                break;
            }

            let shadow_package = self.packages.get(shadow_package_id.id).ok_or(
                InstantiateError::PackageNotFound {
                    id: shadow_package_id,
                },
            )?;

            let empty_set = IndexSet::new();
            let shadow_interfaces = interfaces.get(&shadow_package_id).unwrap_or(&empty_set);

            self.instantiate_shadowed_package(
                shadow_package,
                linker,
                &mut store,
                engine,
                shadow_interfaces,
            )
            .with_context(|_err| {
                instantiate_error::InstantiatePackageDependencySnafu {
                    name: shadow_package.name().to_string(),
                    version: shadow_package.version().cloned(),
                }
            })?;
        }

        let instance = linker
            .instantiate(&mut store, &component)
            .context(instantiate_error::ComponentInstantiationSnafu)?;

        Ok(instance)
    }

    /// Like `instantiate`, but for asynchronous contexts.
    pub async fn instantiate_async(
        &mut self,
        package_id: PackageId,
        linker: &mut component::Linker<D>,
        mut store: impl AsContextMut<Data = D>,
        engine: &wasmtime::Engine,
    ) -> Result<Instance, InstantiateError>
    where
        D: Send + 'static,
        C: Send + Sync + 'static,
    {
        let mut interfaces = IndexMap::<PackageId, IndexSet<String>>::new();

        let load_order = self
            .package_load_order(package_id, &mut interfaces)
            .context(instantiate_error::LoadPackageSnafu)?;

        let package = self
            .packages
            .get(package_id.id)
            .ok_or(InstantiateError::PackageNotFound { id: package_id })?;

        let component = Component::new(engine, package.bytes())
            .context(instantiate_error::ComponentInstantiationSnafu)?;

        for shadow_package_id in load_order {
            if shadow_package_id == package_id {
                break;
            }

            let shadow_package = self.packages.get(shadow_package_id.id).ok_or(
                InstantiateError::PackageNotFound {
                    id: shadow_package_id,
                },
            )?;

            let empty_set = IndexSet::new();
            let shadow_interfaces = interfaces.get(&shadow_package_id).unwrap_or(&empty_set);

            self.instantiate_shadowed_package_async(
                shadow_package,
                linker,
                &mut store,
                engine,
                &shadow_interfaces,
            )
            .await
            .with_context(|_err| {
                instantiate_error::InstantiatePackageDependencySnafu {
                    name: shadow_package.name().to_string(),
                    version: shadow_package.version().cloned(),
                }
            })?;
        }

        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .context(instantiate_error::ComponentInstantiationSnafu)?;

        Ok(instance)
    }

    fn package_load_order(
        &self,
        origin: PackageId,
        interfaces: &mut IndexMap<PackageId, IndexSet<String>>,
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

                return Err(LoadPackageError::PackageCycle {
                    cycle: cycle
                        .into_iter()
                        .map(|package| {
                            self.packages
                                .get(package.id)
                                .map(|package| package.name().to_string())
                                .unwrap_or("{{UNKNOWN_PACKAGE}}".to_string())
                        })
                        .collect(),
                });
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
                    LoadPackageError::MissingPackageDependency {
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
                    .insert(import.interface_name().to_string());
            }
        }

        Ok(load_order.into_iter().chain(load_stack.into_iter().rev()))
    }

    fn instantiate_shadowed_package(
        &self,
        package: &Package,
        linker: &mut component::Linker<D>,
        mut store: impl AsContextMut<Data = D>,
        engine: &wasmtime::Engine,
        interfaces: &IndexSet<String>,
    ) -> Result<(), InstantiatePackageError>
    where
        D: 'static,
        C: Send + Sync + 'static,
    {
        let component = Component::new(engine, package.bytes())
            .context(instantiate_package_error::ComponentInstantiationSnafu)?;

        let shadow_instance = linker
            .instantiate(&mut store, &component)
            .context(instantiate_package_error::ComponentInstantiationSnafu)?;

        self.shadow_package(
            package,
            Rc::new(shadow_instance),
            linker,
            store,
            interfaces,
            SyncInstanceShadower,
        )
    }

    async fn instantiate_shadowed_package_async(
        &self,
        package: &Package,
        linker: &mut component::Linker<D>,
        mut store: impl AsContextMut<Data = D>,
        engine: &wasmtime::Engine,
        interfaces: &IndexSet<String>,
    ) -> Result<(), InstantiatePackageError>
    where
        D: Send + 'static,
        C: Send + Sync + 'static,
    {
        let component = Component::new(engine, package.bytes())
            .context(instantiate_package_error::ComponentInstantiationSnafu)?;

        let shadow_instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .context(instantiate_package_error::ComponentInstantiationSnafu)?;

        self.shadow_package(
            package,
            Rc::new(shadow_instance),
            linker,
            store,
            interfaces,
            AsyncInstanceShadower,
        )
    }

    fn shadow_package(
        &self,
        package: &Package,
        shadow_instance: Rc<Instance>,
        linker: &mut component::Linker<D>,
        mut store: impl AsContextMut<Data = D>,
        interfaces: &IndexSet<String>,
        shadower: impl InstanceShadower<D, C>,
    ) -> Result<(), InstantiatePackageError> {
        for interface_name in interfaces {
            let interface_path = ForeignInterfacePath::new(
                package.name().to_string(),
                interface_name.to_string(),
                package.version().cloned(),
            );

            let interface_full_name = interface_path.to_string();

            let (_, shadow_interface_export_id) = shadow_instance
                .get_export(&mut store, None, &interface_full_name)
                .ok_or_else(|| InstantiatePackageError::InstanceMissingInterfaceExport {
                    interface_name: interface_full_name.to_string(),
                })?;

            let interface_export =
                self.exported_interfaces
                    .get(&interface_path)
                    .ok_or_else(|| InstantiatePackageError::MissingInterfaceExport {
                        path: interface_path.clone(),
                    })?;

            let mut front_instance = linker
                .instance(interface_full_name.as_str())
                .context(instantiate_package_error::LinkerInstanceSnafu)?;

            let interface = &self.types[interface_export.interface];

            for (export_name, export_kind) in &interface.exports {
                let ItemKind::Func(func_id) = export_kind else {
                    continue;
                };

                let (_, shadow_func_export_id) = shadow_instance
                    .get_export(&mut store, Some(&shadow_interface_export_id), export_name)
                    .ok_or_else(
                        || InstantiatePackageError::InstanceMissingInterfaceFuncExport {
                            interface_name: interface_full_name.to_string(),
                            func_name: export_name.to_string(),
                        },
                    )?;

                let shadow_func = shadow_instance
                    .get_func(&mut store, &shadow_func_export_id)
                    .ok_or_else(|| InstantiatePackageError::ComponentFuncRetrievalError {
                        interface_name: interface_full_name.to_string(),
                        func_name: export_name.to_string(),
                    })?;

                shadower.shadow_func(
                    &mut front_instance,
                    export_name,
                    shadow_func,
                    interface_path.clone(),
                    self.types[*func_id].clone(),
                    &interface_export.trampoline,
                )?;
            }
        }

        Ok(())
    }
}

trait InstanceShadower<D, C: Clone> {
    fn shadow_func(
        &self,
        instance: &mut LinkerInstance<D>,
        export_name: &str,
        shadow_func: component::Func,
        interface_path: ForeignInterfacePath,
        func_ty: wac_types::FuncType,
        trampoline: &DynInterfaceTrampoline<D, C>,
    ) -> Result<(), InstantiatePackageError>;
}

#[derive(Copy, Clone, Default, Debug)]
struct SyncInstanceShadower;

impl<D: 'static, C: Clone + Send + Sync + 'static> InstanceShadower<D, C> for SyncInstanceShadower {
    fn shadow_func(
        &self,
        instance: &mut LinkerInstance<D>,
        export_name: &str,
        shadow_func: component::Func,
        interface_path: ForeignInterfacePath,
        func_ty: wac_types::FuncType,
        trampoline: &DynInterfaceTrampoline<D, C>,
    ) -> Result<(), InstantiatePackageError> {
        let fn_export_name = Arc::new(export_name.to_string());
        let fn_interface_path = Arc::new(interface_path);
        let fn_ty = Arc::new(func_ty);

        match &trampoline {
            DynInterfaceTrampoline::Sync(trampoline) => {
                let fn_trampoline = trampoline.clone();

                instance
                    .func_new(export_name, move |store, arguments, result| {
                        let mut result = fn_trampoline.bounce(
                            &shadow_func,
                            store,
                            fn_interface_path.as_ref(),
                            fn_export_name.as_str(),
                            fn_ty.as_ref(),
                            arguments,
                            result,
                        )?;

                        result.post_return()?;

                        Ok(())
                    })
                    .context(instantiate_package_error::LinkFuncInstantiationSnafu)
            }

            DynInterfaceTrampoline::Async(_trampoline) => {
                Err(InstantiatePackageError::InvalidTrampolineSynchronicity)
            }
        }
    }
}

#[derive(Copy, Clone, Default, Debug)]
struct AsyncInstanceShadower;

impl<D: Send + 'static, C: Clone + Send + Sync + 'static> InstanceShadower<D, C>
    for AsyncInstanceShadower
{
    fn shadow_func(
        &self,
        instance: &mut LinkerInstance<D>,
        export_name: &str,
        shadow_func: component::Func,
        interface_path: ForeignInterfacePath,
        func_ty: wac_types::FuncType,
        trampoline: &DynInterfaceTrampoline<D, C>,
    ) -> Result<(), InstantiatePackageError> {
        let fn_export_name = Arc::new(export_name.to_string());
        let fn_interface_path = Arc::new(interface_path);
        let fn_ty = Arc::new(func_ty);

        match &trampoline {
            DynInterfaceTrampoline::Sync(trampoline) => {
                let fn_trampoline = trampoline.clone();

                instance
                    .func_new(export_name, move |store, arguments, result| {
                        let mut result = fn_trampoline.bounce(
                            &shadow_func,
                            store,
                            fn_interface_path.as_ref(),
                            fn_export_name.as_str(),
                            fn_ty.as_ref(),
                            arguments,
                            result,
                        )?;

                        result.post_return()?;

                        Ok(())
                    })
                    .context(instantiate_package_error::LinkFuncInstantiationSnafu)
            }

            DynInterfaceTrampoline::Async(trampoline) => {
                let fn_trampoline = trampoline.clone();

                instance
                    .func_new_async(export_name, move |store, arguments, result| {
                        let export_name = fn_export_name.clone();
                        let trampoline = fn_trampoline.clone();
                        let interface_path = fn_interface_path.clone();
                        let ty = fn_ty.clone();

                        Box::new(async move {
                            let mut result = trampoline
                                .bounce_async(
                                    &shadow_func,
                                    store,
                                    interface_path.as_ref(),
                                    export_name.as_str(),
                                    ty.as_ref(),
                                    arguments,
                                    result,
                                )
                                .await?;

                            result.post_return_async().await?;

                            Ok(())
                        })
                    })
                    .context(instantiate_package_error::LinkFuncInstantiationSnafu)
            }
        }
    }
}

/// Represents a unique identifier for a package within the composition graph.
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct PackageId {
    id: usize,
}

#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
struct InterfaceExport<D, C: Clone> {
    package: PackageId,
    interface: InterfaceId,

    #[derivative(Debug = "ignore")]
    trampoline: DynInterfaceTrampoline<D, C>,
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum AddPackageError {
    #[snafu(display("Duplicate package: {}@{:?}", name, version))]
    DuplicatePackage { name: String, version: Version },

    #[snafu(display("Failed to parse package: {}", source))]
    PackageParseError { source: anyhow::Error },

    #[snafu(display("Failed to parse import '{}': {}", interface, source))]
    ImportParseError {
        interface: String,
        source: InterfacePathParseError,
    },
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum InstantiateError {
    #[snafu(display("Package id '{:?}' not found", id))]
    PackageNotFound { id: PackageId },

    #[snafu(display("Failed to load package: {}", source))]
    LoadPackageError { source: LoadPackageError },

    #[snafu(display(
        "Failed to instantiate package dependency '{}@{:?}': {}",
        name,
        version,
        source
    ))]
    InstantiatePackageDependencyError {
        name: String,
        version: Option<Version>,
        source: InstantiatePackageError,
    },

    #[snafu(display("Failed to instantiate wasm component: {}", source))]
    ComponentInstantiationError { source: anyhow::Error },
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum LoadPackageError {
    #[snafu(display("Package import cycle detected: {:?}", cycle))]
    PackageCycle { cycle: Vec<String> },

    #[snafu(display("Package dependency {} not found", package_name))]
    MissingPackageDependency { package_name: String },

    #[snafu(display("Cannot resolve package version for {}@{:?}", name, version))]
    CannotResolvePackageVersion {
        name: String,
        version: Option<Version>,
    },
}

#[derive(Snafu, Debug)]
#[snafu(module)]
pub enum InstantiatePackageError {
    #[snafu(display("Failed to instantiate wasm component: {}", source))]
    ComponentInstantiationError { source: anyhow::Error },

    #[snafu(display("Failed to create linker instance: {}", source))]
    LinkerInstanceError { source: anyhow::Error },

    #[snafu(display("Instance is missing interface export with name '{}'", interface_name))]
    InstanceMissingInterfaceExport { interface_name: String },

    #[snafu(display(
        "Instance is missing interface func export with name '{}/{}'",
        interface_name,
        func_name
    ))]
    InstanceMissingInterfaceFuncExport {
        interface_name: String,
        func_name: String,
    },

    #[snafu(display(
        "Failed to retrieve component function '{}/{}'",
        interface_name,
        func_name
    ))]
    ComponentFuncRetrievalError {
        interface_name: String,
        func_name: String,
    },

    #[snafu(display("Failed to instantiate function: {}", source))]
    LinkFuncInstantiationError { source: anyhow::Error },

    #[snafu(display("Invalid trampoline sync/async call match"))]
    InvalidTrampolineSynchronicity,

    #[snafu(display("Missing interface export {}", path))]
    MissingInterfaceExport { path: ForeignInterfacePath },
}
