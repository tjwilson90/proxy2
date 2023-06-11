use hyper::Uri;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct Cache {
    cache: Mutex<HashMap<Uri, String>>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, uri: Uri, response: String) {
        let mut cache = self.cache.lock().unwrap();
        cache.insert(uri, response);
    }

    pub fn remove(&self, uri: &Uri) -> Option<String> {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(uri)
    }
}
