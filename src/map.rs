use std::borrow::Borrow;
use std::collections::{hash_map::RandomState, HashMap};
use std::hash::{BuildHasher, Hash, Hasher};
use std::mem;
use std::ptr::{self, NonNull};

#[derive(Debug)]
struct KeyPtr<K>(NonNull<K>);

impl<K> KeyPtr<K> {
    fn from(k: &K) -> KeyPtr<K> {
        KeyPtr(NonNull::from(k))
    }
}

impl<K: PartialEq> PartialEq for KeyPtr<K> {
    fn eq(&self, other: &Self) -> bool {
        unsafe { (self.0.as_ref()).eq(other.0.as_ref()) }
    }
}

impl<K: Eq> Eq for KeyPtr<K> {}

impl<K: Hash> Hash for KeyPtr<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe { (self.0.as_ref()).hash(state) }
    }
}

#[derive(Hash, PartialEq, Eq)]
struct KeyRef<Q: ?Sized>(Q);

impl<Q: ?Sized> KeyRef<Q> {
    fn new(k: &Q) -> &Self {
        unsafe { mem::transmute(k) }
    }
}

impl<K, Q: ?Sized> Borrow<KeyRef<Q>> for KeyPtr<K>
where
    K: Borrow<Q>,
{
    fn borrow(&self) -> &KeyRef<Q> {
        KeyRef::new(unsafe { (self.0.as_ref().borrow()) })
    }
}

struct Node<K, V> {
    next: Option<NonNull<Node<K, V>>>,
    prev: Option<NonNull<Node<K, V>>>,
    k: K,
    v: V,
}

impl<K, V> Node<K, V> {
    fn new(k: K, v: V) -> Self {
        Node {
            next: None,
            prev: None,
            k,
            v,
        }
    }
}

pub struct LinkedHashMap<K, V, S = RandomState> {
    head: Option<NonNull<Node<K, V>>>,
    tail: Option<NonNull<Node<K, V>>>,

    empty: Option<NonNull<Node<K, V>>>,
    empty_len: usize,

    map: HashMap<KeyPtr<K>, NonNull<Node<K, V>>, S>,
}

#[inline]
unsafe fn into_raw_non_null<T: ?Sized>(b: Box<T>) -> NonNull<T> {
    NonNull::new_unchecked(Box::into_raw(b))
}

impl<K: Hash + Eq, V> LinkedHashMap<K, V, RandomState> {
    #[inline]
    pub fn new() -> LinkedHashMap<K, V, RandomState> {
        Default::default()
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> LinkedHashMap<K, V, RandomState> {
        LinkedHashMap::with_capacity_and_hasher(capacity, Default::default())
    }
}

impl<K, V, S> LinkedHashMap<K, V, S> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.map.clear();
        while let Some(_) = self.pop_front_node() {}
        self.drop_empty();
    }

    #[inline]
    fn push_front_node(&mut self, mut node: Box<Node<K, V>>) {
        unsafe {
            node.next = self.head;
            node.prev = None;
            let node = Some(into_raw_non_null(node));

            match self.head {
                None => self.tail = node,
                Some(mut head) => head.as_mut().prev = node,
            }

            self.head = node;
        }
    }

    #[inline]
    fn pop_front_node(&mut self) -> Option<Box<Node<K, V>>> {
        self.head.map(|node| unsafe {
            let node = Box::from_raw(node.as_ptr());
            self.head = node.next;

            match self.head {
                None => self.tail = None,
                Some(mut head) => head.as_mut().prev = None,
            }

            node
        })
    }

    #[inline]
    fn push_back_node(&mut self, mut node: Box<Node<K, V>>) {
        unsafe {
            node.next = None;
            node.prev = self.tail;
            let node = Some(into_raw_non_null(node));

            match self.tail {
                None => self.head = node,
                Some(mut tail) => tail.as_mut().next = node,
            }

            self.tail = node;
        }
    }

    #[inline]
    fn pop_back_node(&mut self) -> Option<Box<Node<K, V>>> {
        self.tail.map(|node| unsafe {
            let node = Box::from_raw(node.as_ptr());
            self.tail = node.prev;

            match self.tail {
                None => self.head = None,
                Some(mut tail) => tail.as_mut().next = None,
            }
            node
        })
    }

    #[inline]
    unsafe fn unlink_node(&mut self, mut node: NonNull<Node<K, V>>) {
        let node = node.as_mut();

        match node.prev {
            Some(mut prev) => prev.as_mut().next = node.next.clone(),
            // this node is the head node
            None => self.head = node.next.clone(),
        };

        match node.next {
            Some(mut next) => next.as_mut().prev = node.prev.clone(),
            // this node is the tail node
            None => self.tail = node.prev.clone(),
        };
    }

    #[inline]
    unsafe fn flush_node(&mut self, mut node: Box<Node<K, V>>) -> (K, V) {
        node.as_mut().next = self.empty;
        node.as_mut().prev = None;
        let k = ptr::read(&node.k);
        let v = ptr::read(&node.v);
        self.empty = Some(into_raw_non_null(node));
        self.empty_len += 1;
        (k, v)
    }

    #[inline]
    fn new_node(&mut self, k: K, v: V) -> NonNull<Node<K, V>> {
        match self.empty {
            Some(empty) => {
                let node = empty;
                unsafe {
                    self.empty = node.as_ref().next;
                    ptr::write(node.as_ptr(), Node::new(k, v));
                }
                self.empty_len -= 1;
                node
            }
            None => unsafe { into_raw_non_null(Box::new(Node::new(k, v))) },
        }
    }

    #[inline]
    fn drop_empty(&mut self) {
        let mut count = 0;
        unsafe {
            while let Some(node) = self.empty {
                count += 1;
                self.empty = node.as_ref().next;
                Box::from_raw(node.as_ptr());
            }
        }
        assert_eq!(count, self.empty_len);
        self.empty_len = 0;
    }
}

