use crate::test::types::types::Level;

wit_bindgen::generate!({
    generate_all,
});

#[derive(Debug)]
pub struct Logger;

impl exports::test::logging::logger::Guest for Logger {
    fn log(lvl: Level, msg: String) {
        let lvl_str = match lvl {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Error => "ERROR",
        };

        println(format!("[{lvl_str}]: {msg}").as_str());
    }
}

export!(Logger);
