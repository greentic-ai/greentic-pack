use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use wit_bindgen_core::Files;
use wit_bindgen_core::WorldGenerator;
use wit_bindgen_core::wit_parser::Resolve;
use wit_bindgen_rust::Opts;

#[path = "build_support/wit_paths.rs"]
mod wit_paths;
use wit_paths::canonical_wit_root;

fn main() -> Result<(), Box<dyn Error>> {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_arch == "wasm32" {
        // Generate guest bindings even when targeting wasm; build happens on host.
    }

    let active_features = active_features();
    ensure_any_world_feature(&active_features)?;

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let staged_root = out_dir.join("wit-staging");
    reset_directory(&staged_root)?;

    let wit_root = canonical_wit_root();
    println!("cargo:rerun-if-changed={}", wit_root.display());
    let mut package_paths = Vec::new();
    discover_packages(&wit_root, &mut package_paths)?;
    // Explicitly ensure new v1 surfaces are staged even if discovery misses them.
    for rel in [
        "greentic/common-types@0.1.0/package.wit",
        "greentic/component-v1@0.1.0/package.wit",
        "greentic/pack-export-v1@0.1.0/package.wit",
    ] {
        let path = wit_root.join(rel);
        if path.exists() && !package_paths.contains(&path) {
            package_paths.push(path);
        }
    }
    {
        let rel = "provider-common/world.wit";
        let path = wit_root.join(rel);
        if path.exists() && !package_paths.contains(&path) {
            package_paths.push(path);
        }
    }

    let mut staged = HashSet::new();
    for package_path in package_paths {
        let package_ref = read_package_ref(&package_path)?;
        if staged.insert(package_ref) {
            stage_package(&package_path, &staged_root, &wit_root)?;
        }
    }

    let bindings_dir = generate_rust_bindings(&staged_root, &out_dir, &active_features)?;
    println!(
        "cargo:rustc-env=GREENTIC_INTERFACES_GUEST_BINDINGS={}",
        bindings_dir.display()
    );

    Ok(())
}

fn stage_package(
    src_path: &Path,
    staged_root: &Path,
    wit_root: &Path,
) -> Result<(), Box<dyn Error>> {
    let package_ref = read_package_ref(src_path)?;
    let dest_dir = staged_root.join(sanitize(&package_ref));
    fs::create_dir_all(&dest_dir)?;
    fs::copy(src_path, dest_dir.join("package.wit"))?;
    println!("cargo:rerun-if-changed={}", src_path.display());

    stage_dependencies(&dest_dir, src_path, wit_root)?;
    Ok(())
}

fn stage_dependencies(
    parent_dir: &Path,
    source_path: &Path,
    wit_root: &Path,
) -> Result<(), Box<dyn Error>> {
    let deps = parse_deps(source_path)?;
    if deps.is_empty() {
        return Ok(());
    }

    let deps_dir = parent_dir.join("deps");
    fs::create_dir_all(&deps_dir)?;

    for dep in deps {
        let dep_src = wit_path(&dep, wit_root)?;
        let dep_dest = deps_dir.join(sanitize(&dep));
        fs::create_dir_all(&dep_dest)?;
        fs::copy(&dep_src, dep_dest.join("package.wit"))?;
        println!("cargo:rerun-if-changed={}", dep_src.display());

        stage_dependencies(&dep_dest, &dep_src, wit_root)?;
    }

    Ok(())
}

fn wit_path(package_ref: &str, wit_root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let (pkg, version) = package_ref
        .split_once('@')
        .ok_or_else(|| format!("invalid package reference: {package_ref}"))?;
    let base_pkg = pkg.split('/').next().unwrap_or(pkg);
    let target_root = format!("{base_pkg}@{version}");
    let mut fallback = None;
    if let Some(found) =
        find_package_recursive(wit_root, package_ref, &target_root, &mut fallback, false)?
    {
        return Ok(found);
    }
    if let Some(found) =
        find_package_recursive(wit_root, package_ref, &target_root, &mut fallback, true)?
    {
        return Ok(found);
    }
    if let Some(path) = fallback {
        return Ok(path);
    }
    Err(format!("missing WIT source for {package_ref}").into())
}

