wit_bindgen::generate!();

#[derive(Debug)]
pub struct Logger;

impl exports::test::logging::logger::Guest for Logger {
    fn log(msg: String) -> () {
        todo!()
    }
}
