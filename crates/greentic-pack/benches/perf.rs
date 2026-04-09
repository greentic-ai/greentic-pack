use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use greentic_pack::{
    pack_lock::{LockedComponent, LockedOperation, PackLockV1, validate_pack_lock},
    static_routes::{
        StaticRouteCacheV1, StaticRouteScopeV1, StaticRouteV1, StaticRoutesExtensionV1,
        validate_static_routes_payload,
    },
};

fn build_pack_lock(components: usize, operations_per_component: usize) -> PackLockV1 {
    let mut entries = BTreeMap::new();
    for component_idx in 0..components {
        let component_id = format!("component-{component_idx:04}");
        let operations = (0..operations_per_component)
            .map(|op_idx| LockedOperation {
                operation_id: format!("op-{op_idx:03}"),
                schema_hash: format!("{:064x}", op_idx + component_idx),
            })
            .collect();
        entries.insert(
            component_id.clone(),
            LockedComponent {
                component_id,
                r#ref: Some(format!(
                    "oci://registry.greentic.ai/demo/component-{component_idx}@sha256:{:064x}",
                    component_idx
                )),
                abi_version: "0.6.0".to_string(),
                resolved_digest: format!("sha256:{:064x}", component_idx + 1),
                describe_hash: format!("{:064x}", component_idx + 2),
                operations,
                world: Some("greentic:component/demo@0.1.0".to_string()),
                component_version: Some("1.2.3".to_string()),
                role: Some("runtime".to_string()),
            },
        );
    }
    PackLockV1::new(entries)
}

fn build_static_routes(routes: usize) -> StaticRoutesExtensionV1 {
    StaticRoutesExtensionV1 {
        version: 1,
        routes: (0..routes)
            .map(|idx| StaticRouteV1 {
                id: format!("route-{idx:04}"),
                public_path: format!("/v1/web/app-{idx:04}"),
                source_root: format!("assets/site-{idx:04}"),
                scope: StaticRouteScopeV1 {
                    tenant: true,
                    team: idx % 2 == 0,
                },
                index_file: Some("index.html".to_string()),
                spa_fallback: Some("index.html".to_string()),
                cache: Some(StaticRouteCacheV1 {
                    strategy: "public-max-age".to_string(),
                    max_age_seconds: Some(3600),
                }),
                exports: BTreeMap::from([(format!("asset-{idx:04}"), format!("export-{idx:04}"))]),
            })
            .collect(),
    }
}

fn legacy_operations_sorted(component: &LockedComponent) -> bool {
    let mut sorted = component.operations.clone();
    sorted.sort_by(|a, b| a.operation_id.cmp(&b.operation_id));
    component.operations == sorted
}

fn current_operations_sorted(component: &LockedComponent) -> bool {
    !component
        .operations
        .windows(2)
        .any(|pair| pair[0].operation_id > pair[1].operation_id)
}

fn legacy_collect_route_uniques(payload: &StaticRoutesExtensionV1) -> usize {
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut seen_exports = std::collections::BTreeSet::new();
    for route in &payload.routes {
        seen_ids.insert(route.id.clone());
        for export_name in route.exports.values() {
            seen_exports.insert(export_name.clone());
        }
    }
    seen_ids.len() + seen_exports.len()
}

fn current_collect_route_uniques(payload: &StaticRoutesExtensionV1) -> usize {
    let mut seen_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut seen_exports: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for route in &payload.routes {
        seen_ids.insert(route.id.as_str());
        for export_name in route.exports.values() {
            seen_exports.insert(export_name.as_str());
        }
    }
    seen_ids.len() + seen_exports.len()
}

fn bench_validate_pack_lock(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_pack_lock");
    for (components, ops) in [(32usize, 8usize), (128, 8), (256, 16)] {
        let lock = build_pack_lock(components, ops);
        group.throughput(Throughput::Elements(components as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{components}x{ops}")),
            &lock,
            |b, lock| b.iter(|| validate_pack_lock(black_box(lock)).expect("valid lock")),
        );
    }
    group.finish();
}

fn bench_sorted_operation_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("pack_lock_operation_sorted_check");
    let lock = build_pack_lock(256, 16);
    let component = lock
        .components
        .values()
        .next()
        .expect("benchmark component exists");

    group.bench_function("legacy_clone_and_sort", |b| {
        b.iter(|| black_box(legacy_operations_sorted(black_box(component))))
    });
    group.bench_function("current_window_scan", |b| {
        b.iter(|| black_box(current_operations_sorted(black_box(component))))
    });
    group.finish();
}

fn bench_validate_static_routes_payload(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate_static_routes_payload");
    for routes in [64usize, 256usize, 512usize] {
        let payload = build_static_routes(routes);
        group.throughput(Throughput::Elements(routes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(routes),
            &payload,
            |b, payload| {
                b.iter(|| {
                    validate_static_routes_payload(black_box(payload), |path: &str| {
                        path.ends_with("index.html") || path.starts_with("assets/site-")
                    })
                    .expect("valid routes")
                })
            },
        );
    }
    group.finish();
}

fn bench_static_route_uniques(c: &mut Criterion) {
    let mut group = c.benchmark_group("static_route_unique_tracking");
    let payload = build_static_routes(512);

    group.bench_function("legacy_clone_strings", |b| {
        b.iter(|| black_box(legacy_collect_route_uniques(black_box(&payload))))
    });
    group.bench_function("current_borrowed_sets", |b| {
        b.iter(|| black_box(current_collect_route_uniques(black_box(&payload))))
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_validate_pack_lock,
    bench_sorted_operation_check,
    bench_validate_static_routes_payload,
    bench_static_route_uniques
);
criterion_main!(benches);
