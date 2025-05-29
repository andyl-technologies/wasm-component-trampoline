use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

wit_bindgen::generate!({
    generate_all,
});

// Thread-safe global key-value store
static GLOBAL_MAP: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug)]
pub struct Store;

impl exports::test::kvstore::store::Guest for Store {
    fn set(key: String, value: String) {
        test::logging::logger::log(format!("setting key '{key}' to value {value:?}").as_str());

        GLOBAL_MAP.lock().unwrap().insert(key, value);
    }

    fn get(key: String) -> Option<String> {
        let value = GLOBAL_MAP.lock().unwrap().get(&key).cloned();

        test::logging::logger::log(format!("getting key '{key}' as value {value:?}").as_str());

        value
    }
}

export!(Store);
