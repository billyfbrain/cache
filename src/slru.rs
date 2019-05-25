use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash};
use std::ptr;

use super::map::LinkedHashMap;

const DEFAULT_MAIN_CF: f64 = 0.75;

pub struct Cache<K, V, S = RandomState> {
    max_size: usize,
    max_size_in: usize,
    max_size_main: usize,

    hit_count: usize,
    miss_count: usize,

    callback: Option<Box<dyn Fn(K, V)>>,

    in_: LinkedHashMap<K, V, S>,
    main: LinkedHashMap<K, V, S>,
}

impl<K: Hash + Eq, V, S> Cache<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher + Clone,
{
    pub fn with_hasher(size: usize, hash_builder: S) -> Cache<K, V, S> {
        Cache::with_param_and_hasher(size, DEFAULT_MAIN_CF, hash_builder)
    }

    pub fn with_param_and_hasher(
        size: usize,
        main_cache_factor: f64,
        hash_builder: S,
    ) -> Cache<K, V, S> {
        let max_size = if size < 2 { 2 } else { size };

        let max_size_main = (max_size as f64 * main_cache_factor) as usize;
        let max_size_in = (max_size as f64 * (1 as f64 - main_cache_factor)) as usize;
        Cache {
            max_size,
            max_size_in,
            max_size_main,

            callback: None,

            hit_count: 0,
            miss_count: 0,

            in_: LinkedHashMap::with_capacity_and_hasher(max_size_in, hash_builder.clone()),
            main: LinkedHashMap::with_capacity_and_hasher(max_size_main, hash_builder),
        }
    }

