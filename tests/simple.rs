mod common;

use common::{AnySystem, Database, Log, Param, RealSystem};

fn sum_abc(db: &dyn Database, _: &(), _: &AnySystem<'_, ()>) -> i32 {
    db.a() + db.b() + db.c()
}

fn square_parametrized(db: &dyn Database, param: &Param, _: &AnySystem<'_, Param>) -> i32 {
    db.parametrized(*param) * db.parametrized(*param)
}

#[test]
fn simple() {
    let mut system = RealSystem::default();

    system.set_a(1);
    system.set_b(2);
    system.set_c(3);

    assert_eq!(*system.query((), sum_abc), 6);
}

#[test]
fn simple_parametrized() {
    let mut system = RealSystem::default();

    system.set_parametrized(Param::Foo, 3);

    assert_eq!(*system.query(Param::Foo, square_parametrized), 9);
}

#[test]
fn simple_caching() {
    let mut system = RealSystem::default();

    system.set_a(1);
    system.set_b(2);
    system.set_c(3);

    system.query((), sum_abc);
    system.query((), sum_abc);

    let log_book = system.log_book();
    assert!(log_book.iter().cloned().filter(Log::is_cache_hit).count() == 1);
    assert!(log_book.iter().cloned().filter(Log::is_query_start).count() == 1);
}

#[test]
fn simple_parametrized_caching() {
    let mut system = RealSystem::default();

    system.set_parametrized(Param::Foo, 3);
    system.set_parametrized(Param::Bar, 5);

    system.query(Param::Foo, square_parametrized);
    system.query(Param::Foo, square_parametrized);

    let log_book = system.log_book();
    assert!(log_book.iter().cloned().filter(Log::is_cache_hit).count() == 1);
    assert!(log_book.iter().cloned().filter(Log::is_query_start).count() == 1);

    system.query(Param::Bar, square_parametrized);

    let log_book = system.log_book();
    // Still just one.
    assert!(log_book.iter().cloned().filter(Log::is_cache_hit).count() == 1);
}

#[test]
fn simple_cache_invalidation() {
    let mut system = RealSystem::default();

    system.set_a(1);
    system.set_b(2);
    system.set_c(3);

    system.query((), sum_abc);
    system.set_b(6);
    let updated = *system.query((), sum_abc);

    assert_eq!(updated, 10);

    let log_book = system.log_book();
    assert!(log_book.iter().cloned().filter(Log::is_cache_hit).count() == 0);
    assert!(log_book.iter().cloned().filter(Log::is_query_start).count() == 2);
}

#[test]
fn simple_parametrized_cache_invalidation() {
    let mut system = RealSystem::default();

    system.set_parametrized(Param::Foo, 3);

    system.query(Param::Foo, square_parametrized);
    system.set_parametrized(Param::Foo, 5);
    let updated = *system.query(Param::Foo, square_parametrized);

    assert_eq!(updated, 25);

    let log_book = system.log_book();
    assert!(log_book.iter().cloned().filter(Log::is_cache_hit).count() == 0);
    assert!(log_book.iter().cloned().filter(Log::is_query_start).count() == 2);
}
