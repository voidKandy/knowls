use core::fmt;
use std::{
    borrow::Borrow,
    collections::HashMap,
    hash::Hash,
    sync::{Arc, RwLock},
};

#[derive(Debug)]
pub struct LRUNode<T>
where
    T: Clone + Hash + Eq + PartialEq,
{
    val: T,
    next: Option<RefCountedLRUNode<T>>,
    prev: Option<RefCountedLRUNode<T>>,
}

type RefCountedLRUNode<T> = Arc<RwLock<LRUNode<T>>>;

#[derive(Debug)]
pub struct LRUCache<K, T>
where
    K: fmt::Debug + Clone + Hash + Eq + PartialEq,
    T: fmt::Debug + Clone + Hash + Eq + PartialEq,
{
    head: Option<RefCountedLRUNode<T>>,
    tail: Option<RefCountedLRUNode<T>>,
    lookup: HashMap<K, RefCountedLRUNode<T>>,
    reverse_lookup: HashMap<T, K>,
    length: usize,
    capacity: usize,
}

impl<T> LRUNode<T>
where
    T: Clone + Hash + Eq + PartialEq,
{
    fn new(val: T) -> Self {
        Self {
            val,
            next: None,
            prev: None,
        }
    }
}

impl<'a, K, T> IntoIterator for &'a LRUCache<K, T>
where
    K: fmt::Debug + Clone + Hash + Eq + PartialEq,
    T: fmt::Debug + Clone + Hash + Eq + PartialEq,
{
    type Item = (&'a K, T);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let keys: Vec<&K> = self.lookup.keys().collect();
        let vals: Vec<T> = self
            .lookup
            .values()
            // I don't love this clone...
            .map(|v| v.read().unwrap().val.clone())
            .collect();
        let iter: Vec<(&'a K, T)> = keys.into_iter().zip(vals.into_iter()).collect();
        iter.into_iter()
    }
}

impl<K, T> LRUCache<K, T>
where
    K: fmt::Debug + Clone + Hash + Eq + PartialEq,
    T: fmt::Debug + Clone + Hash + Eq + PartialEq,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            head: None,
            tail: None,
            lookup: HashMap::new(),
            reverse_lookup: HashMap::new(),
            length: 0,
            capacity,
        }
    }

    pub fn update(&mut self, key: K, value: T) {
        match self.lookup.get_mut(&key).and_then(|n| Some(Arc::clone(&n))) {
            None => {
                let node = Arc::new(RwLock::new(LRUNode::new(value.clone())));
                self.length += 1;
                self.prepend(&node);

                self.lookup.insert(key.clone(), node);
                self.reverse_lookup.insert(value, key);
                // self.trim_cache();
            }
            Some(node) => {
                self.detach(&node);
                self.prepend(&node);
                node.write().expect("Failed to get write lock").val = value;
            }
        }
    }

    /// reads the value at the given key without affecting the LRU
    pub fn read(&self, key: &K) -> Option<T> {
        let node = self.lookup.get(key).and_then(|n| Some(Arc::clone(&n)))?;
        let node_borrow = node.read().expect("Failed to get write lock");
        Some(node_borrow.val.clone())
    }

    pub fn read_all(&self) -> Vec<(&K, &T)> {
        let mut all = vec![];
        for (v, k) in self.reverse_lookup.iter() {
            all.push((k, v));
        }
        all
    }

    pub fn get(&mut self, key: &K) -> Option<T> {
        let node = self.lookup.get(key).and_then(|n| Some(Arc::clone(&n)))?;

        self.detach(&node);
        self.prepend(&node);
        let node_borrow = node.write().expect("Failed to get write lock");

        Some(node_borrow.val.clone())
    }

    pub fn at_capacity(&self) -> bool {
        self.length >= self.capacity
    }

    fn detach(&mut self, node: &RefCountedLRUNode<T>) {
        let mut borrow = node.write().expect("Failed to get write lock");

        if let Some(prev) = borrow.prev.as_ref().and_then(|n| Some(Arc::clone(&n))) {
            let mut prev_borrow = prev.write().expect("Failed to get write lock");
            prev_borrow.next = borrow.next.take();
        }

        if let Some(next) = borrow.next.as_ref().and_then(|n| Some(Arc::clone(&n))) {
            let mut next_borrow = next.write().expect("Failed to get write lock");
            next_borrow.prev = borrow.prev.take();
        }

        if let Some(h) = &self.head {
            // Can't borrow if node is head
            if !h.try_read().is_ok() {
                drop(borrow);
                let next = h
                    .read()
                    .expect("Failed to get read lock")
                    .next
                    .as_ref()
                    .and_then(|n| Some(Arc::clone(&n)));
                self.head = next;
                borrow = node.write().expect("Failed to get write lock");
            }
        }

        if let Some(t) = &self.tail {
            // Can't borrow if node is tail
            if !t.try_read().is_ok() {
                drop(borrow);
                let prev = t
                    .read()
                    .expect("Failed to get read lock")
                    .prev
                    .as_ref()
                    .and_then(|n| Some(Arc::clone(&n)));
                self.tail = prev;
                borrow = node.write().expect("Failed to get write lock");
            }
        }

        borrow.next = None;
        borrow.prev = None;
    }

    fn prepend(&mut self, node: &RefCountedLRUNode<T>) {
        if self.head.is_none() {
            self.head = Some(Arc::clone(node));
            self.tail = Some(Arc::clone(node));
            return;
        }

        let head = self
            .head
            .as_ref()
            .and_then(|n| Some(Arc::clone(&n)))
            .unwrap();

        node.write().expect("Failed to get write lock").next = Some(Arc::clone(&head));
        head.write().expect("Failed to get write lock").prev = Some(Arc::clone(&node));
        self.head = Some(Arc::clone(node));
    }

    fn trim_cache(&mut self) {
        if !self.at_capacity() {
            return;
        }

        let tail = self
            .tail
            .as_ref()
            .and_then(|n| Some(Arc::clone(&n)))
            .unwrap();

        self.detach(&tail);

        let val = &tail.read().expect("Failed to get read lock").val;
        let key = self
            .reverse_lookup
            .get(&val)
            .expect("No tail in reverse lookup");
        self.lookup.remove(key);
        self.reverse_lookup.remove(&val);
    }
}