impl<K, V, S> Default for LinkedHashMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher + Default,
{
    /// Creates an empty `LinkedHashMap<T>`.
    #[inline]
    fn default() -> LinkedHashMap<K, V, S> {
        LinkedHashMap::with_hasher(Default::default())
    }
}

impl<K, V, S> LinkedHashMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.map.reserve(additional);
    }

    #[inline]
    pub fn with_hasher(hash_builder: S) -> LinkedHashMap<K, V, S> {
        LinkedHashMap {
            head: None,
            tail: None,
            empty: None,
            empty_len: 0,
            map: HashMap::with_hasher(hash_builder),
        }
    }

    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> LinkedHashMap<K, V, S> {
        LinkedHashMap {
            head: None,
            tail: None,
            empty: None,
            empty_len: 0,
            map: HashMap::with_capacity_and_hasher(capacity, hash_builder),
        }
    }

    pub fn hasher(&self) -> &S {
        self.map.hasher()
    }

    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.contains_key(KeyRef::new(k))
    }

    #[inline]
    pub fn front(&self) -> Option<(&K, &V)> {
        unsafe {
            self.head
                .as_ref()
                .map(|node| (&node.as_ref().k, &node.as_ref().v))
        }
    }

    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut V> {
        unsafe { self.head.as_mut().map(|node| &mut node.as_mut().v) }
    }

    #[inline]
    pub fn back(&self) -> Option<(&K, &V)> {
        unsafe {
            self.tail
                .as_ref()
                .map(|node| (&node.as_ref().k, &node.as_ref().v))
        }
    }

    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut V> {
        unsafe { self.tail.as_mut().map(|node| &mut node.as_mut().v) }
    }

    pub fn pop_front(&mut self) -> Option<(K, V)> {
        let node = self.pop_front_node()?;
        self.map.remove(&KeyPtr::from(&node.k));
        Some(unsafe { self.flush_node(node) })
    }

    pub fn pop_back(&mut self) -> Option<(K, V)> {
        let node = self.pop_back_node()?;
        self.map.remove(&KeyPtr::from(&node.k));
        Some(unsafe { self.flush_node(node) })
    }

    pub fn get_key_value<Q: ?Sized>(&self, k: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map
            .get(KeyRef::new(k))
            .map(|node| unsafe { (&node.as_ref().k, &node.as_ref().v) })
    }

    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map
            .get(KeyRef::new(k))
            .map(|node| unsafe { &node.as_ref().v })
    }

    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let node = self.map.get_mut(KeyRef::new(k))?;
        Some(unsafe { &mut node.as_mut().v })
    }

    pub fn move_to_front<Q: ?Sized>(&mut self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let node = match self.map.get_mut(KeyRef::new(k)) {
            Some(node) => node.clone(),
            None => return false,
        };
        unsafe {
            self.unlink_node(node);
            self.push_front_node(Box::from_raw(node.as_ptr()));
        }
        true
    }

    pub fn move_to_back<Q: ?Sized>(&mut self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let node = match self.map.get_mut(KeyRef::new(k)) {
            Some(node) => node.clone(),
            None => return false,
        };
        unsafe {
            self.unlink_node(node);
            self.push_back_node(Box::from_raw(node.as_ptr()));
        }
        true
    }

    pub fn shrink_to_fit(&mut self) {
        self.map.shrink_to_fit();
        self.drop_empty();
    }

    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.remove(KeyRef::new(k)).map(|node| unsafe {
            self.unlink_node(node);
            let (_, v) = self.flush_node(Box::from_raw(node.as_ptr()));
            v
        })
    }

    pub fn remove_entry<Q: ?Sized>(&mut self, k: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.remove(KeyRef::new(k)).map(|node| unsafe {
            self.unlink_node(node);
            self.flush_node(Box::from_raw(node.as_ptr()))
        })
    }

    fn insert(&mut self, k: K, v: V) -> (Box<Node<K, V>>, Option<V>) {
        unsafe {
            let (node, old_v) = match self.map.get_mut(&KeyPtr::from(&k)) {
                Some(node) => (*node, Some(ptr::replace(&mut node.as_mut().v, v))),
                None => (self.new_node(k, v), None),
            };
            if old_v.is_some() {
                self.unlink_node(node);
            } else {
                self.map.insert(KeyPtr::from(&node.as_ref().k), node);
            }
            (Box::from_raw(node.as_ptr()), old_v)
        }
    }

    pub fn push_front(&mut self, k: K, v: V) -> Option<V> {
        let (node, old_v) = self.insert(k, v);
        self.push_front_node(node);
        old_v
    }

    pub fn push_back(&mut self, k: K, v: V) -> Option<V> {
        let (node, old_v) = self.insert(k, v);
        self.push_back_node(node);
        old_v
    }
}