    pub fn set_eviction_callback<C>(&mut self, cb: C)
    where
        C: Fn(K, V) + 'static,
    {
        self.callback = Some(Box::new(cb));
    }

    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.main.contains_key(key) || self.in_.contains_key(key)
    }

    pub fn get<Q: ?Sized>(&mut self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        if self.main.contains_key(key) {
            self.hit_count += 1;
            self.main.move_to_front(key);
            return self.main.get(key);
        }

        if let Some((k, v)) = self.in_.remove_entry(key) {
            self.hit_count += 1;
            self.ensure_space(true);
            self.main.push_front(k, v);
            return self.main.get(key);
        }
        self.miss_count += 1;
        None
    }

    pub fn add(&mut self, key: K, value: V) -> Option<V> {
        if let Some(v) = self.main.get_mut(&key) {
            let old_v = unsafe { ptr::replace(v, value) };
            self.main.move_to_front(&key);
            return Some(old_v);
        }

        if self.in_.remove_entry(&key).is_some() {
            self.ensure_space(true);
            self.main.push_front(key, value);
            return None;
        }

        self.ensure_space(false);
        self.in_.push_front(key, value);
        None
    }

    fn ensure_space(&mut self, main: bool) {
        if main && self.main.len() >= self.max_size_main {
            if let Some((k, v)) = self.main.pop_back() {
                self.in_.push_front(k, v);
            }
        }

        let in_len = self.in_.len();
        let main_len = self.main.len();
        if in_len + main_len < self.max_size {
            return;
        }

        if in_len > 0 && (in_len > self.max_size_in || (in_len == self.max_size_in && !main)) {
            if let Some((k, v)) = self.in_.pop_back() {
                self.callback.as_ref().map(|cb| cb(k, v));
            }
        }
    }

    pub fn remove(&mut self, key: &K) -> bool {
        self.main
            .remove(key)
            .or_else(|| self.in_.remove(key))
            .is_some()
    }

    pub fn purge(&mut self) {
        self.main.clear();
        self.in_.clear();
    }

    pub fn len(&self) -> usize {
        self.main.len() + self.in_.len()
    }

    pub fn is_empty(&self) -> bool {
        self.main.is_empty() && self.in_.is_empty()
    }

    pub fn peek(&self, key: &K) -> Option<&V> {
        if let Some(v) = self.main.get(key) {
            return Some(v);
        }
        self.in_.get(key)
    }

    pub fn shrink_to_fit(&mut self) {
        self.in_.shrink_to_fit();
        self.main.shrink_to_fit();
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

impl<K: Hash + Eq, V> Cache<K, V, RandomState> {
    pub fn with_params(size: usize, main_cache_factor: f64) -> Cache<K, V, RandomState> {
        Cache::with_param_and_hasher(size, main_cache_factor, Default::default())
    }

    pub fn new(size: usize) -> Cache<K, V, RandomState> {
        Cache::with_params(size, DEFAULT_MAIN_CF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{self, Rng};

    #[test]
    fn test_random_ops() {
        let size = 128;
        let mut cache: Cache<i64, i64> = Cache::new(size);
        let mut rng = rand::thread_rng();

        for _ in 0usize..20000 {
            let mut key: i64 = rng.gen();
            key %= 512;
            let r: i64 = rng.gen();

            match r % 3 {
                0 => {
                    cache.add(key, key);
                }
                1 => {
                    cache.get(&key);
                }
                2 => {
                    cache.remove(&key);
                }
                _ => {}
            }
            assert!(cache.main.len() + cache.in_.len() <= size);
        }
    }

    #[test]
    fn test_get_in_main() {
        let size = 128;
        let mut cache: Cache<usize, usize> = Cache::new(size);
        for i in 0usize..size {
            cache.add(i, i);
        }

        assert_eq!(cache.in_.len(), 128);
        assert_eq!(cache.main.len(), 0);

        for i in 0usize..size {
            assert!(cache.get(&i).is_some());
        }

        assert_eq!(cache.in_.len(), 32);
        assert_eq!(cache.main.len(), 96);
    }

    #[test]
    fn test_add_in_to_main() {
        let size = 128;
        let mut cache: Cache<usize, usize> = Cache::new(size);

        cache.add(1, 1);
        assert_eq!(cache.in_.len(), 1);
        assert_eq!(cache.main.len(), 0);

        cache.add(1, 1);
        assert_eq!(cache.in_.len(), 0);
        assert_eq!(cache.main.len(), 1);

        cache.add(1, 1);
        assert_eq!(cache.in_.len(), 0);
        assert_eq!(cache.main.len(), 1);
    }

    use std::cell::RefCell;
    use std::rc::Rc;


    #[test]
    fn test_back_to_in_front() {
        let size = 4;
        let mut cache: Cache<usize, usize> = Cache::new(size);

        let e_count = Rc::new(RefCell::new(0));
        let count = e_count.clone();
        cache.set_eviction_callback(move |_, _| {
            *count.borrow_mut() += 1;
        });

        cache.add(1, 1);
        cache.add(2, 2);
        cache.add(3, 3);
        cache.add(4, 4);
        cache.add(5, 5);
        assert_eq!(cache.in_.len(), 4);
        assert_eq!(cache.main.len(), 0);
        assert_eq!(*e_count.as_ref().borrow(), 1);

        assert!(cache.get(&4).is_some());
        assert!(cache.get(&3).is_some());
        assert!(cache.get(&5).is_some());
        assert_eq!(cache.in_.len(), 1);
        assert_eq!(cache.main.len(), 3);
        assert_eq!(cache.main.front(), Some((&5, &5)));

        assert!(cache.get(&2).is_some());
        assert_eq!(cache.in_.len(), 1);
        assert_eq!(cache.main.len(), 3);

        assert_eq!(cache.in_.front(), Some((&4, &4)));
        assert_eq!(cache.main.back(), Some((&3, &3)));
        assert_eq!(cache.main.front(), Some((&2, &2)));

        cache.add(6, 6);
        assert!(cache.get(&6).is_some());
        assert_eq!(cache.main.back(), Some((&5, &5)));
        assert_eq!(cache.main.front(), Some((&6, &6)));
        assert_eq!(cache.in_.front(), Some((&3, &3)));

        assert_eq!(cache.in_.len(), 1);
        assert_eq!(cache.main.len(), 3);
    }

    #[test]
    fn test_cache() {
        let mut cache: Cache<usize, usize> = Cache::new(128);
        for i in 0usize..256 {
            cache.add(i, i);
        }
        assert_eq!(cache.len(), 128);

        for i in 0usize..128 {
            assert!(cache.get(&i).is_none());
        }

        for i in 128usize..256 {
            assert!(cache.get(&i).is_some());
        }

        for i in 128usize..192 {
            cache.remove(&i);
            assert!(cache.get(&i).is_none());
        }

        cache.purge();
        assert!(cache.is_empty());
        assert!(cache.get(&200).is_none());
    }

    #[test]
    fn test_contains() {
        let mut cache: Cache<usize, usize> = Cache::new(2);
        cache.add(1, 1);
        cache.add(2, 2);
        assert!(cache.contains_key(&1));
        cache.add(3, 3);
        assert!(!cache.contains_key(&1));
    }

    #[test]
    fn test_peek() {
        let mut cache: Cache<usize, usize> = Cache::new(2);
        cache.add(1, 1);
        cache.add(2, 2);
        assert_eq!(cache.peek(&1), Some(&1));
        cache.add(3, 3);
        assert!(!cache.contains_key(&1));
    }
}