fn read_package_ref(path: &Path) -> Result<String, Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("package ") {
            return Ok(rest.trim_end_matches(';').trim().to_string());
        }
    }
    Err(format!("unable to locate package declaration in {}", path.display()).into())
}

fn parse_deps(path: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    let mut deps = Vec::new();

    for line in contents.lines() {
        let trimmed = line.trim_start();
        let rest = if let Some(rest) = trimmed.strip_prefix("use ") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("import ") {
            rest
        } else {
            continue;
        };

        let token = rest.split_whitespace().next().unwrap_or("");
        let token = token.trim_end_matches(';');
        let token = token.split(".{").next().unwrap_or(token);
        let token = token.split('{').next().unwrap_or(token);

        let (pkg_part, version_part) = match token.split_once('@') {
            Some(parts) => parts,
            None => continue,
        };

        let base_pkg = pkg_part.split('/').next().unwrap_or(pkg_part);
        let mut version = String::new();
        for ch in version_part.chars() {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                version.push(ch);
            } else {
                break;
            }
        }
        while version.ends_with('.') {
            version.pop();
        }
        if version.is_empty() {
            continue;
        }

        let dep_ref = format!("{base_pkg}@{version}");
        if !deps.contains(&dep_ref) {
            deps.push(dep_ref);
        }
    }

    Ok(deps)
}

fn sanitize(package_ref: &str) -> String {
    package_ref.replace([':', '@', '/'], "-")
}

fn generate_rust_bindings(
    staged_root: &Path,
    out_dir: &Path,
    active_features: &HashSet<String>,
) -> Result<PathBuf, Box<dyn Error>> {
    let bindings_dir = out_dir.join("bindings");
    reset_directory(&bindings_dir)?;

    let mut package_paths = Vec::new();
    let mut inserted = HashSet::new();

    for entry in fs::read_dir(staged_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let package_path = path.join("package.wit");
        if !package_path.exists() {
            continue;
        }

        let package_ref = read_package_ref(&package_path)?;
        if !inserted.insert(package_ref) {
            continue;
        }

        package_paths.push(path);
    }

    if package_paths.is_empty() {
        return Err("no WIT worlds discovered to generate bindings for".into());
    }

    package_paths.sort();

    let opts = Opts {
        generate_all: true,
        generate_unused_types: true,
        ..Default::default()
    };

    let mut mod_rs = String::new();

    for path in package_paths {
        let mut resolve = Resolve::new();
        let (pkg, _) = resolve.push_dir(&path)?;
        let package_name = resolve.packages[pkg].name.clone();
        let package_ref = read_package_ref(&path.join("package.wit"))?;

        let mut worlds: Vec<_> = resolve.packages[pkg]
            .worlds
            .iter()
            .map(|(name, id)| (name.to_string(), *id))
            .collect();
        worlds.sort_by(|(a_name, _), (b_name, _)| a_name.cmp(b_name));

        for (world_name, world_id) in worlds {
            if !world_enabled(&package_ref, &world_name, active_features) {
                continue;
            }
            let module_name = module_name(&package_name, &world_name);
            let mut files = Files::default();
            let mut generator = opts.clone().build();
            generator.generate(&mut resolve, world_id, &mut files)?;

            let mut combined = Vec::new();
            for (_, contents) in files.iter() {
                combined.extend_from_slice(contents);
            }
            fs::write(bindings_dir.join(format!("{module_name}.rs")), combined)?;
            mod_rs.push_str(&format!(
                "pub mod {module_name} {{ include!(concat!(env!(\"GREENTIC_INTERFACES_GUEST_BINDINGS\"), \"/{module_name}.rs\")); }}\n"
            ));
        }
    }

    fs::write(bindings_dir.join("mod.rs"), mod_rs)?;

    Ok(bindings_dir)
}

fn module_name(name: &wit_bindgen_core::wit_parser::PackageName, world: &str) -> String {
    let formatted = format!("{name}-{world}");
    sanitize(&formatted).replace(['-', '.'], "_")
}

fn reset_directory(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn discover_packages(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("deps") {
                continue;
            }
            let package_file = path.join("package.wit");
            if package_file.exists() {
                out.push(package_file);
            }
            discover_packages(&path, out)?;
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("wit") {
            out.push(path);
        }
    }
    Ok(())
}

