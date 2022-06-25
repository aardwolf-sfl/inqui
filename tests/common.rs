#![allow(dead_code)]

use std::{any, collections::HashMap, fmt, hash::Hash, marker::PhantomData, sync::Arc};

use inqui::{query::CycleDebug, Cycle, QueryCache, QueryContext, Runtime};
use parking_lot::{Mutex, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Param {
    Foo,
    Bar,
    Baz,
    Qux,
}

#[inqui::database]
pub trait Database {
    fn a(&self) -> i32;
    fn b(&self) -> i32;
    fn c(&self) -> i32;
    fn parametrized(&self, param: Param) -> i32;
    fn optional(&self) -> Option<i32>;
}

pub trait System<P> {
    fn query<F, R>(&self, param: P, f: F) -> Arc<R>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> R + 'static,
        R: Send + Sync + 'static;

    fn query_or_cycle<F, R>(&self, param: P, f: F) -> Result<Arc<R>, Cycle>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, Cycle> + 'static,
        R: Send + Sync + 'static;

    fn try_query<F, R, E>(&self, param: P, f: F) -> Result<Arc<R>, E>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, E> + 'static,
        R: Send + Sync + 'static,
        E: From<Cycle>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputName {
    A,
    B,
    C,
    Parametrized,
    Optional,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Log {
    CacheHit(QueryName),
    CacheMiss(QueryName),
    QueryStart(QueryName),
    QueryDone(QueryName),
    SetInputBefore(InputName, Option<Param>),
    SetInputAfter(InputName, Option<Param>),
    GetInputBefore(InputName, Option<Param>),
    GetInputAfter(InputName, Option<Param>),
}

impl Log {
    pub fn is_cache_hit(&self) -> bool {
        matches!(self, Log::CacheHit(_))
    }

    pub fn is_cache_miss(&self) -> bool {
        matches!(self, Log::CacheMiss(_))
    }

    pub fn is_query_start(&self) -> bool {
        matches!(self, Log::QueryStart(_))
    }

    pub fn is_query_done(&self) -> bool {
        matches!(self, Log::QueryDone(_))
    }
}

#[derive(Clone)]
pub struct RealSystem<P> {
    runtime: Runtime<DatabaseStorage>,
    queries: Arc<QueryCache<P>>,
    use_lock: bool,
    log_book: Arc<Mutex<Vec<Log>>>,
}

impl<P> RealSystem<P> {
    pub fn new(use_lock: bool) -> Self {
        let mut this = Self {
            runtime: Default::default(),
            queries: Default::default(),
            use_lock,
            log_book: Default::default(),
        };

        this.set_a(0);
        this.set_b(0);
        this.set_c(0);
        this.set_parametrized(Param::Foo, 0);
        this.set_parametrized(Param::Bar, 0);
        this.set_parametrized(Param::Baz, 0);
        this.set_parametrized(Param::Qux, 0);
        // Do not set optional

        this
    }

    pub fn set_a(&mut self, value: i32) {
        self.log(Log::SetInputBefore(InputName::A, None));
        self.runtime.set_input::<AInput>((), value);
        self.log(Log::SetInputAfter(InputName::A, None));
    }

    pub fn set_b(&mut self, value: i32) {
        self.log(Log::SetInputBefore(InputName::B, None));
        self.runtime.set_input::<BInput>((), value);
        self.log(Log::SetInputAfter(InputName::B, None));
    }

    pub fn set_c(&mut self, value: i32) {
        self.log(Log::SetInputBefore(InputName::C, None));
        self.runtime.set_input::<CInput>((), value);
        self.log(Log::SetInputAfter(InputName::C, None));
    }

    pub fn set_parametrized(&mut self, param: Param, value: i32) {
        self.log(Log::SetInputBefore(InputName::Parametrized, Some(param)));
        self.runtime.set_input::<ParametrizedInput>(param, value);
        self.log(Log::SetInputAfter(InputName::Parametrized, Some(param)));
    }

    pub fn set_optional(&mut self, value: i32) {
        self.log(Log::SetInputBefore(InputName::Optional, None));
        self.runtime.set_input::<OptionalInput>((), value);
        self.log(Log::SetInputAfter(InputName::Optional, None));
    }

    pub fn log_book(&self) -> Vec<Log> {
        self.log_book.lock().clone()
    }

    fn log(&self, log: Log) {
        self.log_book.lock().push(log);
    }
}

impl<P: Clone + Eq + Hash> RealSystem<P> {
    pub fn query<F, R>(&self, param: P, f: F) -> Arc<R>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> R + 'static,
        R: Send + Sync + 'static,
        P: fmt::Debug,
    {
        self.try_query::<_, R, Cycle>(param, |db, param, system| Ok(f(db, param, system)))
            .unwrap()
    }

    pub fn query_or_cycle<F, R>(&self, param: P, f: F) -> Result<Arc<R>, Cycle>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, Cycle> + 'static,
        R: Send + Sync + 'static,
    {
        self.try_query::<_, R, Cycle>(param, f)
    }

    pub fn try_query<F, R, E>(&self, param: P, f: F) -> Result<Arc<R>, E>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, E> + 'static,
        R: Send + Sync + 'static,
        E: From<Cycle>,
    {
        let query_name = QueryName(any::type_name::<F>().to_string());

        self.queries
            .cached::<F, _, _>(&param, &self.runtime)
            // Option::inspect is unstable (https://github.com/rust-lang/rust/issues/91345)
            .map(|value| {
                self.log(Log::CacheHit(query_name.clone()));
                value
            })
            .map(Ok)
            .unwrap_or_else(|| {
                self.log(Log::CacheMiss(query_name.clone()));
                let guard = self.use_lock.then(|| self.runtime.lock_readonly());

                self.log(Log::QueryStart(query_name.clone()));
                let output = self.queries.try_insert_with::<F, _, E, _, _>(
                    &self.runtime,
                    param,
                    |param, ctx| {
                        let database = DatabaseImpl { ctx, system: self };
                        f(&database, param, &AnySystem::Real(self))
                    },
                );
                self.log(Log::QueryDone(query_name.clone()));

                drop(guard);

                output
            })
    }

    pub fn debug_cycle(&self, cycle: Cycle) -> CycleDebug<'_, P> {
        self.queries.debug_cycle(cycle)
    }
}

impl<P> Default for RealSystem<P> {
    fn default() -> Self {
        Self::new(true)
    }
}

impl<P: Clone + Eq + Hash + fmt::Debug> System<P> for RealSystem<P> {
    fn query<F, R>(&self, param: P, f: F) -> Arc<R>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> R + 'static,
        R: Send + Sync + 'static,
    {
        self.query(param, f)
    }

    fn query_or_cycle<F, R>(&self, param: P, f: F) -> Result<Arc<R>, Cycle>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, Cycle> + 'static,
        R: Send + Sync + 'static,
    {
        self.query_or_cycle(param, f)
    }

    fn try_query<F, R, E>(&self, param: P, f: F) -> Result<Arc<R>, E>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, E> + 'static,
        R: Send + Sync + 'static,
        E: From<Cycle>,
    {
        self.try_query(param, f)
    }
}

struct DatabaseImpl<'r, P> {
    ctx: &'r QueryContext<'r, DatabaseStorage>,
    system: &'r RealSystem<P>,
}

impl<P> Database for DatabaseImpl<'_, P> {
    fn a(&self) -> i32 {
        self.system.log(Log::GetInputBefore(InputName::A, None));
        let input = self.ctx.use_input::<AInput>(&()).unwrap();
        self.system.log(Log::GetInputAfter(InputName::A, None));
        input
    }

    fn b(&self) -> i32 {
        self.system.log(Log::GetInputBefore(InputName::B, None));
        let input = self.ctx.use_input::<BInput>(&()).unwrap();
        self.system.log(Log::GetInputAfter(InputName::B, None));
        input
    }

    fn c(&self) -> i32 {
        self.system.log(Log::GetInputBefore(InputName::C, None));
        let input = self.ctx.use_input::<CInput>(&()).unwrap();
        self.system.log(Log::GetInputAfter(InputName::C, None));
        input
    }

    fn parametrized(&self, param: Param) -> i32 {
        self.system
            .log(Log::GetInputBefore(InputName::Parametrized, Some(param)));
        let input = self.ctx.use_input::<ParametrizedInput>(&param).unwrap();
        self.system
            .log(Log::GetInputAfter(InputName::Parametrized, Some(param)));
        input
    }

    fn optional(&self) -> Option<i32> {
        self.system
            .log(Log::GetInputBefore(InputName::Optional, None));
        let input = self.ctx.use_input::<OptionalInput>(&());
        self.system
            .log(Log::GetInputAfter(InputName::Optional, None));
        input
    }
}

