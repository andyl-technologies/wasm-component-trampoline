use crate::path::ForeignInterfacePath;
use derivative::Derivative;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;
use wac_types::FuncType;
use wasmtime::component::{Func, Val};
use wasmtime::{AsContext, AsContextMut, StoreContext, StoreContextMut};

/// A trampoline is a mechanism to intercept WASM component function calls when switching
/// component contexts.
///
/// It allows for custom logic to be securely executed before and after the actual function call
/// on the host side.
pub trait Trampoline<D, C = ()>: Send + Sync + 'static {
    fn bounce<'c>(
        &self,
        call: GuestCall<'c, D, C>,
    ) -> Result<GuestResult<'c, D, C>, anyhow::Error> {
        call.call()
    }
}

impl<D: 'static, C: 'static> Trampoline<D, C> for Arc<dyn Trampoline<D, C>> {
    fn bounce<'c>(
        &self,
        call: GuestCall<'c, D, C>,
    ) -> Result<GuestResult<'c, D, C>, anyhow::Error> {
        self.deref().bounce(call)
    }
}

fn _assert_trampoline_object_safe(_object: &dyn Trampoline<()>) {
    unreachable!("only used for compile time assertion");
}

/// Like `Trampoline`, but for asynchronous WASM function calls.
pub trait AsyncTrampoline<D: Send, C: Send + Sync = ()>: Send + Sync + 'static {
    fn bounce_async<'c>(
        &'c self,
        call: AsyncGuestCall<'c, D, C>,
    ) -> Pin<Box<dyn Future<Output = Result<AsyncGuestResult<'c, D, C>, anyhow::Error>> + Send + 'c>>
    {
        Box::pin(async move { call.call_async().await })
    }
}

impl<D: Send + 'static, C: Send + Sync + 'static> AsyncTrampoline<D, C>
    for Arc<dyn AsyncTrampoline<D, C>>
{
    fn bounce_async<'c>(
        &'c self,
        call: AsyncGuestCall<'c, D, C>,
    ) -> Pin<Box<dyn Future<Output = Result<AsyncGuestResult<'c, D, C>, anyhow::Error>> + Send + 'c>>
    {
        Box::pin(async move { self.deref().bounce_async(call).await })
    }
}

fn _assert_async_trampoline_object_safe(_object: &dyn AsyncTrampoline<()>) {
    unreachable!("only used for compile time assertion");
}

/// Data structure that holds the common context for a guest call to a WASM component function.
pub struct GuestCallData<'c, D: 'static, C> {
    store: StoreContextMut<'c, D>,
    function: &'c Func,
    context: &'c C,
    path: &'c ForeignInterfacePath,
    method: &'c str,
    ty: &'c FuncType,
    arguments: &'c [Val],
    results: &'c mut [Val],
}

impl<'c, D: 'static, C> GuestCallData<'c, D, C> {
    /// Returns the WASM runtime store context.
    pub fn store(&self) -> StoreContext<'_, D> {
        self.store.as_context()
    }

    /// Returns a mutable reference to the WASM runtime store context.
    pub fn store_mut(&mut self) -> StoreContextMut<'_, D> {
        self.store.as_context_mut()
    }

    /// Returns the custom trampoline-specific context.
    pub fn context(&mut self) -> &C {
        self.context
    }

    /// Returns the fully-qualified WIT foreign interface path of the function being called.
    #[must_use]
    pub fn interface(&self) -> &ForeignInterfacePath {
        self.path
    }

    /// Returns the method name of the function being called.
    #[must_use]
    pub fn method(&self) -> &str {
        self.method
    }

    /// Returns the type signature of the function being called.
    #[must_use]
    pub fn func_type(&self) -> &FuncType {
        self.ty
    }

    /// Provides an immutable reference to the input arguments of the function call.
    #[must_use]
    pub fn arguments(&self) -> &[Val] {
        self.arguments
    }
}

/// A guest call to a WASM component function, which must be executed synchronously.
///
/// It's expected that the `call` method will be called to execute the function call in all cases,
/// unless an error occurs during the setup of the call.
pub struct GuestCall<'c, D: 'static, C> {
    data: GuestCallData<'c, D, C>,
}

