use std::{sync::mpsc, thread, time::Duration};

mod common;

use common::{AnySystem, Database, RealSystem};

fn longer(db: &dyn Database, wait: &bool, _: &AnySystem<'_, bool>) -> i32 {
    let a = db.a();
    if *wait {
        thread::sleep(Duration::from_millis(50));
    }
    let b = db.b();
    a + b
}

#[test]
fn consistency_with_locking() {
    let mut system = RealSystem::new(true);
    let (sender, receiver) = mpsc::sync_channel(1);

    system.set_a(3);
    system.set_b(5);

    let t1 = thread::spawn({
        let system = system.clone();
        move || {
            let output = *system.query(true, move |db, param, system| {
                sender.send(()).unwrap();
                longer(db, param, system)
            });
            assert_eq!(output, 8);
        }
    });

    receiver.recv().unwrap();

    system.set_b(10);
    assert_eq!(*system.query(false, longer), 13);

    t1.join().unwrap();
}

#[test]
fn inconsistency_without_locking() {
    let mut system = RealSystem::new(false);
    let (sender, receiver) = mpsc::sync_channel(1);

    system.set_a(3);
    system.set_b(5);

    let t1 = thread::spawn({
        let system = system.clone();
        move || {
            let output = *system.query(true, move |db, param, system| {
                sender.send(()).unwrap();
                longer(db, param, system)
            });
            // `b` already changed to 10!
            assert_eq!(output, 13);
        }
    });

    receiver.recv().unwrap();

    system.set_b(10);
    assert_eq!(*system.query(false, longer), 13);

    t1.join().unwrap();
}

#[test]
fn parallel_queries() {
    let mut system = RealSystem::new(true);

    system.set_a(3);
    system.set_b(5);

    let t1 = thread::spawn({
        let system = system.clone();
        move || {
            let output = *system.query(true, longer);
            assert_eq!(output, 8);
        }
    });

    let t2 = thread::spawn({
        let system = system.clone();
        move || {
            let output = *system.query(true, longer);
            assert_eq!(output, 8);
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();

    let log_book = system.log_book();

    let latest_start = log_book
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, log)| if log.is_query_start() { Some(i) } else { None })
        .unwrap();

    let earliest_done = log_book
        .iter()
        .enumerate()
        .find_map(|(i, log)| if log.is_query_done() { Some(i) } else { None })
        .unwrap();

    // Neither is finished before the other => they run in parallel.
    assert!(latest_start < earliest_done);
}
