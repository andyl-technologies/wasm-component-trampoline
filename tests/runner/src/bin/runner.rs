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
    use anyhow::Error;
    use clap::Parser;
    use semver::Version;
    use std::path::PathBuf;

    use runner::cli::Args;
    use std::sync::Arc;
    use regex::Regex;
    use tokio::fs;
    use wasm_component_trampoline::{CompositionGraph, GuestCall, GuestResult, ImportRule, RegexMatchFilter, Trampoline};
    use wasmtime::component::HasSelf;
    use wasmtime::{Config, Engine, Store, component::Linker};

    wasmtime::component::bindgen!({
        path: "../wasm/application/wit",
        async: false,
    });

    mod kvstore {
        wasmtime::component::bindgen!({
            path: "../wasm/kvstore/wit",
            async: false,
        });
    }

    mod logger {
        wasmtime::component::bindgen!({
            path: "../wasm/logger/wit",
            async: false,
        });
    }

    #[derive(Default, Debug)]
    struct HostInterface;

    impl logger::test::logging::system::Host for HostInterface {
        fn println(&mut self, msg: String) {
            eprintln!("{msg}")
        }
    }

    // Define our store data type
    #[derive(Default, Debug)]
    struct AppData {
        host: HostInterface,
        stack_depth: usize,
    }

    // Simple async trampoline that just passes calls through
    struct PassthroughTrampoline {}
    impl Trampoline<AppData, ()> for PassthroughTrampoline {
        fn bounce<'c>(
            &self,
            mut call: GuestCall<'c, AppData, ()>,
        ) -> Result<GuestResult<'c, AppData, ()>, Error> {
            eprintln!(
                "[{}] Bounced call '{}#{}'",
                call.store().data().stack_depth,
                call.interface(),
                call.method(),
            );

            call.store_mut().data_mut().stack_depth += 1;

            let mut result = call.call()?;

            result.store_mut().data_mut().stack_depth -= 1;

            eprintln!(
                "[{}] Bounced return '{}#{}'",
                result.store().data().stack_depth,
                result.interface(),
                result.method(),
            );

            Ok(result)
        }
    }

    async fn add_package(
        graph: &mut CompositionGraph<AppData>,
        wasm_dir: &PathBuf,
        path: &str,
        name: &str,
        version: Version,
    ) -> Result<wasm_component_trampoline::PackageId, wasm_component_trampoline::AddPackageError>
    {
        eprintln!("Loading {path} component...");

        let wasm_file = format!("{path}.component.wasm").to_string();
        let pkg_bytes = fs::read(wasm_dir.join(&wasm_file))
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "failed to read {}/{wasm_file}; make sure it's been compiled!",
                    wasm_dir.display()
                )
            });

        let trampoline: Arc<dyn Trampoline<AppData, ()>> = Arc::new(PassthroughTrampoline {});
        let pkg =
            wasm_component_trampoline::PackageTrampoline::with_default_context(trampoline, ());

        let ret = graph.add_package(name.to_string(), version, pkg_bytes, pkg);
        eprintln!("{name} component loaded successfully.");
        ret
    }

    pub async fn main() -> anyhow::Result<()> {
        let args = Args::parse();

        // Configure the WebAssembly engine
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(false);

        let engine = Engine::new(&config)?;
        let mut linker: Linker<AppData> = Linker::new(&engine);
        let mut store = Store::new(&engine, AppData::default());

        // Add host interfaces to the linker.
        logger::test::logging::system::add_to_linker::<_, HasSelf<_>>(
            &mut linker,
            |ctx: &mut _| &mut ctx.host,
        )?;

        // Create our composition graph
        let mut graph = CompositionGraph::<AppData>::new();

        graph.set_import_filter(RegexMatchFilter::new(
            Regex::new(r"^test:logging/system")?,
            ImportRule::Skip,
        ));

        // Load the logger component
        add_package(
            &mut graph,
            &args.wasm_dir,
            "logger",
            "test:logging",
            Version::new(1, 1, 1),
        )
        .await?;
        add_package(
            &mut graph,
            &args.wasm_dir,
            "logger",
            "test:logging",
            Version::new(1, 1, 1),
        )
        .await
        .expect_err("Duplicate logger component should not be allowed");

        // Load the KV store component
        let _kvstore_id = add_package(
            &mut graph,
            &args.wasm_dir,
            "kvstore",
            "test:kvstore",
            Version::new(2, 1, 6),
        )
        .await?;

        // Load the application component
        let app_id = add_package(
            &mut graph,
            &args.wasm_dir,
            "application",
            "test:application",
            Version::new(0, 4, 0),
        )
        .await?;

        // Instantiate the components
        eprintln!("Instantiating components...");
        if args.verbose {
            eprintln!("graph: {graph:#?}");
        }

        let instance = graph.instantiate(app_id, &mut linker, &mut store, &engine)?;

        eprintln!("Components instantiated successfully.");

        let application = Application::new(&mut store, &instance)?;

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
