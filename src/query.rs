use core::fmt;
use std::{
    any::{Any, TypeId},
    hash::Hash,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use rustc_hash::FxHashMap;

use crate::{
    hash::{FxDashMap, FxDashSet},
    input::{Input, InputIndex, KeyIndex},
    revision::Revision,
    runtime::Runtime,
    Cycle,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryId(pub(crate) u32);

pub struct QueryCache<K> {
    id_map: FxDashMap<QueryType, FxHashMap<K, QueryId>>,
    query_map: FxDashMap<QueryId, QueryData>,
    query_id: AtomicU32,
}

struct QueryData {
    output: Arc<dyn Any + Send + Sync>,
    valid_at: Revision,
    dependencies: Vec<(InputIndex, KeyIndex)>,
}

#[derive(Debug, Clone, Copy)]
struct QueryType {
    type_id: TypeId,
    // For debugging purposes.
    name: &'static str,
}

impl<K> QueryCache<K> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K: Hash + Eq + Clone> QueryCache<K> {
    pub fn cached<Q: 'static, O: Send + Sync + 'static, I>(
        &self,
        param: &K,
        runtime: &Runtime<I>,
    ) -> Option<Arc<O>> {
        self.id_map
            .get(&QueryType::of::<Q>())
            .and_then(|map| map.value().get(param).copied())
            .and_then(|id| {
                // The situation when id_map contains the query type and param,
                // but query_map does not contain corresponding value, happens
                // when we have started a query, but not finished it yet, and we
                // are called again.
                let data = self.query_map.get(&id)?;
                let last_rev = runtime.last_rev_of(&data.dependencies);

                if last_rev <= data.valid_at {
                    Some(Arc::downcast(data.output.clone()).unwrap())
                } else {
                    None
                }
            })
    }

    pub fn insert_with<'r, Q: 'static, O: Send + Sync + 'static, I, F>(
        &self,
        runtime: &'r Runtime<I>,
        param: K,
        f: F,
    ) -> Arc<O>
    where
        F: FnOnce(&K, &QueryContext<'r, I>) -> O,
        K: fmt::Debug,
    {
        self.try_insert_with::<Q, _, Cycle, _, _>(runtime, param, |param, ctx| Ok(f(param, ctx)))
            .unwrap_or_else(|cycle| panic!("{:?}", self.debug_cycle(cycle)))
    }

    pub fn try_insert_with<'r, Q: 'static, O: Send + Sync + 'static, E, I, F>(
        &self,
        runtime: &'r Runtime<I>,
        param: K,
        f: F,
    ) -> Result<Arc<O>, E>
    where
        F: FnOnce(&K, &QueryContext<'r, I>) -> Result<O, E>,
        E: From<Cycle>,
    {
        let query_id = *self
            .id_map
            .entry(QueryType::of::<Q>())
            .or_default()
            .entry(param.clone())
            .or_insert_with(|| QueryId(self.query_id.fetch_add(1, Ordering::SeqCst)));

        let guard = runtime.query_stack().push(query_id)?;

        let ctx = QueryContext::new(runtime);
        let output = Arc::new(f(&param, &ctx)?);
        let valid_at = runtime.rev();
        let dependencies = ctx.into_dependencies();

        drop(guard);

        self.query_map.insert(
            query_id,
            QueryData {
                output: output.clone(),
                valid_at,
                dependencies,
            },
        );

        Ok(output)
    }

    pub fn id<Q: 'static>(&self, param: &K) -> Option<QueryId> {
        self.id_map
            .get(&QueryType::of::<Q>())
            .and_then(|map| map.get(param).copied())
    }

    pub fn debug_cycle(&self, cycle: Cycle) -> CycleDebug<'_, K> {
        CycleDebug { cache: self, cycle }
    }
}

impl<K> Default for QueryCache<K> {
    fn default() -> Self {
        Self {
            id_map: Default::default(),
            query_map: Default::default(),
            query_id: Default::default(),
        }
    }
}

pub struct QueryContext<'r, I> {
    dependencies: FxDashSet<(InputIndex, KeyIndex)>,
    runtime: &'r Runtime<I>,
}

impl<'r, I> QueryContext<'r, I> {
    fn new(runtime: &'r Runtime<I>) -> Self {
        Self {
            dependencies: Default::default(),
            runtime,
        }
    }

    pub fn use_input<T>(&self, key: &T::Key) -> Option<T::Value>
    where
        T: Input<StorageGroup = I>,
    {
        let (value, key_index) = self
            .runtime
            .with_storage::<T, _, _>(|storage| storage.get(key))?;
        self.dependencies.insert((InputIndex(T::INDEX), key_index));
        Some(value)
    }

    fn into_dependencies(self) -> Vec<(InputIndex, KeyIndex)> {
        self.dependencies.into_iter().collect()
    }
}

pub struct CycleDebug<'a, K> {
    cache: &'a QueryCache<K>,
    cycle: Cycle,
}

impl<K: fmt::Debug> CycleDebug<'_, K> {
    pub fn to_strings(&self) -> Vec<String> {
        self.cycle
            .cycle()
            .iter()
            .fold(Vec::new(), |mut all, query_id| {
                self.cache.id_map.iter().for_each(|kv| {
                    let ty = *kv.key();
                    let iter = kv.iter().filter_map(move |(param, id)| {
                        if id == query_id {
                            Some(format!("{}({:?})", ty.name(), param))
                        } else {
                            None
                        }
                    });
                    all.extend(iter);
                });

                all
            })
    }
}

impl<K: fmt::Debug> fmt::Debug for CycleDebug<'_, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cycle {{ cycle: [ ")?;

        let mut iter = self.to_strings().into_iter();

        if let Some(entry) = iter.next() {
            write!(f, "{}", entry)?;
        }

        for entry in iter {
            write!(f, ", {}", entry)?;
        }

        write!(f, " ] }}")
    }
}

impl QueryType {
    pub fn of<Q: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<Q>(),
            name: std::any::type_name::<Q>(),
        }
    }

    pub fn name(&self) -> &str {
        self.name
    }
}

impl PartialEq for QueryType {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id
    }
}

impl Eq for QueryType {}

impl Hash for QueryType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
    }
}
