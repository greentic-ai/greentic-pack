mod perf_support;

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use greentic_pack::{pack_lock::validate_pack_lock, static_routes::validate_static_routes_payload};

fn run_scaling_workload(threads: usize, iterations_per_thread: usize) -> Duration {
    let lock = Arc::new(perf_support::build_pack_lock(128, 12));
    let routes = Arc::new(perf_support::build_static_routes(256));
    let start = Instant::now();

    thread::scope(|scope| {
        for _ in 0..threads {
            let lock = Arc::clone(&lock);
            let routes = Arc::clone(&routes);
            scope.spawn(move || {
                for _ in 0..iterations_per_thread {
                    validate_pack_lock(&lock).expect("pack lock validation should pass");
                    validate_static_routes_payload(&routes, |path: &str| {
                        path.ends_with("index.html") || path.starts_with("assets/site-")
                    })
                    .expect("static route validation should pass");
                }
            });
        }
    });

    start.elapsed()
}

#[test]
fn scaling_should_not_degrade_badly() {
    let t1 = run_scaling_workload(1, 40);
    let t4 = run_scaling_workload(4, 40);

    let per_iter_1 = t1.as_secs_f64();
    let per_iter_4 = t4.as_secs_f64() / 4.0;

    assert!(
        per_iter_4 <= per_iter_1 * 2.2,
        "4-thread per-iteration cost regressed too much: t1={t1:?}, t4={t4:?}, per_iter_1={per_iter_1:.6}, per_iter_4={per_iter_4:.6}",
    );
}