unsafe impl<K: Send, V: Send, S: Send> Send for LinkedHashMap<K, V, S> {}

unsafe impl<K: Sync, V: Sync, S: Sync> Sync for LinkedHashMap<K, V, S> {}

impl<K, V, S> Drop for LinkedHashMap<K, V, S> {
    fn drop(&mut self) {
        while let Some(_) = self.pop_front_node() {}
        self.drop_empty();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_common() {
        type LHM = LinkedHashMap<i32, i32>;
        let mut m = LHM::new();
        assert_eq!(m.capacity(), 0);
        assert!(m.is_empty());
        assert!(m.map.is_empty());

        assert_eq!(m.push_front(1, 1), None);
        assert_eq!(m.push_front(2, 2), None);
        assert_eq!(m.push_front(3, 3), None);
        assert_eq!(m.push_front(4, 4), None);
        assert_eq!(m.push_front(5, 5), None);

        assert_eq!(m.get(&1), Some(&1));
        assert_eq!(m.get(&2), Some(&2));
        assert_eq!(m.get(&3), Some(&3));
        assert_eq!(m.get(&4), Some(&4));
        assert_eq!(m.get(&5), Some(&5));

        assert_eq!(m.front(), Some((&5, &5)));
        assert_eq!(m.back(), Some((&1, &1)));

        let el = m.get_mut(&5).unwrap();
        *el = 6;
        assert_eq!(m.get(&5), Some(&6));

        assert_eq!(m.empty_len, 0);
        assert_eq!(m.len(), 5);

        assert_eq!(m.pop_back(), Some((1, 1)));
        assert_eq!(m.pop_front(), Some((5, 6)));

        assert_eq!(m.front(), Some((&4, &4)));
        assert_eq!(m.back(), Some((&2, &2)));

        assert_eq!(m.remove(&4), Some(4));
        assert_eq!(m.get(&4), None);
        assert_eq!(m.front(), Some((&3, &3)));
        assert_eq!(m.back(), Some((&2, &2)));

        assert_eq!(m.remove(&2), Some(2));
        assert_eq!(m.back(), Some((&3, &3)));
        assert_eq!(m.front(), Some((&3, &3)));

        assert_eq!(m.empty_len, 4);
        assert!(m.empty.is_some());
        assert_eq!(m.len(), 1);

        m.shrink_to_fit();
        assert_eq!(m.empty_len, 0);
        assert!(m.empty.is_none());

        m.clear();
        assert!(m.is_empty());
        assert!(m.map.is_empty());
    }

    #[test]
    fn test_move_to_front() {
        type LHM = LinkedHashMap<i32, i32>;
        let mut m = LHM::new();
        assert_eq!(m.capacity(), 0);
        assert!(m.is_empty());
        assert!(m.map.is_empty());

        assert_eq!(m.push_front(1, 1), None);
        assert_eq!(m.push_front(2, 2), None);
        assert_eq!(m.push_front(3, 3), None);
        assert_eq!(m.push_front(4, 4), None);
        assert_eq!(m.push_front(5, 5), None);

        assert!(m.move_to_front(&1));
        assert_eq!(m.front(), Some((&1, &1)));
        assert_eq!(m.back(), Some((&2, &2)));

        assert!(m.move_to_front(&2));
        assert_eq!(m.front(), Some((&2, &2)));
        assert_eq!(m.back(), Some((&3, &3)));

        assert!(m.move_to_front(&3));
        assert_eq!(m.front(), Some((&3, &3)));
        assert_eq!(m.back(), Some((&4, &4)));

        assert!(m.move_to_front(&4));
        assert_eq!(m.front(), Some((&4, &4)));
        assert_eq!(m.back(), Some((&5, &5)));

        assert!(m.move_to_front(&5));
        assert_eq!(m.front(), Some((&5, &5)));
        assert_eq!(m.back(), Some((&1, &1)));
    }
}
