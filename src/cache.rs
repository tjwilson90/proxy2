use std::collections::HashMap;
use std::sync::Mutex;

pub struct Cache {
    cache: Mutex<HashMap<String, String>>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, uri: String, response: String) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(uri, response);
    }

    pub fn remove(&self, uri: &str) -> Option<String> {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(uri)
    }
}
