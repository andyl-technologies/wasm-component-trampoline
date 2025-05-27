wit_bindgen::generate!();

#[derive(Debug)]
pub struct Logger;

impl exports::test::logging::logger::Guest for Logger {
    fn log(msg: String) {
        println(format!("[LOG]: {msg}").as_str());
    }
}

export!(Logger);
