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
        GLOBAL_MAP.lock().unwrap().insert(key, value);
    }

    fn get(key: String) -> Option<String> {
        GLOBAL_MAP.lock().unwrap().get(&key).cloned()
    }
}
