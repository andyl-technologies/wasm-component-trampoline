wit_bindgen::generate!({
    generate_all,
});

#[derive(Debug)]
pub struct Store;

impl exports::test::application::greeter::Guest for Store {
    fn hello() -> String {
        let name = test::kvstore::store::get("name").unwrap_or("World".to_string());
        format!("Hello {}!", name)
    }

    fn set_name(name: String) -> () {
        test::logging::logger::log(format!("setting name to {}", name).as_str());
        test::kvstore::store::set("name", name.as_str());
    }
}
