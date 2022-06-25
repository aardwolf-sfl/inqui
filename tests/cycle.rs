use inqui::Cycle;

mod common;

use common::{AnySystem, Database, RealSystem, System};

fn foo(_: &dyn Database, n: &u32, system: &AnySystem<'_, u32>) -> Result<u32, Cycle> {
    let n = *n;

    if n > 1 {
        Ok(*system.query_or_cycle(n / 2, bar)?)
    } else {
        Ok(n)
    }
}

fn bar(_: &dyn Database, n: &u32, system: &AnySystem<'_, u32>) -> Result<u32, Cycle> {
    let n = *n;

    if n % 2 == 0 {
        Ok(*system.query_or_cycle(n, foo)?)
    } else {
        Ok(*system.query_or_cycle(n, baz)?)
    }
}

fn baz(_: &dyn Database, n: &u32, system: &AnySystem<'_, u32>) -> Result<u32, Cycle> {
    Ok(*system.query_or_cycle(*n + 1, bar)?)
}

#[test]
fn cycle1() {
    let system = RealSystem::default();

    let result = system.query_or_cycle(12, foo);
    assert!(result.is_err());

    let cycle = system.debug_cycle(result.unwrap_err()).to_strings();
    assert_eq!(
        cycle,
        &[
            "cycle::bar(2)",
            "cycle::foo(2)",
            "cycle::bar(1)",
            "cycle::baz(1)",
            "cycle::bar(2)"
        ]
    );
}
