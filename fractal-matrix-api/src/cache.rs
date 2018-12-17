use std::collections::HashMap;
use std::time::Instant;

#[derive(Clone)]
pub struct CacheMap<T> {
    map: HashMap<String, (Instant, T)>,
    timeout: u64,
}

impl<T> CacheMap<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            timeout: 10,
        }
    }

    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn get(&self, k: &String) -> Option<&T> {
        self.map.get(k).and_then(|t| {
            if t.0.elapsed().as_secs() >= self.timeout {
                None
            } else {
                Some(&t.1)
            }
        })
    }

    pub fn insert(&mut self, k: String, v: T) {
        let now = Instant::now();
        self.map.insert(k, (now, v));
    }
}