#[derive(Debug, Clone)]
pub struct SystemModel<P> {
    a: Arc<RwLock<i32>>,
    b: Arc<RwLock<i32>>,
    c: Arc<RwLock<i32>>,
    parametrized: Arc<RwLock<HashMap<Param, i32>>>,
    optional: Arc<RwLock<Option<i32>>>,
    phantom: PhantomData<P>,
}

impl<P> SystemModel<P> {
    pub fn new() -> Self {
        Self {
            a: Arc::new(RwLock::new(0)),
            b: Arc::new(RwLock::new(0)),
            c: Arc::new(RwLock::new(0)),
            parametrized: Arc::new(RwLock::new(
                [
                    (Param::Foo, 0),
                    (Param::Bar, 0),
                    (Param::Baz, 0),
                    (Param::Qux, 0),
                ]
                .into_iter()
                .collect(),
            )),
            optional: Arc::new(RwLock::new(None)),
            phantom: PhantomData,
        }
    }

    pub fn set_a(&mut self, value: i32) {
        *self.a.write() = value;
    }

    pub fn set_b(&mut self, value: i32) {
        *self.b.write() = value;
    }

    pub fn set_c(&mut self, value: i32) {
        *self.c.write() = value;
    }

    pub fn set_parametrized(&mut self, param: Param, value: i32) {
        self.parametrized.write().insert(param, value);
    }

