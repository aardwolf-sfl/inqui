pub type FxDashMap<K, V> =
    dashmap::DashMap<K, V, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>;
pub type FxDashSet<T> = dashmap::DashSet<T, std::hash::BuildHasherDefault<rustc_hash::FxHasher>>;