impl<'c, D: 'static, C> GuestCall<'c, D, C> {
    /// Calls the underlying WASM component function with the provided arguments and results.
    ///
    /// Returns an error if the function call fails, or a `GuestResult` containing the results of
    /// the call.
    pub fn call(mut self) -> Result<GuestResult<'c, D, C>, anyhow::Error> {
        self.function
            .call(&mut self.data.store, self.data.arguments, self.data.results)?;

        Ok(GuestResult { context: self.data })
    }
}

impl<'c, D, C> Deref for GuestCall<'c, D, C> {
    type Target = GuestCallData<'c, D, C>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<D, C> DerefMut for GuestCall<'_, D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

/// A guest call to a WASM component function, which may be executed asynchronously.
///
/// It's expected that the `call_async` method will be called to execute the function call in all
/// cases, unless an error occurs during the setup of the call.
pub struct AsyncGuestCall<'c, D: Send + 'static, C> {
    data: GuestCallData<'c, D, C>,
}

impl<'c, D: Send, C> AsyncGuestCall<'c, D, C> {
    /// Calls the underlying WASM component function with the provided arguments and results.
    ///
    /// Returns an error if the function call fails, or an `AsyncGuestResult` containing the results
    /// of the call.
    pub async fn call_async(mut self) -> Result<AsyncGuestResult<'c, D, C>, anyhow::Error> {
        self.function
            .call_async(&mut self.data.store, self.data.arguments, self.data.results)
            .await?;

        Ok(AsyncGuestResult { context: self.data })
    }
}

impl<'c, D: Send, C> Deref for AsyncGuestCall<'c, D, C> {
    type Target = GuestCallData<'c, D, C>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<D: Send, C> DerefMut for AsyncGuestCall<'_, D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

/// A result of a guest call to a WASM component function, which contains the returned value(s) of
/// the underlying WASM call.
pub struct GuestResult<'c, D: 'static, C> {
    context: GuestCallData<'c, D, C>,
}

impl<D: 'static, C> GuestResult<'_, D, C> {
    /// Returns an immutable reference to the results of the WASM function call.
    #[must_use]
    pub fn results(&self) -> &[Val] {
        self.context.results
    }

    pub(crate) fn post_return(&mut self) -> Result<(), anyhow::Error> {
        self.context.function.post_return(&mut self.context.store)
    }
}

impl<'c, D: 'static, C> Deref for GuestResult<'c, D, C> {
    type Target = GuestCallData<'c, D, C>;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl<D, C> DerefMut for GuestResult<'_, D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}

/// Like `GuestResult`, but for asynchronous WASM function calls.
pub struct AsyncGuestResult<'c, D: Send + 'static, C> {
    context: GuestCallData<'c, D, C>,
}

impl<'c, D: Send + 'static, C> AsyncGuestResult<'c, D, C> {
    /// Returns an immutable reference to the results of the WASM function call.
    pub fn results(&self) -> &[Val] {
        self.context.results
    }

    pub(crate) async fn post_return_async(&mut self) -> Result<(), anyhow::Error> {
        self.context
            .function
            .post_return_async(&mut self.context.store)
            .await
    }
}

impl<'c, D: Send, C> Deref for AsyncGuestResult<'c, D, C> {
    type Target = GuestCallData<'c, D, C>;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl<'c, D: Send, C> DerefMut for AsyncGuestResult<'c, D, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.context
    }
}

/// A trampoline that manages multiple interfaces and their respect trampoline functions and
/// contexts for a component package.
pub struct PackageTrampoline<T, C> {
    trampoline: T,
    interface_context_overrides: HashMap<String, C>,
    default_context: C,
}

impl<T, C> PackageTrampoline<T, C> {
    /// Creates a new `PackageTrampoline` with the given trampoline and a default context.
    pub fn new(trampoline: T) -> Self
    where
        C: Default,
    {
        Self::with_default_context(trampoline, C::default())
    }

    /// Creates a new `PackageTrampoline` with the given trampoline and a specific default context.
    pub fn with_default_context(trampoline: T, default_context: C) -> Self {
        Self {
            trampoline,
            interface_context_overrides: HashMap::new(),
            default_context,
        }
    }

    /// Returns a reference to the trampoline function.
    pub fn trampoline(&self) -> &T {
        &self.trampoline
    }

