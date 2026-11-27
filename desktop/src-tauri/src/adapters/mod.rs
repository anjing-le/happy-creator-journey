pub mod antigravity;
pub mod codex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    Planning,
    ImageGeneration,
    ImageRevision,
    WorkspaceExec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterStatus {
    pub id: String,
    pub display_name: String,
    pub available: bool,
    pub capabilities: Vec<Capability>,
    pub detail: Option<String>,
}

pub struct TurnRequest {
    pub thread_id: Option<String>,
    pub cwd: PathBuf,
    pub developer_instructions: String,
    pub user_messages: Vec<String>,
    pub skill_attachment: Option<(String, PathBuf)>,
    pub dynamic_tools: Vec<Value>,
}

pub struct StartedTurn {
    pub thread_id: String,
    pub turn_id: Option<String>,
}

#[derive(Debug)]
pub enum AdapterEvent {
    TurnStarted {
        thread_id: String,
        turn_id: String,
    },
    Activity {
        thread_id: String,
        text: String,
    },
    MessageDelta {
        thread_id: String,
        item_id: String,
        delta: String,
    },
    MessageCompleted {
        thread_id: String,
        item_id: String,
        text: String,
    },
    ImageGenerated {
        thread_id: String,
        saved_path: Option<PathBuf>,
        base64_data: Option<String>,
    },
    TurnCompleted {
        thread_id: String,
        status: String,
        error: Option<String>,
    },
    Disconnected {
        error: String,
    },
}

pub trait EventSink: Send + Sync + 'static {
    fn on_event(&self, event: AdapterEvent);
    fn on_tool_call(
        &self,
        thread_id: &str,
        turn_id: &str,
        tool: &str,
        arguments: &Value,
    ) -> Result<Value, String>;
}

pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn status(&self, verify_live: bool) -> AdapterStatus;
    fn start_turn(
        &self,
        req: TurnRequest,
        sink: Arc<dyn EventSink>,
    ) -> Result<StartedTurn, String>;
    fn cancel_turn(&self, thread_id: &str, turn_id: &str) -> Result<(), String>;
    fn shutdown(&self);
}

pub struct AdapterRegistry {
    adapters: Vec<Arc<dyn AgentAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: vec![
                Arc::new(codex::CodexAdapter::new()),
                Arc::new(antigravity::AntigravityAdapter::new()),
            ],
        }
    }

    #[cfg(test)]
    pub fn with_adapters(adapters: Vec<Arc<dyn AgentAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn list_statuses(&self, verify_id: Option<&str>) -> Vec<AdapterStatus> {
        self.adapters
            .iter()
            .map(|adapter| adapter.status(verify_id == Some(adapter.id())))
            .collect()
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn AgentAdapter>, String> {
        self.adapters
            .iter()
            .find(|a| a.id() == id)
            .cloned()
            .ok_or_else(|| format!("未注册的本地 Agent 适配器：{id}"))
    }

    pub fn shutdown_all(&self) {
        for adapter in &self.adapters {
            adapter.shutdown();
        }
    }
}
