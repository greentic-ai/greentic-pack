#[derive(Debug, Clone)]
pub struct McpAdapterRef {
    pub protocol: &'static str,
    pub image: &'static str,
    pub digest: Option<&'static str>,
}

/// Pinned MCP adapter reference for protocol 25.06.18.
/// TODO(maarten): replace with real adapter tag/digest once greentic-mcp publishes it.
pub const MCP_ADAPTER_25_06_18: McpAdapterRef = McpAdapterRef {
    protocol: "25.06.18",
    image: "ghcr.io/greentic-ai/greentic-mcp-adapter:25.06.18-v0.4.4",
    digest: None, // TODO: fill digest once known
};
