use std::collections::HashMap;
use std::hash::Hash;
use std::time::Instant;

#[derive(Clone)]
pub struct CacheMap<K: Clone, V: Clone> {
    map: HashMap<K, (Instant, V)>,
    timeout: u64,
}

impl<K: Clone + Eq + Hash, V: Clone> CacheMap<K, V> {
    pub fn new() -> Self {
        CacheMap {
            map: HashMap::new(),
            timeout: 10,
        }
    }

    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        match self.map.get(k) {
            Some(t) => {
                if t.0.elapsed().as_secs() >= self.timeout {
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

impl<K: Clone + Eq + Hash, V: Clone> Default for CacheMap<K, V> {
    fn default() -> Self {
        CacheMap::new()
    }
}
