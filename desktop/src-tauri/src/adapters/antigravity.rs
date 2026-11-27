//! Reserved local Antigravity adapter slot.
//! Shares the exact same Workspace File Contract (`creations/<job-id>/`) as Codex without copying credentials.
use super::{AdapterStatus, AgentAdapter, Capability, EventSink, StartedTurn, TurnRequest};
use std::{path::Path, sync::Arc};

pub struct AntigravityAdapter;

impl AntigravityAdapter {
    pub fn new() -> Self {
        Self
    }

    fn detect_local_install() -> bool {
        if std::env::var_os("CREATORX_ANTIGRAVITY_URL").is_some() {
            return true;
        }
        if let Some(home) = std::env::var_os("HOME") {
            if Path::new(&home).join(".gemini/antigravity").is_dir() {
                return true;
            }
        }
        #[cfg(target_os = "macos")]
        if Path::new("/Applications/Antigravity.app").exists() {
            return true;
        }
        false
    }
}

impl AgentAdapter for AntigravityAdapter {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn status(&self, _verify_live: bool) -> AdapterStatus {
        let capabilities = vec![
            Capability::Planning,
            Capability::ImageGeneration,
            Capability::WorkspaceExec,
        ];
        let bridge_configured = std::env::var("CREATORX_ANTIGRAVITY_URL")
            .ok()
            .is_some_and(|v| !v.trim().is_empty());
        if bridge_configured {
            AdapterStatus {
                id: "antigravity".into(),
                display_name: "Antigravity（桥接实验槽位）".into(),
                available: false,
                capabilities,
                detail: Some(
                    "已预留 Antigravity 标准适配器接口，会话执行桥接尚未启用；当前请切换 Codex 执行。"
                        .into(),
                ),
            }
        } else if Self::detect_local_install() {
            AdapterStatus {
                id: "antigravity".into(),
                display_name: "Antigravity（预留待接入）".into(),
                available: false,
                capabilities,
                detail: Some(
                    "已检测到本机 Antigravity 环境；适配器接口与沙盒文件契约已就绪，待启用本地桥接后可直接切换。"
                        .into(),
                ),
            }
        } else {
            AdapterStatus {
                id: "antigravity".into(),
                display_name: "Antigravity（未安装）".into(),
                available: false,
                capabilities,
                detail: Some("未检测到本机 Antigravity 环境".into()),
            }
        }
    }

    fn start_turn(
        &self,
        _req: TurnRequest,
        _sink: Arc<dyn EventSink>,
    ) -> Result<StartedTurn, String> {
        Err(
            "Antigravity 本地会话桥接当前为架构预留槽位，尚未开启直连执行；请在顶部引擎选择器切换回 Codex。"
                .into(),
        )
    }

    fn cancel_turn(&self, _thread_id: &str, _turn_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn shutdown(&self) {}
}
