use serde::{Deserialize, Serialize};

pub mod pack_export {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FlowInfo {
        pub id: String,
        pub human_name: Option<String>,
        pub description: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SchemaDoc {
        pub flow_id: String,
        pub schema_json: serde_json::Value,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PrepareResult {
        pub status: String,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct RunResult {
        pub status: String,
        pub output: Option<serde_json::Value>,
        pub error: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct A2AItem {
        pub title: String,
        pub flow_id: String,
    }

    pub trait PackExport {
        fn list_flows(&self) -> Vec<FlowInfo>;
        fn get_flow_schema(&self, flow_id: &str) -> Option<SchemaDoc>;
        fn prepare_flow(&self, flow_id: &str) -> PrepareResult;
        fn run_flow(&self, flow_id: &str, input: serde_json::Value) -> RunResult;
        fn a2a_search(&self, query: &str) -> Vec<A2AItem>;
    }
}