    pub fn set_optional(&mut self, value: i32) {
        *self.optional.write() = Some(value);
    }

    pub fn query<F, R>(&self, param: P, f: F) -> Arc<R>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> R + 'static,
        R: Send + Sync + 'static,
    {
        Arc::new(f(self, &param, &AnySystem::Model(self)))
    }
}

impl<P> Default for SystemModel<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> Database for SystemModel<P> {
    fn a(&self) -> i32 {
        *self.a.read()
    }

    fn b(&self) -> i32 {
        *self.b.read()
    }

    fn c(&self) -> i32 {
        *self.c.read()
    }

    fn parametrized(&self, param: Param) -> i32 {
        self.parametrized.read().get(&param).copied().unwrap()
    }

    fn optional(&self) -> Option<i32> {
        *self.optional.read()
    }
}

impl<P> System<P> for SystemModel<P> {
    fn query<F, R>(&self, param: P, f: F) -> Arc<R>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> R + 'static,
        R: Send + Sync + 'static,
    {
        self.query(param, f)
    }

    fn query_or_cycle<F, R>(&self, _: P, _: F) -> Result<Arc<R>, Cycle>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, Cycle> + 'static,
        R: Send + Sync + 'static,
    {
        panic!("system model does not handle cycles");
    }

    fn try_query<F, R, E>(&self, _: P, _: F) -> Result<Arc<R>, E>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, E> + 'static,
        R: Send + Sync + 'static,
        E: From<Cycle>,
    {
        panic!("system model does not handle cycles");
    }
}

pub enum AnySystem<'a, P> {
    Real(&'a RealSystem<P>),
    Model(&'a SystemModel<P>),
}

impl<P: Clone + Eq + Hash + fmt::Debug> System<P> for AnySystem<'_, P> {
    fn query<F, R>(&self, param: P, f: F) -> Arc<R>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> R + 'static,
        R: Send + Sync + 'static,
    {
        match self {
            AnySystem::Real(system) => system.query(param, f),
            AnySystem::Model(system) => system.query(param, f),
        }
    }

    fn query_or_cycle<F, R>(&self, param: P, f: F) -> Result<Arc<R>, Cycle>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, Cycle> + 'static,
        R: Send + Sync + 'static,
    {
        match self {
            AnySystem::Real(system) => system.query_or_cycle(param, f),
            AnySystem::Model(system) => system.query_or_cycle(param, f),
        }
    }

    fn try_query<F, R, E>(&self, param: P, f: F) -> Result<Arc<R>, E>
    where
        F: FnOnce(&dyn Database, &P, &AnySystem<'_, P>) -> Result<R, E> + 'static,
        R: Send + Sync + 'static,
        E: From<Cycle>,
    {
        match self {
            AnySystem::Real(system) => system.try_query(param, f),
            AnySystem::Model(system) => system.try_query(param, f),
        }
    }
}
