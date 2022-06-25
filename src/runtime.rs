use std::sync::Arc;

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use crate::{
    input::{Input, InputIndex, InputStorage, KeyIndex},
    query_stack::QueryStack,
    revision::Revision,
};

#[derive(Default)]
pub struct Runtime<I> {
    shared: Arc<RwLock<SharedState<I>>>,
    query_stack: QueryStack,
    query_lock: Arc<RwLock<()>>,
}

impl<I: Default> Runtime<I> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<I> Runtime<I> {
    pub fn get_input<T>(&self, key: &T::Key) -> Option<T::Value>
    where
        T: Input<StorageGroup = I>,
    {
        self.with_storage::<T, _, _>(|storage| storage.get(key).map(|(value, _)| value))
    }

    pub fn set_input<T>(&mut self, key: T::Key, value: T::Value)
    where
        T: Input<StorageGroup = I>,
    {
        let guard = self.query_lock.write();
        let mut shared = self.shared.write();

        let key_index = T::storage_mut(&mut shared.inputs).set(key, value);

        shared.rev.increment();
        let rev = shared.rev;

        shared
            .input_revs
            .insert((InputIndex(T::INDEX), key_index), rev);

        drop(guard);
    }

    pub fn remove_input<T>(&mut self, key: &T::Key)
    where
        T: Input<StorageGroup = I>,
    {
        let guard = self.query_lock.write();
        let mut shared = self.shared.write();

        if let Some((_, key_index)) = T::storage_mut(&mut shared.inputs).remove(key) {
            shared.rev.increment();
            let rev = shared.rev;

            shared
                .input_revs
                .insert((InputIndex(T::INDEX), key_index), rev);
        }

        drop(guard);
    }

    pub(crate) fn with_storage<T, F, R>(&self, f: F) -> R
    where
        T: Input<StorageGroup = I>,
        F: FnOnce(&InputStorage<T>) -> R,
    {
        f(T::storage(&self.shared.read().inputs))
    }

    pub(crate) fn rev(&self) -> Revision {
        self.shared.read().rev
    }

    pub(crate) fn last_rev_of(&self, dependencies: &[(InputIndex, KeyIndex)]) -> Revision {
        let shared = self.shared.read();
        dependencies
            .iter()
            .map(|index| shared.input_revs[index])
            .max()
            .unwrap_or_default()
    }

    pub(crate) fn query_stack(&self) -> &QueryStack {
        &self.query_stack
    }

    pub fn lock_readonly(&self) -> ReadOnlyGuard<'_> {
        ReadOnlyGuard(self.query_lock.read())
    }
}

impl<I> Clone for Runtime<I> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            // Query stack is local to every thread.
            query_stack: Default::default(),
            query_lock: self.query_lock.clone(),
        }
    }
}

#[derive(Default)]
struct SharedState<I> {
    rev: Revision,
    inputs: I,
    input_revs: FxHashMap<(InputIndex, KeyIndex), Revision>,
}

pub struct ReadOnlyGuard<'a>(parking_lot::RwLockReadGuard<'a, ()>);
