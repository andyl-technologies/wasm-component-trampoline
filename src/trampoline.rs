use crate::graph::InterfacePath;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::rc::Rc;
use wasmtime::StoreContextMut;
use wasmtime::component::{Func, Val};

pub trait Trampoline<D, C = ()>: Send + Sync + 'static {
    fn bounce<'c>(
        &self,
        call: GuestCall<'c, D, C>,
    ) -> Result<GuestResult<'c, D, C>, anyhow::Error> {
        call.call()
    }
}

fn _assert_trampoline_object_safe(_object: &dyn Trampoline<()>) {
    unreachable!("only used for compile time assertion");
}

pub trait AsyncTrampoline<D: Send, C: Send = ()>: Send + Sync + 'static {
    fn bounce_async<'c>(
        &self,
        call: AsyncGuestCall<'c, D, C>,
    ) -> Pin<Box<dyn Future<Output = Result<GuestResult<'c, D, C>, anyhow::Error>> + Send + 'c>>
    {
        Box::pin(async move { call.call_async().await })
    }
}

fn _assert_async_trampoline_object_safe(_object: &dyn AsyncTrampoline<()>) {
    unreachable!("only used for compile time assertion");
}

pub struct GuestCallData<'c, D, C> {
    store: StoreContextMut<'c, D>,
    context: &'c mut C,
    path: &'c InterfacePath,
    method: &'c str,
    arguments: &'c [Val],
    results: &'c mut [Val],
}

impl<'c, D, C> GuestCallData<'c, D, C> {
    pub fn store(&mut self) -> &mut StoreContextMut<'c, D> {
        &mut self.store
    }

    pub fn context(&mut self) -> &mut C {
        self.context
    }

    pub fn interface(&self) -> &InterfacePath {
        self.path
    }

    pub fn method(&self) -> &str {
        self.method
    }

    pub fn arguments(&self) -> &[Val] {
        self.arguments
    }
}

pub struct GuestCall<'c, D, C> {
    data: GuestCallData<'c, D, C>,
    function: Func,
}

impl<'c, D, C> GuestCall<'c, D, C> {
    pub fn call(self) -> Result<GuestResult<'c, D, C>, anyhow::Error> {
        let function = self.function;

        let mut store = self.data.store;
        let arguments = self.data.arguments;
        let results = self.data.results;

        function.call(&mut store, arguments, results)?;

        Ok(GuestResult {
            context: GuestCallData {
                store,
                context: self.data.context,
                path: self.data.path,
                method: self.data.method,
                arguments,
                results,
            },
        })
    }
}

impl<'c, D, C> Deref for GuestCall<'c, D, C> {
    type Target = GuestCallData<'c, D, C>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'c, D, C> DerefMut for GuestCall<'c, D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub struct AsyncGuestCall<'c, D: Send, C> {
    data: GuestCallData<'c, D, C>,
    function: Func,
}

impl<'c, D: Send, C> AsyncGuestCall<'c, D, C> {
    pub async fn call_async(self) -> Result<GuestResult<'c, D, C>, anyhow::Error> {
        let function = self.function;

        let mut store = self.data.store;
        let arguments = self.data.arguments;
        let results = self.data.results;

        function.call_async(&mut store, arguments, results).await?;

        Ok(GuestResult {
            context: GuestCallData {
                store,
                context: self.data.context,
                path: self.data.path,
                method: self.data.method,
                arguments,
                results,
            },
        })
    }
}

impl<'c, D: Send, C> Deref for AsyncGuestCall<'c, D, C> {
    type Target = GuestCallData<'c, D, C>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'c, D: Send, C> DerefMut for AsyncGuestCall<'c, D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub struct GuestResult<'c, D, C> {
    context: GuestCallData<'c, D, C>,
}

impl<'c, D, C> GuestResult<'c, D, C> {
    pub fn results(&self) -> &[Val] {
        self.context.results
    }

    pub fn results_mut(&mut self) -> &mut [Val] {
        self.context.results
    }
}

impl<'c, D, C> Deref for GuestResult<'c, D, C> {
    type Target = GuestCallData<'c, D, C>;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl<'c, D, C> DerefMut for GuestResult<'c, D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}

pub struct PackageTrampoline<T, C> {
    trampoline: T,
    interface_context_overrides: HashMap<String, C>,
    default_context: C,
}

impl<T, C> PackageTrampoline<T, C> {
    pub fn new(trampoline: T) -> Self
    where
        C: Default,
    {
        Self::with_default_context(trampoline, C::default())
    }

    pub fn with_default_context(trampoline: T, default_context: C) -> Self {
        Self {
            trampoline,
            interface_context_overrides: HashMap::new(),
            default_context,
        }
    }

    pub fn trampoline(&self) -> &T {
        &self.trampoline
    }

    pub fn default_context(&self) -> &C {
        &self.default_context
    }

    pub fn set_default_context(&mut self, context: C) {
        self.default_context = context;
    }

    pub fn get_interface_context(&self, interface_name: &str) -> Option<&C> {
        self.interface_context_overrides.get(interface_name)
    }

    pub fn set_interface_context(&mut self, interface_name: &str, context: C) {
        self.interface_context_overrides
            .insert(interface_name.to_string(), context);
    }

    pub fn remove_interface_context(&mut self, interface_name: &str) {
        self.interface_context_overrides.remove(interface_name);
    }

    pub fn interface_trampoline(&self, interface_name: &str) -> InterfaceTrampoline<T, C>
    where
        T: Clone,
        C: Clone,
    {
        let context = self
            .interface_context_overrides
            .get(interface_name)
            .unwrap_or(&self.default_context);

        InterfaceTrampoline {
            trampoline: self.trampoline.clone(),
            context: context.clone(),
        }
    }
}

#[derive(Clone)]
pub struct InterfaceTrampoline<T, C> {
    trampoline: T,
    context: C,
}

pub trait DynPackageTrampoline<D, C> {
    fn interface_trampoline(&self, interface_name: &str) -> DynInterfaceTrampoline<D, C>;
}

impl<D, C: Clone> DynPackageTrampoline<D, C> for PackageTrampoline<Rc<dyn Trampoline<D, C>>, C> {
    fn interface_trampoline(&self, interface_name: &str) -> DynInterfaceTrampoline<D, C> {
        DynInterfaceTrampoline::Sync(self.interface_trampoline(interface_name))
    }
}

impl<D, C: Clone> DynPackageTrampoline<D, C>
    for PackageTrampoline<Rc<dyn AsyncTrampoline<D, C>>, C>
{
    fn interface_trampoline(&self, interface_name: &str) -> DynInterfaceTrampoline<D, C> {
        DynInterfaceTrampoline::Async(self.interface_trampoline(interface_name))
    }
}

pub enum DynInterfaceTrampoline<D, C> {
    Sync(InterfaceTrampoline<Rc<dyn Trampoline<D, C>>, C>),
    Async(InterfaceTrampoline<Rc<dyn AsyncTrampoline<D, C>>, C>),
}
