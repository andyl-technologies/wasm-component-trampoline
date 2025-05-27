#[cfg(target_family = "wasm")]
fn main() {}

#[cfg(not(target_family = "wasm"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    runner::main().await
}

#[cfg(not(target_family = "wasm"))]
mod runner {
    use anyhow::Error;
    use semver::Version;
    use std::path::Path;
    use std::pin::Pin;
    use std::sync::Arc;
    use tokio::fs;
    use wac_trampoline::{AsyncGuestCall, AsyncGuestResult, AsyncTrampoline, CompositionGraph};
    use wasmtime::{Config, Engine, Store, component::Linker};

    wasmtime::component::bindgen!({
        path: "../wasm/application/wit",
        async: true,
    });

    // Define our store data type
    #[derive(Debug)]
    struct AppData {
        // We could add application-specific data here if needed
    }

    // Simple async trampoline that just passes calls through
    struct PassthroughTrampoline /*<C: Clone + Sync + Send + 'static>*/ {}

    impl AsyncTrampoline<AppData, ()> for PassthroughTrampoline {
        fn bounce_async<'c>(
            &'c self,
            call: AsyncGuestCall<'c, AppData, ()>,
        ) -> Pin<
            Box<dyn Future<Output = Result<AsyncGuestResult<'c, AppData, ()>, Error>> + Send + 'c>,
        > {
            Box::pin(async move {
                eprintln!(
                    "Bounced call '{}#{}'",
                    call.interface().to_string(),
                    call.method()
                );

                let result = call.call_async().await?;

                eprintln!(
                    "Bounced return '{}#{}'",
                    result.interface().to_string(),
                    result.method()
                );

                Ok(result)
            })
        }
    }

    // TODO(bill): directory from command line
    const WASM_DIR: &str = "target/wasm32-unknown-unknown/release/";

    // TODO(bill): packages from command line
    async fn add_package(
        graph: &mut CompositionGraph<AppData>,
        path: &str,
        name: &str,
        version: Version,
    ) -> Result<wac_trampoline::PackageId, wac_trampoline::AddPackageError> {
        eprintln!("Loading {path} component...");
        let wasm_dir = Path::new(WASM_DIR);
        let wasm_file = format!("{path}.component.wasm").to_string();
        let pkg_bytes = fs::read(wasm_dir.join(&wasm_file))
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to read {}/{wasm_file} Make sure it's been compiled",
                    wasm_dir.display()
                )
            });

        let trampoline: Arc<dyn AsyncTrampoline<AppData, ()>> = Arc::new(PassthroughTrampoline {});
        let pkg = wac_trampoline::PackageTrampoline::with_default_context(trampoline, ());

        let ret = graph.add_package(name.to_string(), version, pkg_bytes, pkg);
        eprintln!("{name} component loaded successfully.");
        ret
    }

    pub async fn main() -> anyhow::Result<()> {
        let verbose = false; // TODO(bill): command line option
        // Configure the WebAssembly engine
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);

        let engine = Engine::new(&config)?;
        let mut linker = Linker::new(&engine);
        let mut store = Store::new(&engine, AppData {});

        // Create our composition graph
        let mut graph = CompositionGraph::<AppData>::new();
        // Load the logger component
        add_package(&mut graph, "logger", "test:logging", Version::new(1, 1, 1)).await?;

        // Load the KV store component
        let _kvstore_id =
            add_package(&mut graph, "kvstore", "test:kvstore", Version::new(2, 1, 6)).await?;

        // Load the application component
        let app_id = add_package(
            &mut graph,
            "application",
            "test:application",
            Version::new(0, 4, 0),
        )
        .await?;

        // Instantiate the components
        eprintln!("Instantiating components...");
        if verbose {
            eprintln!("graph: {graph:#?}");
        }

        let instance = graph
            .instantiate_async(app_id, &mut linker, &mut store, &engine)
            .await?;

        eprintln!("Components instantiated successfully.");

        let application = Application::new(&mut store, &instance)?;

        application
            .test_application_greeter()
            .call_set_name(&mut store, "Dave")
            .await?;

        let hello = application
            .test_application_greeter()
            .call_hello(&mut store)
            .await?;

        println!("Greeter Output: {:?}", &hello);
        assert_eq!(hello, "Hello Dave!");

        println!("Test completed successfully!");
        Ok(())
    }
}