fn find_package_recursive(
    dir: &Path,
    package_ref: &str,
    target_root: &str,
    fallback: &mut Option<PathBuf>,
    include_deps: bool,
) -> Result<Option<PathBuf>, Box<dyn Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if !include_deps && path.file_name().and_then(|n| n.to_str()) == Some("deps") {
                continue;
            }
            let package_file = path.join("package.wit");
            if package_file.exists() {
                let entry_package = read_package_ref(&package_file)?;
                if entry_package == package_ref {
                    return Ok(Some(package_file));
                }
                if fallback.is_none() && entry_package == target_root {
                    *fallback = Some(package_file.clone());
                }
            }
            if let Some(found) =
                find_package_recursive(&path, package_ref, target_root, fallback, include_deps)?
            {
                return Ok(Some(found));
            }
        } else if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("wit") {
            let entry_package = read_package_ref(&path)?;
            if entry_package == package_ref {
                return Ok(Some(path));
            }
            if fallback.is_none() && entry_package == target_root {
                *fallback = Some(path.clone());
            }
        }
    }
    Ok(None)
}

#[derive(Debug)]
struct WorldFeature {
    package: &'static str,
    world: &'static str,
    feature: &'static str,
}

const WORLD_FEATURES: &[WorldFeature] = &[
    WorldFeature {
        package: "greentic:component@0.5.0",
        world: "component",
        feature: "component-node",
    },
    WorldFeature {
        package: "greentic:component@0.6.0",
        world: "component",
        feature: "component-v0-6",
    },
    WorldFeature {
        package: "greentic:component@0.4.0",
        world: "component",
        feature: "component-node-v0-4",
    },
    WorldFeature {
        package: "greentic:component@1.0.0",
        world: "component",
        feature: "component-v1",
    },
    WorldFeature {
        package: "greentic:component-v1@0.1.0",
        world: "component-host",
        feature: "component-v1",
    },
    WorldFeature {
        package: "greentic:lifecycle@1.0.0",
        world: "component-lifecycle",
        feature: "lifecycle",
    },
    WorldFeature {
        package: "greentic:build@1.0.0",
        world: "builder",
        feature: "build",
    },
    WorldFeature {
        package: "greentic:deploy-plan@1.0.0",
        world: "plan",
        feature: "deploy-plan",
    },
    WorldFeature {
        package: "greentic:distribution@1.0.0",
        world: "distribution",
        feature: "distribution",
    },
    WorldFeature {
        package: "greentic:distributor-api@1.0.0",
        world: "distributor-api",
        feature: "distributor-api",
    },
    WorldFeature {
        package: "greentic:distributor-api@1.0.0",
        world: "distributor-api-imports",
        feature: "distributor-api-imports",
    },
    WorldFeature {
        package: "greentic:distributor-api@1.1.0",
        world: "distributor-api",
        feature: "distributor-api-v1-1",
    },
    WorldFeature {
        package: "greentic:distributor-api@1.1.0",
        world: "distributor-api-imports",
        feature: "distributor-api-v1-1-imports",
    },
    WorldFeature {
        package: "greentic:http@1.0.0",
        world: "client",
        feature: "http-client",
    },
    WorldFeature {
        package: "greentic:http@1.1.0",
        world: "client",
        feature: "http-client-v1-1",
    },
    WorldFeature {
        package: "greentic:telemetry@1.0.0",
        world: "logger",
        feature: "telemetry",
    },
    WorldFeature {
        package: "greentic:oauth-broker@1.0.0",
        world: "broker",
        feature: "oauth-broker",
    },
    WorldFeature {
        package: "greentic:oauth-broker@1.0.0",
        world: "broker-client",
        feature: "oauth-broker",
    },
    WorldFeature {
        package: "greentic:worker@1.0.0",
        world: "worker",
        feature: "worker",
    },
    WorldFeature {
        package: "greentic:gui@1.0.0",
        world: "gui-fragment",
        feature: "gui-fragment",
    },
    WorldFeature {
        package: "greentic:secrets-store@1.0.0",
        world: "store",
        feature: "secrets",
    },
    WorldFeature {
        package: "provider:common@0.0.2",
        world: "common",
        feature: "provider-common",
    },
    WorldFeature {
        package: "greentic:provider-schema-core@1.0.0",
        world: "schema-core",
        feature: "provider-core-v1",
    },
    WorldFeature {
        package: "greentic:operator@1.0.0",
        world: "hook-provider",
        feature: "operator-hooks-v1",
    },
    WorldFeature {
        package: "greentic:state@1.0.0",
        world: "store",
        feature: "state-store",
    },
    WorldFeature {
        package: "greentic:metadata@1.0.0",
        world: "metadata-store",
        feature: "metadata",
    },
    WorldFeature {
        package: "greentic:pack-export@0.2.0",
        world: "pack-exports",
        feature: "pack-export",
    },
    WorldFeature {
        package: "greentic:pack-export@0.4.0",
        world: "pack-exports",
        feature: "pack-export",
    },
    WorldFeature {
        package: "greentic:pack-export-v1@0.1.0",
        world: "pack-host",
        feature: "pack-export-v1",
    },
    WorldFeature {
        package: "greentic:pack-validate@0.1.0",
        world: "pack-validator",
        feature: "pack-validate",
    },
    WorldFeature {
        package: "greentic:provision@0.1.0",
        world: "provision-runner",
        feature: "provision",
    },
    WorldFeature {
        package: "greentic:interfaces-pack@0.1.0",
        world: "component",
        feature: "pack-export",
    },
    WorldFeature {
        package: "greentic:source@1.0.0",
        world: "source-sync",
        feature: "repo",
    },
    WorldFeature {
        package: "greentic:scan@1.0.0",
        world: "scanner",
        feature: "scan",
    },
    WorldFeature {
        package: "greentic:signing@1.0.0",
        world: "signer",
        feature: "signing",
    },
    WorldFeature {
        package: "greentic:attestation@1.0.0",
        world: "attester",
        feature: "attestation",
    },
    WorldFeature {
        package: "greentic:policy@1.0.0",
        world: "policy-evaluator",
        feature: "policy",
    },
    WorldFeature {
        package: "greentic:oci@1.0.0",
        world: "oci-distribution",
        feature: "oci",
    },
    WorldFeature {
        package: "greentic:repo-ui-actions@1.0.0",
        world: "repo-ui-worker",
        feature: "repo-ui-actions",
    },
    WorldFeature {
        package: "greentic:host@1.0.0",
        world: "runner-host",
        feature: "runner",
    },
    WorldFeature {
        package: "greentic:types-core@0.2.0",
        world: "core",
        feature: "types-core",
    },
    WorldFeature {
        package: "greentic:types-core@0.4.0",
        world: "core",
        feature: "types-core",
    },
    WorldFeature {
        package: "wasix:mcp@24.11.5",
        world: "mcp-router",
        feature: "wasix-mcp-24-11-05-guest",
    },
    WorldFeature {
        package: "wasix:mcp@25.3.26",
        world: "mcp-router",
        feature: "wasix-mcp-25-03-26-guest",
    },
    WorldFeature {
        package: "wasix:mcp@25.6.18",
        world: "mcp-router",
        feature: "wasix-mcp-25-06-18-guest",
    },
];

fn active_features() -> HashSet<String> {
    env::vars()
        .filter_map(|(key, _)| {
            key.strip_prefix("CARGO_FEATURE_")
                .map(|value| value.to_ascii_lowercase().replace('_', "-"))
        })
        .collect()
}

fn ensure_any_world_feature(active_features: &HashSet<String>) -> Result<(), Box<dyn Error>> {
    if active_features
        .iter()
        .any(|feature| WORLD_FEATURES.iter().any(|wf| wf.feature == feature))
    {
        return Ok(());
    }

    Err("no world features enabled; enable at least one (e.g. component-node)".into())
}

fn world_enabled(package_ref: &str, world: &str, active_features: &HashSet<String>) -> bool {
    let guest_enabled = active_features.contains("guest");
    if let Some(world_feature) = WORLD_FEATURES
        .iter()
        .find(|wf| wf.package == package_ref && wf.world == world)
    {
        return active_features.contains(world_feature.feature);
    }

    guest_enabled
}
