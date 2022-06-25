// Adapted almost 1:1 from
// https://github.com/salsa-rs/salsa/blob/cf22135ec272609924eddb60a5145721a3d54a54/examples/hello_world/main.rs.

use std::sync::Arc;

// Step 1. Define the database of inputs
//
// A **database** is a collection in *inputs* that can be used in queries. A
// database is represented by a trait decorated with the `#[inqui::database]`
// attribute. Each input is represented by its method, where the arguments of
// the method are **keys** and the return value is the **value** of the
// corresponding input. Both keys and value have some constraints, for example
// they need to be `'static`, implement `Clone` and keys must implement `Eq` and
// `Hash`.
//
// The attribute generates infrastructure code for the database, namely the
// database storage (name of the trait suffixed with `Storage`), which holds
// storages for all inputs of the database, and a new type for each input (name
// of the input field in pascal case suffixed with `Input`).

#[inqui::database]
trait HelloWorld {
    fn my_string(&self, key: ()) -> String;
    fn optional_string(&self, key: ()) -> Option<String>;
}

// Step 2. Implement the database trait
//
// There needs to be an implementation of the database trait (`HelloWorld` in
// this example), which will then be passed into queries so they can get the
// inputs. The implementation uses query context to access the inputs in the
// storage and track the query's dependencies.
//
// Arguably, this implementation should also be auto-generated via a procedural
// macro.

struct HelloWorldImpl<'r> {
    ctx: &'r inqui::QueryContext<'r, HelloWorldStorage>,
}

impl HelloWorld for HelloWorldImpl<'_> {
    fn my_string(&self, key: ()) -> String {
        // `use_input` retrieves the input value from the storage and adds it to
        // the query's dependencies. The method returns None if the input has
        // not been set or has been removed.
        self.ctx.use_input::<MyStringInput>(&key).unwrap()
    }

    fn optional_string(&self, key: ()) -> Option<String> {
        // Inputs with `Option<T>` return type are considered optional. The
        // corresponding input storage uses `T` as the value type, but the trait
        // uses `Option<T>` to be able to indicate missing input. Notice no
        // `unwrap` as opposed to the `my_string` input.
        self.ctx.use_input::<OptionalStringInput>(&key)
    }
}

// Step 3. Define your query system
//
// Use the building blocks to define your query system and its API according to
// your needs and opinions. The main tools provided by inqui are `Runtime`
// holding the inputs storage and `QueryCache` used clever caching for the
// queries.
//
// The `QueryCache` is parametrized over query **parameter**, which needs to be
// the same for *all* queries that your system will support. This is a
// limitation, but there is a range how flexible one can be, from using a single
// fixed type over a "compound" type with many `From<T>` implementations to
// perhaps a beast based on `TypedId`-based map.
//
// The query system is also responsible for managing the inputs depending on the
// application. It can either provide methods for setting the inputs, have a
// different way to communicate them or manage them somehow internally.

#[derive(Default)]
struct System {
    runtime: inqui::Runtime<HelloWorldStorage>,
    queries: inqui::QueryCache<()>,
}

impl System {
    // Providing a way how to set the inputs. In this example via public method.
    pub fn set_my_string(&mut self, value: impl Into<String>) {
        self.runtime.set_input::<MyStringInput>((), value.into());
    }

    // The main entrypoint for the queries. Inqui is not opinionated on what the
    // interface for the queries should be. In this example, we represent
    // queries as functions with an arbitrary return value. Another option is
    // defining a `Query` trait with `type Output` associated type and
    // `run_query` method. There are also variations of these approaches.
    pub fn query<F, R>(&self, f: F) -> Arc<R>
    where
        // The function takes reference to the inputs database and a parameter
        // (unit type in this example). It needs to be `'static` due to the
        // current limitations of
        // [`TypeId`](https://doc.rust-lang.org/nightly/std/any/struct.TypeId.html).
        F: FnOnce(&dyn HelloWorld, ()) -> R + 'static,
        // The return value needs to be `Send` and `Sync` to support paralel
        // queries. It also needs to be `'static` because it is stored in the
        // cache.
        R: Send + Sync + 'static,
    {
        // This is just to show how a param would be used in the query running
        // machinery.
        let param = ();

        self.queries
            // First, try if the query with the parameter is (valid) in the
            // cache. We need to specify the type for identifying the query
            // type.
            .cached::<F, _, _>(&param, &self.runtime)
            .unwrap_or_else(|| {
                // If we get a cache miss, we need to run the query and cache
                // it.
                self.queries
                    .insert_with::<F, _, _, _>(&self.runtime, param, |param, ctx| {
                        #[allow(clippy::unit_arg)]
                        // Run the query itself.
                        f(&HelloWorldImpl { ctx }, *param)
                    })
            })
    }
}

// Step 4. Define (or let others define) the queries
//
// The form of queries depends on the chosen implementation of the query system.
// This example uses functions that return an arbitrary value (that satisfy
// `QueryCache` constraints).
//
// The queries here can't call other queries, but that is just a limitation of
// the implementation in this example, not of inqui itself. To allow calling
// queries from other queries, the system would pass an additional argument on
// which one could call the `query` method.

fn length(db: &dyn HelloWorld, (): ()) -> usize {
    // Get the input.
    let my_string = db.my_string(());

    // Compute the output.
    my_string.len()
}

fn main() {
    let mut system = System::default();

    system.set_my_string("Hello world!");

    println!("Now, the length is {}.", system.query(length));
}
