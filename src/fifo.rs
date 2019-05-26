use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash};
use std::ptr;

use super::map::LinkedHashMap;

pub struct Cache<K, V, S = RandomState> {
    max_size: usize,

    hit_count: usize,
    miss_count: usize,

    callback: Option<Box<dyn Fn(K, V)>>,

    l_map: LinkedHashMap<K, V, S>,
}

impl<K: Hash + Eq, V> Cache<K, V, RandomState> {
    pub fn new(max_size: usize) -> Cache<K, V, RandomState> {
        Cache::with_hasher(max_size, Default::default())
    }
}

impl<K: Hash + Eq, V, S> Cache<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    pub fn with_hasher(max_size: usize, hash_builder: S) -> Cache<K, V, S> {
        let max_size = if max_size < 1 { 1 } else { max_size };
        Cache {
            max_size,
            hit_count: 0,
            miss_count: 0,
            callback: None,
            l_map: LinkedHashMap::with_capacity_and_hasher(max_size, hash_builder),
        }
    }

    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.l_map.contains_key(k)
    }

    pub fn set_eviction_callback<C>(&mut self, cb: C)
    where
        C: Fn(K, V) + 'static,
    {
        self.callback = Some(Box::new(cb));
    }

    pub fn get<Q: ?Sized>(&mut self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        if let Some(v) = self.l_map.get(k) {
            self.hit_count += 1;
            return Some(&v);
        }
        self.miss_count += 1;
        None
    }

    pub fn peek<Q: ?Sized>(&mut self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.l_map.get(k)
    }

    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.l_map.remove(k)
    }

    pub fn remove_entry<Q: ?Sized>(&mut self, k: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.l_map.remove_entry(k)
    }

    pub fn add(&mut self, k: K, v: V) -> Option<V> {
        if let Some(val) = self.l_map.get_mut(&k) {
            let old_v = unsafe { ptr::replace(val, v) };
            return Some(old_v);
        }

        if self.len() + 1 > self.max_size {
            self.l_map
                .pop_back()
                .map(|(k, v)| self.callback.as_ref().map(|cb| cb(k, v)));
        }
        self.l_map.push_front(k, v);
        None
    }

    pub fn len(&self) -> usize {
        self.l_map.len()
    }

    pub fn purge(&mut self) {
        self.l_map.clear()
    }

    pub fn is_empty(&self) -> bool {
        self.l_map.is_empty()
    }

    pub fn shrink_to_fit(&mut self) {
        self.l_map.shrink_to_fit();
    }

    pub fn stat(&self) -> Info {
        Info {
            hit_count: self.hit_count,
            miss_count: self.miss_count,
        }
    }
}

pub struct Info {
    pub hit_count: usize,
    pub miss_count: usize,
}
