use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

// user info cache, uid -> (name, avatar)
#[derive(Clone, Debug)]
pub struct CacheMap<K: Clone + Eq + Hash, V: Clone> {
    map: HashMap<K, (Instant, V)>,
    timeout: Duration,
}

impl<K: Clone + Eq + Hash, V: Clone> CacheMap<K, V> {
    pub fn new() -> Self {
        CacheMap {
            map: HashMap::new(),
            timeout: Duration::from_secs(10),
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        match self.map.get(k) {
            Some(t) => {
                if t.0.elapsed() >= self.timeout {
                    return None;
                }
                Some(&t.1)
            }
            None => None,
        }
    }

    pub fn insert(&mut self, k: K, v: V) {
        let now = Instant::now();
        self.map.insert(k, (now, v));
    }
}
