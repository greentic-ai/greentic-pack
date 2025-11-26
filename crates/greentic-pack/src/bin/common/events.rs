use anyhow::{Result, anyhow};
use clap::ValueEnum;
use greentic_pack::events::EventProviderSpec;
use greentic_pack::reader::{SigningPolicy, open_pack};
use serde_json::json;

use crate::EventsListArgs;
use crate::input::materialize_pack_path;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
}

pub fn list(args: &EventsListArgs) -> Result<()> {
    let (temp, pack_path) = materialize_pack_path(&args.path, args.verbose)?;
    let load = open_pack(&pack_path, SigningPolicy::DevOk).map_err(|err| anyhow!(err.message))?;
    let providers = load
        .manifest
        .meta
        .events
        .as_ref()
        .map(|events| events.providers.clone())
        .unwrap_or_default();

    match args.format {
        OutputFormat::Table => print_table(&providers),
        OutputFormat::Json => print_json(&providers)?,
        OutputFormat::Yaml => print_yaml(&providers)?,
    }

    drop(temp);
    Ok(())
}

fn print_table(providers: &[EventProviderSpec]) {
    if providers.is_empty() {
        println!("No events providers declared.");
        return;
    }

    println!(
        "{:<20} {:<8} {:<28} {:<12} TOPICS",
        "NAME", "KIND", "COMPONENT", "TRANSPORT"
    );
    for provider in providers {
        let transport = provider
            .capabilities
            .transport
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "-".to_string());
        let topics = summarize_topics(&provider.capabilities.topics);
        println!(
            "{:<20} {:<8} {:<28} {:<12} {}",
            provider.name, provider.kind, provider.component, transport, topics
        );
    }
}

fn print_json(providers: &[EventProviderSpec]) -> Result<()> {
    let payload = json!(providers);
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn print_yaml(providers: &[EventProviderSpec]) -> Result<()> {
    let doc = serde_yaml_bw::to_string(providers)?;
    println!("{doc}");
    Ok(())
}

fn summarize_topics(topics: &[String]) -> String {
    if topics.is_empty() {
        return "-".to_string();
    }
    let combined = topics.join(", ");
    if combined.len() > 60 {
        format!("{}...", &combined[..57])
    } else {
        combined
    }
}
