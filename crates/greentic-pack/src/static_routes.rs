use anyhow::{Result, bail};
use greentic_types::pack_manifest::{ExtensionInline, ExtensionRef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const STATIC_ROUTES_EXTENSION_KEY: &str = "greentic.static-routes.v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticRoutesExtensionV1 {
    pub version: u64,
    #[serde(default)]
    pub routes: Vec<StaticRouteV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticRouteV1 {
    pub id: String,
    pub public_path: String,
    pub source_root: String,
    #[serde(default)]
    pub scope: StaticRouteScopeV1,
    #[serde(default)]
    pub index_file: Option<String>,
    #[serde(default)]
    pub spa_fallback: Option<String>,
    #[serde(default)]
    pub cache: Option<StaticRouteCacheV1>,
    #[serde(default)]
    pub exports: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticRouteScopeV1 {
    #[serde(default)]
    pub tenant: bool,
    #[serde(default)]
    pub team: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StaticRouteCacheV1 {
    pub strategy: String,
    #[serde(default)]
    pub max_age_seconds: Option<u64>,
}

pub fn parse_static_routes_extension(
    extensions: &Option<BTreeMap<String, ExtensionRef>>,
) -> Result<Option<StaticRoutesExtensionV1>> {
    let Some(ext) = extensions
        .as_ref()
        .and_then(|all| all.get(STATIC_ROUTES_EXTENSION_KEY))
    else {
        return Ok(None);
    };

    let inline = ext.inline.as_ref().ok_or_else(|| {
        anyhow::anyhow!("extensions[{STATIC_ROUTES_EXTENSION_KEY}] inline is required")
    })?;

    let value = match inline {
        ExtensionInline::Other(value) => value.clone(),
        other => serde_json::to_value(other)?,
    };

    let payload: StaticRoutesExtensionV1 = serde_json::from_value(value)
        .map_err(|err| anyhow::anyhow!("invalid static routes extension payload: {err}"))?;
    Ok(Some(payload))
}

pub fn validate_static_routes_payload<F>(
    payload: &StaticRoutesExtensionV1,
    mut pack_path_exists: F,
) -> Result<()>
where
    F: FnMut(&str) -> bool,
{
    if payload.version != 1 {
        bail!("extensions[{STATIC_ROUTES_EXTENSION_KEY}] version must be 1");
    }
    if payload.routes.is_empty() {
        bail!("extensions[{STATIC_ROUTES_EXTENSION_KEY}] routes must not be empty");
    }

    let mut seen_ids = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();
    let mut seen_exports = BTreeSet::new();

    for route in &payload.routes {
        if route.id.trim().is_empty() {
            bail!("extensions[{STATIC_ROUTES_EXTENSION_KEY}] route id must not be empty");
        }
        if !seen_ids.insert(route.id.clone()) {
            bail!(
                "extensions[{STATIC_ROUTES_EXTENSION_KEY}] duplicate route id `{}`",
                route.id
            );
        }

        validate_public_path(&route.public_path).map_err(|err| {
            anyhow::anyhow!(
                "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` public_path invalid: {err}",
                route.id
            )
        })?;
        let normalized_path = normalize_public_path(&route.public_path);
        if !seen_paths.insert(normalized_path.clone()) {
            bail!(
                "extensions[{STATIC_ROUTES_EXTENSION_KEY}] duplicate public_path `{}`",
                normalized_path
            );
        }

        validate_source_root(&route.source_root).map_err(|err| {
            anyhow::anyhow!(
                "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` source_root invalid: {err}",
                route.id
            )
        })?;
        if !pack_path_exists(&route.source_root) {
            bail!(
                "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` source_root missing: {}",
                route.id,
                route.source_root
            );
        }

        if route.scope.team && !route.scope.tenant {
            bail!(
                "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` cannot set scope.team=true when scope.tenant=false",
                route.id
            );
        }

        validate_cache(route).map_err(|err| {
            anyhow::anyhow!(
                "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` cache invalid: {err}",
                route.id
            )
        })?;

        if let Some(index_file) = route.index_file.as_deref() {
            let logical = route_asset_path(&route.source_root, index_file).map_err(|err| {
                anyhow::anyhow!(
                    "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` index_file invalid: {err}",
                    route.id
                )
            })?;
            if !pack_path_exists(&logical) {
                bail!(
                    "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` index_file missing: {}",
                    route.id,
                    logical
                );
            }
        }

        if let Some(spa_fallback) = route.spa_fallback.as_deref() {
            let logical = route_asset_path(&route.source_root, spa_fallback).map_err(|err| {
                anyhow::anyhow!(
                    "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` spa_fallback invalid: {err}",
                    route.id
                )
            })?;
            if !pack_path_exists(&logical) {
                bail!(
                    "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` spa_fallback missing: {}",
                    route.id,
                    logical
                );
            }
        }

        for (export_key, export_name) in &route.exports {
            if export_key.trim().is_empty() {
                bail!(
                    "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` export keys must not be empty",
                    route.id
                );
            }
            if export_name.trim().is_empty() {
                bail!(
                    "extensions[{STATIC_ROUTES_EXTENSION_KEY}] route `{}` export `{}` must not be empty",
                    route.id,
                    export_key
                );
            }
            if !seen_exports.insert(export_name.clone()) {
                bail!(
                    "extensions[{STATIC_ROUTES_EXTENSION_KEY}] duplicate export name `{}`",
                    export_name
                );
            }
        }
    }

    Ok(())
}

pub fn validate_public_path(path: &str) -> Result<()> {
    if !path.starts_with('/') {
        bail!("must start with `/`");
    }
    if !path.starts_with("/v1/web/") {
        bail!("must start with `/v1/web/`");
    }
    if path.contains('?') || path.contains('#') {
        bail!("query strings and fragments are not allowed");
    }

    for segment in path.split('/').skip(1) {
        if segment.is_empty() {
            bail!("empty path segments are not allowed");
        }
        match segment {
            "{tenant}" | "{team}" => continue,
            "." | ".." => bail!("path traversal segments are not allowed"),
            _ => {}
        }
        if !segment
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            bail!("unsupported path segment `{segment}`");
        }
    }

    Ok(())
}

pub fn normalize_public_path(path: &str) -> String {
    if path.len() > 1 {
        path.trim_end_matches('/').to_string()
    } else {
        path.to_string()
    }
}

pub fn validate_source_root(path: &str) -> Result<()> {
    if !path.starts_with("assets/") {
        bail!("must start with `assets/`");
    }
    if path.ends_with('/') {
        bail!("must not end with `/`");
    }
    validate_relative_path(&path["assets/".len()..])
}

pub fn route_asset_path(source_root: &str, relative: &str) -> Result<String> {
    validate_relative_path(relative)?;
    Ok(format!("{source_root}/{relative}"))
}

fn validate_cache(route: &StaticRouteV1) -> Result<()> {
    let Some(cache) = route.cache.as_ref() else {
        return Ok(());
    };

    match cache.strategy.as_str() {
        "none" => {
            if cache.max_age_seconds.is_some() {
                bail!("max_age_seconds is only valid when strategy is `public-max-age`");
            }
        }
        "public-max-age" => {
            if cache.max_age_seconds.is_none() {
                bail!("max_age_seconds is required when strategy is `public-max-age`");
            }
        }
        other => bail!("unknown cache strategy `{other}`"),
    }

    Ok(())
}

fn validate_relative_path(path: &str) -> Result<()> {
    if path.trim().is_empty() {
        bail!("must not be empty");
    }
    if path.starts_with('/') {
        bail!("must be relative");
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            bail!("empty path segments are not allowed");
        }
        if matches!(segment, "." | "..") {
            bail!("path traversal segments are not allowed");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_path_rejects_unknown_placeholders() {
        let err = validate_public_path("/v1/web/demo/{pack}").unwrap_err();
        assert!(err.to_string().contains("unsupported path segment"));
    }

    #[test]
    fn payload_validation_enforces_unique_export_names() {
        let payload = StaticRoutesExtensionV1 {
            version: 1,
            routes: vec![
                StaticRouteV1 {
                    id: "a".into(),
                    public_path: "/v1/web/demo".into(),
                    source_root: "assets/demo".into(),
                    scope: StaticRouteScopeV1::default(),
                    index_file: Some("index.html".into()),
                    spa_fallback: None,
                    cache: None,
                    exports: BTreeMap::from([("base_url".into(), "shared_url".into())]),
                },
                StaticRouteV1 {
                    id: "b".into(),
                    public_path: "/v1/web/demo2".into(),
                    source_root: "assets/demo2".into(),
                    scope: StaticRouteScopeV1::default(),
                    index_file: Some("index.html".into()),
                    spa_fallback: None,
                    cache: None,
                    exports: BTreeMap::from([("entry_url".into(), "shared_url".into())]),
                },
            ],
        };

        let err = validate_static_routes_payload(&payload, |path| {
            matches!(
                path,
                "assets/demo"
                    | "assets/demo/index.html"
                    | "assets/demo2"
                    | "assets/demo2/index.html"
            )
        })
        .unwrap_err();
        assert!(err.to_string().contains("duplicate export name"));
    }
}
