use super::report::{ComponentInfo, InfoReport, SignatureInfo, SignatureStatus};
use std::fmt::Write;

pub fn render(r: &InfoReport) -> String {
    let mut s = String::new();
    let header = match &r.kind {
        Some(k) => format!("{} {} · {}", r.name, r.version, k),
        None => format!("{} {}", r.name, r.version),
    };
    let _ = writeln!(s, "{header}");
    if let Some(d) = &r.description {
        let _ = writeln!(s, "{d}");
    }
    let _ = writeln!(s);

    if !r.authors.is_empty() {
        kv(&mut s, "Authors", &r.authors.join(", "));
    }
    if let Some(v) = &r.license {
        kv(&mut s, "License", v);
    }
    if let Some(v) = &r.homepage {
        kv(&mut s, "Homepage", v);
    }
    kv(&mut s, "Created", &r.created_at_utc);
    kv(&mut s, "Signature", &signature_line(&r.signature));

    if !r.components.is_empty() {
        let _ = writeln!(s, "\nComponents ({})", r.components.len());
        for c in &r.components {
            render_component(&mut s, c);
        }
    }

    if !r.entry_flows.is_empty() {
        let _ = writeln!(s, "\nEntry flows ({})", r.entry_flows.len());
        for f in &r.entry_flows {
            let _ = writeln!(s, "  {f}");
        }
    }

    if !r.imports.is_empty() {
        let _ = writeln!(s, "\nImports ({})", r.imports.len());
        for i in &r.imports {
            let _ = writeln!(s, "  {} {}", i.pack_id, i.version_req);
        }
    }

    if !r.interfaces.is_empty() {
        let _ = writeln!(s, "\nInterfaces ({})", r.interfaces.len());
        for i in &r.interfaces {
            let _ = writeln!(s, "  {i}");
        }
    }

    s
}

fn kv(s: &mut String, label: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    let _ = writeln!(s, "{:<12} {}", label, value);
}

fn render_component(s: &mut String, c: &ComponentInfo) {
    let kind = c.kind.as_deref().unwrap_or("");
    let _ = writeln!(s, "  {:<28} {:<10} {}", c.component_id, c.version, kind);
}

fn signature_line(sig: &SignatureInfo) -> String {
    match (sig.status, &sig.key_id) {
        (SignatureStatus::Signed, Some(id)) => format!("signed · key_id={id}"),
        (SignatureStatus::Signed, None) => "signed".into(),
        (SignatureStatus::Unsigned, _) => "unsigned".into(),
        (SignatureStatus::Invalid, Some(id)) => format!("invalid · key_id={id}"),
        (SignatureStatus::Invalid, None) => "invalid".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::report::*;
    use super::*;

    fn sample(sig: SignatureInfo) -> InfoReport {
        InfoReport {
            info_schema_version: 1,
            name: "hello-bot".into(),
            version: "1.2.3".into(),
            kind: Some("application".into()),
            description: Some("A demo".into()),
            authors: vec!["Alice".into()],
            license: Some("MIT".into()),
            homepage: Some("https://greentic.ai/packs/hello-bot".into()),
            support: None,
            vendor: None,
            created_at_utc: "2026-01-01T00:00:00Z".into(),
            signature: sig,
            components: vec![ComponentInfo {
                component_id: "messaging/slack".into(),
                version: "1.4.0".into(),
                kind: Some("messaging".into()),
            }],
            entry_flows: vec!["flows/a.ygtc".into()],
            imports: vec![ImportInfo {
                pack_id: "greentic/core".into(),
                version_req: "^0.6.0".into(),
            }],
            interfaces: vec!["greentic:component@0.6.0".into()],
        }
    }

    #[test]
    fn signed_pack_renders_header_and_key_id() {
        let out = render(&sample(SignatureInfo {
            status: SignatureStatus::Signed,
            key_id: Some("ed25519:abc".into()),
        }));
        assert!(out.contains("hello-bot 1.2.3 · application"));
        assert!(out.contains("A demo"));
        assert!(out.contains("signed · key_id=ed25519:abc"));
        assert!(out.contains("messaging/slack"));
        assert!(out.contains("greentic/core ^0.6.0"));
        assert!(out.contains("greentic:component@0.6.0"));
    }

    #[test]
    fn unsigned_pack_renders_unsigned() {
        let out = render(&sample(SignatureInfo {
            status: SignatureStatus::Unsigned,
            key_id: None,
        }));
        assert!(out.contains("unsigned"));
        assert!(!out.contains("key_id="));
    }

    #[test]
    fn invalid_signature_with_key_id_renders_invalid_and_key_id() {
        let out = render(&sample(SignatureInfo {
            status: SignatureStatus::Invalid,
            key_id: Some("ed25519:bad".into()),
        }));
        assert!(out.contains("invalid · key_id=ed25519:bad"));
        assert!(!out.contains("signed"));
        assert!(!out.contains("unsigned"));
    }

    #[test]
    fn invalid_signature_without_key_id_renders_bare_invalid() {
        let out = render(&sample(SignatureInfo {
            status: SignatureStatus::Invalid,
            key_id: None,
        }));
        // Must contain "invalid" on the Signature row but NOT "key_id=".
        assert!(out.contains("invalid"));
        assert!(!out.contains("key_id="));
        // Also must not accidentally render "signed" or "unsigned" as the status.
        let signature_line = out.lines().find(|l| l.starts_with("Signature")).unwrap();
        assert_eq!(signature_line.trim(), "Signature    invalid");
    }

    #[test]
    fn omits_sections_when_empty() {
        let r = InfoReport {
            info_schema_version: 1,
            name: "min".into(),
            version: "0.1.0".into(),
            kind: None,
            description: None,
            authors: vec![],
            license: None,
            homepage: None,
            support: None,
            vendor: None,
            created_at_utc: "2026-01-01T00:00:00Z".into(),
            signature: SignatureInfo {
                status: SignatureStatus::Unsigned,
                key_id: None,
            },
            components: vec![],
            entry_flows: vec![],
            imports: vec![],
            interfaces: vec![],
        };
        let out = render(&r);
        assert!(out.contains("min 0.1.0"));
        assert!(!out.contains("Authors"));
        assert!(!out.contains("Components"));
        assert!(!out.contains("Entry flows"));
        assert!(!out.contains("Imports"));
        assert!(!out.contains("Interfaces"));
    }
}
