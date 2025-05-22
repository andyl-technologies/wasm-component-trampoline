use std::collections::HashMap;
use std::sync::LazyLock;

wit_bindgen::generate!({
    generate_all,
});

static mut GLOBAL_MAP: LazyLock<HashMap<String, String>> = LazyLock::default();

#[derive(Debug)]
pub struct Store;

impl exports::test::kvstore::store::Guest for Store {
    fn set(key: String, value: String) -> () {
        // Safety: This is a single-threaded application, so we can safely mutate the global map.
        unsafe {
            GLOBAL_MAP.insert(key.clone(), value.clone());
        }
    }

    fn get(key: String) -> Option<String> {
        // Safety: This is a single-threaded application, so we can safely read the global map.
        unsafe { GLOBAL_MAP.get(&key).cloned() }
    }
}
