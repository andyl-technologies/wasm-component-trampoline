#[cfg(target_family = "wasm")]
fn main() {
    // This is a no-op for the wasm target, as the main function is not used.
    eprintln!("This is a WebAssembly target, no main function to run.");
}

#[cfg(not(target_family = "wasm"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    runner::main().await
}

#[cfg(not(target_family = "wasm"))]
mod runner {
    pub const WASM_NOOP: &str = r#"
(module 
  (func (export "entrypoint")
    nop
  )
)
"#;

    pub const WASM_COMPONENT_NOOP: &str = r#"
(component
  (core module (;0;)
    (table (;0;) 1 1 funcref)
    (memory (;0;) 16)
    (global $__stack_pointer (;0;) (mut i32) i32.const 1048576)
    (global (;1;) i32 i32.const 1048576)
    (global (;2;) i32 i32.const 1048576)
    (export "memory" (memory 0))
    (export "__data_end" (global 1))
    (export "__heap_base" (global 2))
    (@custom "target_features" (after export) "\04+\0amultivalue+\0fmutable-globals+\0freference-types+\08sign-ext")
  )
  (core instance (;0;) (instantiate 0))
  (alias core export 0 "memory" (core memory (;0;)))
  (@producers
    (processed-by "trampoline-mocks" "1.2.3")
  )
)
"#;

    use anyhow::Error;
    use semver::Version;
    use std::fmt::Debug;
    use std::sync::Arc;
    use wasm_component_trampoline::{CompositionGraph, GuestCall, GuestResult, Trampoline};
    use wasmtime::{component::Linker, Config, Engine, Store};

    wasmtime::component::bindgen!({
      path:  "../wasm/application/wit",
      async: false,
    });

    // Define our store data type
    #[derive(Debug)]
    struct AppData {}

    impl std::fmt::Debug for Application {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Application")
        }
    }

    // Simple async trampoline that just passes calls through
    struct PassthroughTrampoline {}
    impl Trampoline<AppData, ()> for PassthroughTrampoline {
        fn bounce<'c>(
            &self,
            mut _call: GuestCall<'c, AppData, ()>,
        ) -> Result<GuestResult<'c, AppData, ()>, Error> {
            todo!("we expect this to fail, so we don't implement the bounce logic here");
        }
    }

    // TODO(bill): packages from command line
    async fn add_package(
        graph: &mut CompositionGraph<AppData>,
        name: &str,
        version: Version,
        pkg_bytes: Vec<u8>,
    ) -> Result<wasm_component_trampoline::PackageId, wasm_component_trampoline::AddPackageError>
    {
        eprintln!("Loading {name} component...");
        let trampoline: Arc<dyn Trampoline<AppData, ()>> = Arc::new(PassthroughTrampoline {});
        let pkg =
            wasm_component_trampoline::PackageTrampoline::with_default_context(trampoline, ());

        let ret = graph.add_package(name.to_string(), version, pkg_bytes, pkg);
        eprintln!("{name} component loaded successfully.");
        ret
    }

    pub async fn main() -> anyhow::Result<()> {
        let verbose = false; // TODO(bill): command line option
                             // Configure the WebAssembly engine
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(false);

        let engine = Engine::new(&config)?;
        let mut linker = Linker::new(&engine);
        let mut store = Store::new(&engine, AppData {});

        // Add global functions to the linker.
        linker.root().func_wrap(
            "println",
            |_store: wasmtime::StoreContextMut<'_, AppData>, args: (String,)| {
                let (message,) = args;
                eprintln!("{}", message);
                Ok(())
            },
        )?;

        // Create our composition graph
        let mut graph = CompositionGraph::<AppData>::new();

        let noop_component = wat::parse_str(WASM_COMPONENT_NOOP)?;
        let noop_module = wat::parse_str(WASM_NOOP)?;

        // Fail to load a module instead of a component
        add_package(
            &mut graph,
            "anything:module",
            Version::new(1, 1, 1),
            noop_module.clone(),
        )
        .await
        .expect_err("Failed to load noop module component");

        add_package(
            &mut graph,
            "test:component",
            Version::new(1, 1, 1),
            noop_component.clone(),
        )
        .await
        .expect("cannot load a noop component");

        // Fail to load the logger component with the wrong version
        add_package(
            &mut graph,
            "test:logging",
            Version::new(100, 0, 0),
            noop_component.clone(),
        )
        .await
        .expect_err("logger component with wrong version should not be allowed");

        // Load the KV store component
        let kvstore_id = add_package(
            &mut graph,
            "test:kvstore",
            Version::new(2, 1, 6),
            noop_component.clone(),
        )
        .await?;
        eprintln!("Components instantiated successfully.");

        // Instantiate the components
        eprintln!("Instantiating components...");
        if verbose {
            eprintln!("graph: {graph:#?}");
        }

        let instance = graph.instantiate(kvstore_id, &mut linker, &mut store, &engine)?;

        // We have not yet imported any Application, so this should fail.
        Application::new(&mut store, &instance).unwrap_err();

        // Load the application component
        let app_id = add_package(
            &mut graph,
            "test:application",
            Version::new(0, 1, 0),
            noop_component.clone(),
        )
        .await?;
        let instance = graph.instantiate(app_id, &mut linker, &mut store, &engine)?;

        // We have not yet imported the correct version for Application.
        Application::new(&mut store, &instance).unwrap_err();

        // Load the application component
        let app_id = add_package(
            &mut graph,
            "test:application",
            Version::new(0, 4, 0),
            noop_component.clone(),
        )
        .await?;
        let instance = graph.instantiate(app_id, &mut linker, &mut store, &engine)?;
        let application = Application::new(&mut store, &instance).unwrap();

        // Load the application component again
        let app_id = add_package(
            &mut graph,
            "test:application",
            Version::new(0, 4, 0),
            noop_component.clone(),
        )
        .await?;
        let instance = graph.instantiate(app_id, &mut linker, &mut store, &engine)?;
        Application::new(&mut store, &instance).unwrap_err();

        application
            .test_application_greeter()
            .call_set_name(&mut store, "Dave")?;

        let hello = application
            .test_application_greeter()
            .call_hello(&mut store)?;

        println!("Greeter Output: {:?}", &hello);
        assert_eq!(hello, "Hello Dave!");

        println!("Test completed successfully!");
        Ok(())
    }
}
