use std::{sync::Arc, thread, time::Duration};

use inqui::{QueryCache, QueryContext, Runtime};

#[inqui::database]
pub trait Database {
    fn number(&self) -> i32;
}

#[derive(Clone)]
pub struct Calculations {
    runtime: Runtime<DatabaseStorage>,
    queries: Arc<QueryCache<()>>,
}

impl Calculations {
    pub fn new(initial: i32) -> Self {
        let mut this = Self {
            runtime: Runtime::new(),
            queries: Arc::new(QueryCache::new()),
        };

        this.set_number(initial);

        this
    }

    pub fn set_number(&mut self, value: i32) {
        eprintln!("before number = {} ({:?})", value, thread::current().id());
        self.runtime.set_input::<NumberInput>((), value);
        eprintln!("after number = {} ({:?})", value, thread::current().id());
    }

    pub fn calculate<F>(&self, f: F) -> i32
    where
        F: FnOnce(&dyn Database) -> i32 + 'static,
    {
        eprintln!("before calculate ({:?})", thread::current().id());
        let output = *self
            .queries
            .cached::<F, i32, _>(&(), &self.runtime)
            .unwrap_or_else(|| {
                // Enforce consistency of inputs. As long as the lock guard is
                // held, no input can be set or removed.
                let guard = self.runtime.lock_readonly();

                let output =
                    self.queries
                        .insert_with::<F, i32, _, _>(&self.runtime, (), |_, ctx| {
                            f(&DatabaseImpl { ctx })
                        });

                drop(guard);

                output
            });
        eprintln!("after calculate ({:?})", thread::current().id());

        output
    }
}

struct DatabaseImpl<'r> {
    ctx: &'r QueryContext<'r, DatabaseStorage>,
}

impl Database for DatabaseImpl<'_> {
    fn number(&self) -> i32 {
        self.ctx.use_input::<NumberInput>(&()).unwrap()
    }
}

fn fib_query(data: &dyn Database) -> i32 {
    fib(data.number())
}

fn fib(n: i32) -> i32 {
    if n == 0 || n == 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn main() {
    let mut calc = Calculations::new(45);

    let t1 = thread::spawn({
        let calc = calc.clone();
        move || {
            println!("fib = {}", calc.calculate(fib_query));
        }
    });

    let t2 = thread::spawn({
        let calc = calc.clone();
        move || {
            println!("fib = {}", calc.calculate(fib_query));
        }
    });

    thread::sleep(Duration::from_secs(1));

    calc.set_number(30);

    println!("fib = {}", calc.calculate(fib_query));

    t1.join().unwrap();
    t2.join().unwrap();
}
