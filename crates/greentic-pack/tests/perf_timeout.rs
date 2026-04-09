mod perf_support;

use std::sync::mpsc;
use std::time::Duration;

use greentic_pack::{pack_lock::validate_pack_lock, static_routes::validate_static_routes_payload};

#[test]
fn hot_paths_finish_within_timeout() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let lock = perf_support::build_pack_lock(256, 16);
        let routes = perf_support::build_static_routes(512);

        for _ in 0..25 {
            validate_pack_lock(&lock).expect("pack lock validation should pass");
            validate_static_routes_payload(&routes, |path: &str| {
                path.ends_with("index.html") || path.starts_with("assets/site-")
            })
            .expect("static route validation should pass");
        }

        tx.send(()).expect("signal completion");
    });

    rx.recv_timeout(Duration::from_secs(2))
        .expect("hot paths should complete well within the timeout");
}
