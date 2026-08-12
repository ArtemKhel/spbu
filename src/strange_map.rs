use std::{borrow::Borrow, collections::HashMap, hash::Hash};

/// Strange append only HashMap that stores entries as:
/// - single kv pair if only one was added
/// - array of [`ARRAY_THRESHOLD`] kv pairs if 2..=`ARRAY_THRESHOLD` was added
/// - [`std::collections::HashMap`] if >`ARRAY_THRESHOLD` was added
#[derive(Debug)]
pub struct StrangeMap<K, V> {
    len: usize,
    storage: Storage<K, V>,
}

const ARRAY_THRESHOLD: usize = 5;
const _: () = {
    assert!(ARRAY_THRESHOLD >= 2);
};

#[derive(Debug)]
enum Storage<K, V> {
    Empty,
    Single(Entry<K, V>),
    Array([Option<Entry<K, V>>; ARRAY_THRESHOLD]),
    HashMap(HashMap<K, V>),
}

#[derive(Debug)]
struct Entry<K, V> {
    key: K,
    value: V,
}

impl<K, V> Default for StrangeMap<K, V> {
    fn default() -> Self { Self::new() }
}

impl<K, V> StrangeMap<K, V> {
    /// Creates a new empty [`StrangeMap`]
    pub fn new() -> Self {
        Self {
            len: 0,
            storage: Storage::Empty,
        }
    }

    /// Number of elements in [`StrangeMap`]
    pub fn len(&self) -> usize { self.len }

    pub fn is_empty(&self) -> bool { self.len == 0 }
}

impl<K, V> StrangeMap<K, V>
where K: Eq + Hash
{
    /// Inserts a value, on collision replaces and returns the old one
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let storage = std::mem::replace(&mut self.storage, Storage::Empty);

        match storage {
            Storage::Empty => {
                self.len = 1;
                self.storage = Storage::Single(Entry { key, value });
                None
            }
            Storage::Single(mut entry) if entry.key == key => {
                let prev = std::mem::replace(&mut entry.value, value);
                self.storage = Storage::Single(entry);
                Some(prev)
            }
            Storage::Single(entry) => {
                self.len = 2;
                let mut entries = std::array::from_fn(|_| None);
                entries[0] = Some(entry);
                entries[1] = Some(Entry { key, value });
                self.storage = Storage::Array(entries);
                None
            }
            Storage::Array(mut entries) => {
                // if key already in map, replace it
                if let Some(entry) = entries.iter_mut().flatten().find(|entry| entry.key == key) {
                    let prev = std::mem::replace(&mut entry.value, value);
                    self.storage = Storage::Array(entries);
                    return Some(prev);
                }

                if let Some(slot) = entries.iter_mut().find(|slot| slot.is_none()) {
                    *slot = Some(Entry { key, value });
                    self.len += 1;
                    self.storage = Storage::Array(entries);
                    return None;
                }

                let mut map = HashMap::with_capacity(ARRAY_THRESHOLD + 1);
                for entry in entries.into_iter().flatten() {
                    map.insert(entry.key, entry.value);
                }
                map.insert(key, value);
                self.len += 1;
                self.storage = Storage::HashMap(map);
                None
            }
            Storage::HashMap(mut map) => {
                let previous = map.insert(key, value);
                if previous.is_none() {
                    self.len += 1;
                }
                self.storage = Storage::HashMap(map);
                previous
            }
        }
    }

    /// Checks if the map contains the `key`
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        Q: Eq + Hash + ?Sized, // can hash and compare keys, allow types with unknown at comptime size (str, [T])
        K: Borrow<Q>,          // map's key type K can be viewed as &Q (allow lookup of String keys with &str)
    {
        match &self.storage {
            Storage::Empty => false,
            Storage::Single(entry) => entry.key.borrow() == key,
            Storage::Array(entries) => entries.iter().flatten().any(|entry| entry.key.borrow() == key),
            Storage::HashMap(map) => map.contains_key(key),
        }
    }

    /// Returns `Some(value)` for `key` if `key` is found, `None` otherwise
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        Q: Eq + Hash + ?Sized,
        K: Borrow<Q>, {
        match &self.storage {
            Storage::Empty => None,
            Storage::Single(entry) if entry.key.borrow() == key => Some(&entry.value),
            Storage::Single(_) => None,
            Storage::Array(entries) => entries
                .iter()
                .flatten()
                .find(|entry| entry.key.borrow() == key)
                .map(|entry| &entry.value),
            Storage::HashMap(map) => map.get(key),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    #[test]
    fn correct_repr() {
        let mut map = StrangeMap::new();
        assert_matches!(map.storage, Storage::Empty);

        map.insert(0, 0);
        assert_matches!(map.storage, Storage::Single(_));

        for key in 1..ARRAY_THRESHOLD {
            map.insert(key, key);
        }
        assert_matches!(map.storage, Storage::Array(_));

        map.insert(ARRAY_THRESHOLD, ARRAY_THRESHOLD);
        assert_matches!(map.storage, Storage::HashMap(_));
        assert_eq!(map.len(), ARRAY_THRESHOLD + 1);
    }

    #[test]
    fn insert() {
        let mut map = StrangeMap::new();
        assert_eq!(map.insert("language", "Rust"), None);
        assert_eq!(map.insert("language", "Zig"), Some("Rust"));

        assert_eq!(map.len(), 1);
        assert_eq!(map.get("language"), Some(&"Zig"));
    }

    #[test]
    fn missing() {
        let map = StrangeMap::<i32, i32>::new();
        assert_eq!(map.get(&10), None);
        assert!(!map.contains_key(&10));
    }

    #[test]
    fn keeps_values() {
        let mut map = StrangeMap::new();

        for number in 0..ARRAY_THRESHOLD + 1 {
            map.insert(number, number * number);
        }

        assert_matches!(map.storage, Storage::HashMap(_));
        assert_eq!(map.len(), ARRAY_THRESHOLD + 1);

        for n in 0..ARRAY_THRESHOLD + 1 {
            assert!(map.contains_key(&n));
            let val = map.get(&n).unwrap();
            let expected = n * n;
            assert_eq!(*val, expected);
        }

        assert!(ARRAY_THRESHOLD < 100500);
        assert_eq!(map.get(&100500), None);
    }

    #[test]
    fn accepts_borrowed_keys_for_owned_strings() {
        let mut map = StrangeMap::new();
        map.insert(String::from("course"), 42);

        assert!(map.contains_key("course"));
        assert_eq!(map.get("course"), Some(&42));
    }
}
