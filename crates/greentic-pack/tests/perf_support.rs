use std::collections::BTreeMap;

use greentic_pack::{
    pack_lock::{LockedComponent, LockedOperation, PackLockV1},
    static_routes::{
        StaticRouteCacheV1, StaticRouteScopeV1, StaticRouteV1, StaticRoutesExtensionV1,
    },
};

pub fn build_pack_lock(components: usize, operations_per_component: usize) -> PackLockV1 {
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

pub fn build_static_routes(routes: usize) -> StaticRoutesExtensionV1 {
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