    /// Returns a reference to the trampoline context used for all interfaces not otherwise defined.
    pub fn default_context(&self) -> &C {
        &self.default_context
    }

    /// Sets the default context for the trampoline.
    pub fn set_default_context(&mut self, context: C) {
        self.default_context = context;
    }

    /// Returns a reference to the trampoline context for a specific interface, if it has been
    /// overridden. If `None` is return, it's expected that the default context will be used.
    pub fn get_interface_context(&self, interface_name: &str) -> Option<&C> {
        self.interface_context_overrides.get(interface_name)
    }

    /// Sets the trampoline context for a specific interface, overriding the default context.
    pub fn set_interface_context(&mut self, interface_name: &str, context: C) {
        self.interface_context_overrides
            .insert(interface_name.to_string(), context);
    }

    /// Removes the trampoline context override for a specific interface, reverting to the default.
    ///
    /// If the interface context override does not exist, this is a no-op.
    pub fn remove_interface_context(&mut self, interface_name: &str) {
        self.interface_context_overrides.remove(interface_name);
    }

    /// Returns an `InterfaceTrampoline` for the specified interface name, using the context
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

/// A trampoline that allows for calling a specific interface function with a context.
#[derive(Clone)]
pub struct InterfaceTrampoline<T, C> {
    trampoline: T,
    context: C,
}

impl<T, C> InterfaceTrampoline<T, C> {
    /// Runs the specified function with the given arguments and results, using the trampoline for
    /// execution interception.
    #[allow(clippy::too_many_arguments)]
    pub fn bounce<'c, D: 'static>(
        &'c self,
        function: &'c Func,
        store: StoreContextMut<'c, D>,
        path: &'c ForeignInterfacePath,
        method: &'c str,
        ty: &'c FuncType,
        arguments: &'c [Val],
        results: &'c mut [Val],
    ) -> Result<GuestResult<'c, D, C>, anyhow::Error>
    where
        T: Trampoline<D, C>,
    {
        self.trampoline.bounce(GuestCall {
            data: GuestCallData {
                store,
                function,
                context: &self.context,
                path,
                method,
                ty,
                arguments,
                results,
            },
        })
    }

    /// Like `bounce`, but for asynchronous function calls.
    #[allow(clippy::too_many_arguments)]
    pub async fn bounce_async<'c, D>(
        &'c self,
        function: &'c Func,
        store: StoreContextMut<'c, D>,
        path: &'c ForeignInterfacePath,
        method: &'c str,
        ty: &'c FuncType,
        arguments: &'c [Val],
        results: &'c mut [Val],
    ) -> Result<AsyncGuestResult<'c, D, C>, anyhow::Error>
    where
        D: Send + 'static,
        C: Send + Sync,
        T: AsyncTrampoline<D, C>,
    {
        self.trampoline
            .bounce_async(AsyncGuestCall {
                data: GuestCallData {
                    store,
                    function,
                    context: &self.context,
                    path,
                    method,
                    ty,
                    arguments,
                    results,
                },
            })
            .await
    }
}

/// An abstract trampoline that is either defined for synchronous or asynchronous WASM function calls.
#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub enum DynInterfaceTrampoline<D, C: Clone> {
    Sync(InterfaceTrampoline<Arc<dyn Trampoline<D, C>>, C>),
    Async(InterfaceTrampoline<Arc<dyn AsyncTrampoline<D, C>>, C>),
}

/// A package-level trampoline factory for each interface name.
pub trait DynPackageTrampoline<D, C: Clone> {
    fn interface_trampoline(&self, interface_name: &str) -> DynInterfaceTrampoline<D, C>;
}

impl<D, C: Clone> DynPackageTrampoline<D, C> for PackageTrampoline<Arc<dyn Trampoline<D, C>>, C> {
    fn interface_trampoline(&self, interface_name: &str) -> DynInterfaceTrampoline<D, C> {
        DynInterfaceTrampoline::Sync(self.interface_trampoline(interface_name))
    }
}

impl<D, C: Clone> DynPackageTrampoline<D, C>
    for PackageTrampoline<Arc<dyn AsyncTrampoline<D, C>>, C>
{
    fn interface_trampoline(&self, interface_name: &str) -> DynInterfaceTrampoline<D, C> {
        DynInterfaceTrampoline::Async(self.interface_trampoline(interface_name))
    }
}
