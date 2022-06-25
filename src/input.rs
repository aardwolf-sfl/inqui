use std::hash::Hash;

use rustc_hash::FxHashMap;

pub trait Input {
    type Key: Hash + Eq;
    type Value: Clone;
    type StorageGroup;

    const INDEX: u16;

    fn storage(group: &Self::StorageGroup) -> &InputStorage<Self>;
    fn storage_mut(group: &mut Self::StorageGroup) -> &mut InputStorage<Self>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputIndex(pub(crate) u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyIndex(pub(crate) u32);

#[derive(Debug)]
pub struct InputStorage<T: Input + ?Sized> {
    index_map: FxHashMap<T::Key, KeyIndex>,
    value_map: FxHashMap<KeyIndex, T::Value>,
    key_index: u32,
}

impl<T: Input + ?Sized> InputStorage<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &T::Key) -> Option<(T::Value, KeyIndex)> {
        self.index_map.get(key).map(|index| {
            let value = self.value_map.get(index).unwrap().clone();
            (value, *index)
        })
    }

    pub fn set(&mut self, key: T::Key, value: T::Value) -> KeyIndex {
        let new_index = KeyIndex(self.key_index);
        self.key_index += 1;
        let index = *self.index_map.entry(key).or_insert(new_index);
        self.value_map.insert(index, value);
        index
    }

    pub fn remove(&mut self, key: &T::Key) -> Option<(T::Value, KeyIndex)> {
        self.index_map.remove(key).map(|index| {
            let value = self.value_map.remove(&index).unwrap();
            (value, index)
        })
    }
}

impl<T: Input + ?Sized> Default for InputStorage<T> {
    fn default() -> Self {
        Self {
            index_map: Default::default(),
            value_map: Default::default(),
            key_index: 0,
        }
    }
}
