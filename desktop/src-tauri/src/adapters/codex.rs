//! Local Codex app-server adapter. Credentials stay owned by the user's local Codex installation.
use super::{
    AdapterEvent, AdapterStatus, AgentAdapter, Capability, EventSink, StartedTurn, TurnRequest,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

const MODEL: &str = "gpt-6.1-sol";
const ALLOWED_TOOLS: &[&str] = &[
    "creatorx_update_blueprint",
    "creatorx_render_slot",
    "creatorx_generate_image",
];

type PendingReplies = Arc<Mutex<HashMap<u64, mpsc::Sender<Result<Value, String>>>>>;

struct Client {
    stdin: Mutex<ChildStdin>,
    pending: PendingReplies,
    next: std::sync::atomic::AtomicU64,
    child: Mutex<Child>,
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(child) = self.child.get_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Client {
    fn send(&self, value: Value) -> Result<(), String> {
        let mut input = self.stdin.lock().map_err(|_| "Codex 连接正忙")?;
        writeln!(input, "{value}")
            .and_then(|_| input.flush())
            .map_err(|_| "Codex 连接已断开".into())
    }

    fn rpc(&self, method: &str, params: Value) -> Result<Value, String> {
        use std::sync::atomic::Ordering;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (send, receive) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| "Codex 连接正忙")?
            .insert(id, send);
        if let Err(e) = self.send(json!({"id":id,"method":method,"params":params})) {
            let _ = self.pending.lock().map(|mut p| p.remove(&id));
            return Err(e);
        }
        let result = receive
            .recv_timeout(Duration::from_secs(45))
            .map_err(|_| "Codex 请求超时；任务状态需核对，不会自动重试生成".to_string());
        let _ = self.pending.lock().map(|mut p| p.remove(&id));
        result?
    }
}

pub struct CodexAdapter {
    client: Mutex<Option<Arc<Client>>>,
    sink: Arc<Mutex<Option<Arc<dyn EventSink>>>>,
}

impl CodexAdapter {
    pub fn new() -> Self {
        Self {
            client: Mutex::new(None),
            sink: Arc::new(Mutex::new(None)),
        }
    }

    fn codex_path() -> Result<PathBuf, String> {
        if let Some(path) = std::env::var_os("CREATORX_CODEX_PATH") {
            if Path::new(&path).is_file() {
                return Ok(path.into());
            }
        }
        let file = if cfg!(windows) { "codex.exe" } else { "codex" };
        if let Some(paths) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&paths) {
                let path = dir.join(file);
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
        #[cfg(target_os = "macos")]
        for path in [
            "/Applications/ChatGPT.app/Contents/Resources/codex-cli/CodexCLI.app/Contents/MacOS/codex",
            "/Applications/Codex.app/Contents/Resources/codex",
            "/opt/homebrew/bin/codex",
            "/usr/local/bin/codex",
        ] {
            if Path::new(path).is_file() {
                return Ok(path.into());
            }
        }
        Err("未找到本机 Codex。请安装并登录 Codex CLI，或用 CREATORX_CODEX_PATH 指定可执行文件。Windows 需要原生 codex.exe。".into())
    }

    fn client(&self, sink_override: Option<Arc<dyn EventSink>>) -> Result<Arc<Client>, String> {
        if let Some(sink) = sink_override {
            if let Ok(mut slot) = self.sink.lock() {
                *slot = Some(sink);
            }
        }
        let mut slot = self.client.lock().map_err(|_| "Codex 连接正忙")?;
        if let Some(client) = slot.as_ref() {
            if client
                .child
                .lock()
                .map_err(|_| "Codex 进程正忙")?
                .try_wait()
                .map_err(|_| "无法检查 Codex")?
                .is_none()
            {
                return Ok(client.clone());
            }
            *slot = None;
        }
        let path = Self::codex_path()?;
        let mut command = Command::new(path);
        command
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("无法启动本机 Codex：{e}"))?;
        let input = child.stdin.take().ok_or("无法打开 Codex 输入")?;
        let output = child.stdout.take().ok_or("无法打开 Codex 输出")?;
        let pending = Arc::new(Mutex::new(HashMap::<
            u64,
            mpsc::Sender<Result<Value, String>>,
        >::new()));
        let client = Arc::new(Client {
            stdin: Mutex::new(input),
            pending: pending.clone(),
            next: std::sync::atomic::AtomicU64::new(1),
            child: Mutex::new(child),
        });
        let sink_holder = self.sink.clone();
        let weak_client = Arc::downgrade(&client);
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if let Some(request_id) = msg.get("id") {
                    if msg.get("method").is_some() {
                        let method = msg["method"].as_str().unwrap_or("");
                        let tool = msg["params"]["tool"].as_str().unwrap_or("");
                        if method == "item/tool/call" && ALLOWED_TOOLS.contains(&tool) {
                            let sink = sink_holder.lock().ok().and_then(|s| s.clone());
                            if let (Some(sink), Some(client)) = (sink, weak_client.upgrade()) {
                                let request = msg.clone();
                                let tool_name = tool.to_string();
                                std::thread::spawn(move || {
                                    let params = &request["params"];
                                    let thread_id = params["threadId"].as_str().unwrap_or("");
                                    let turn_id = params["turnId"].as_str().unwrap_or("");
                                    let result = sink.on_tool_call(
                                        thread_id,
                                        turn_id,
                                        &tool_name,
                                        &params["arguments"],
                                    );
                                    let (success, text) = match result {
                                        Ok(val) => {
                                            let ok = val
                                                .get("ok")
                                                .and_then(Value::as_bool)
                                                .unwrap_or(true);
                                            (ok, val.to_string())
                                        }
                                        Err(error) => (false, error),
                                    };
                                    let _ = client.send(json!({
                                        "id": request["id"],
                                        "result": {
                                            "success": success,
                                            "contentItems": [{"type": "inputText", "text": text}]
                                        }
                                    }));
                                });
                            }
                            continue;
                        }
                        if let Some(client) = weak_client.upgrade() {
                            let _ = client.send(json!({
                                "id": request_id,
                                "error": {
                                    "code": -32601,
                                    "message": "CreatorX Harness does not grant interactive command or permission approvals."
                                }
                            }));
                        }
                    } else if let Ok(mut map) = pending.lock() {
                        if let Some(send) = map.remove(&request_id.as_u64().unwrap_or(0)) {
                            let _ = send.send(if let Some(error) = msg.get("error") {
                                Err(error
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .unwrap_or("Codex 请求失败")
                                    .to_string())
                            } else {
                                Ok(msg.get("result").cloned().unwrap_or(Value::Null))
                            });
                        }
                    }
                } else if let Some(sink) = sink_holder.lock().ok().and_then(|s| s.clone()) {
                    forward_notification(sink.as_ref(), &msg);
                }
            }
            if let Ok(mut map) = pending.lock() {
                for (_, send) in map.drain() {
                    let _ = send.send(Err("Codex 连接已断开".into()));
                }
            }
            if let Some(sink) = sink_holder.lock().ok().and_then(|s| s.clone()) {
                sink.on_event(AdapterEvent::Disconnected {
                    error: "Codex 连接中断，可在原任务继续".into(),
                });
            }
        });
        client.rpc(
            "initialize",
            json!({
                "capabilities": {"experimentalApi": true},
                "clientInfo": {
                    "name": "creatorx",
                    "title": "CreatorX Creation Harness",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )?;
        client.send(json!({"method": "initialized", "params": {}}))?;
        *slot = Some(client.clone());
        Ok(client)
    }
}

fn forward_notification(sink: &dyn EventSink, msg: &Value) {
    let params = &msg["params"];
    let Some(thread_id) = params["threadId"].as_str() else {
        return;
    };
    let thread_id = thread_id.to_string();
    match msg["method"].as_str().unwrap_or("") {
        "turn/started" => {
            if let Some(turn_id) = params["turn"]["id"].as_str() {
                sink.on_event(AdapterEvent::TurnStarted {
                    thread_id,
                    turn_id: turn_id.into(),
                });
            }
        }
        "item/started" => {
            let text = match params["item"]["type"].as_str() {
                Some("imageGeneration") => "正在生成图片",
                Some("commandExecution") => "正在读取快照与同步蓝图文件",
                _ => "Codex 正在处理",
            };
            sink.on_event(AdapterEvent::Activity {
                thread_id,
                text: text.into(),
            });
        }
        "item/agentMessage/delta" => {
            sink.on_event(AdapterEvent::MessageDelta {
                thread_id,
                item_id: params["itemId"].as_str().unwrap_or("response").into(),
                delta: params["delta"].as_str().unwrap_or("").into(),
            });
        }
        "item/completed" => {
            let item = &params["item"];
            let item_id = item["id"].as_str().unwrap_or("response").to_string();
            if item["type"] == "agentMessage" {
                sink.on_event(AdapterEvent::MessageCompleted {
                    thread_id,
                    item_id,
                    text: item["text"]
                        .as_str()
                        .unwrap_or("")
                        .chars()
                        .take(200_000)
                        .collect(),
                });
            } else if item["type"] == "imageGeneration" && item["status"] == "completed" {
                sink.on_event(AdapterEvent::ImageGenerated {
                    thread_id,
                    saved_path: item["savedPath"].as_str().map(PathBuf::from),
                    base64_data: item["result"].as_str().map(str::to_string),
                });
            }
        }
        "turn/completed" => {
            sink.on_event(AdapterEvent::TurnCompleted {
                thread_id,
                status: params["turn"]["status"].as_str().unwrap_or("error").into(),
                error: params["turn"]["error"]["message"]
                    .as_str()
                    .map(str::to_string),
            });
        }
        _ => {}
    }
}

impl AgentAdapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn status(&self, verify_live: bool) -> AdapterStatus {
        let capabilities = vec![
            Capability::Planning,
            Capability::ImageGeneration,
            Capability::ImageRevision,
            Capability::WorkspaceExec,
        ];
        if !verify_live {
            return match Self::codex_path() {
                Ok(_) => AdapterStatus {
                    id: "codex".into(),
                    display_name: "Codex · 6.1 Sol · 中等".into(),
                    available: true,
                    capabilities,
                    detail: Some("本机 Codex CLI 已安装".into()),
                },
                Err(err) => AdapterStatus {
                    id: "codex".into(),
                    display_name: "Codex（未就绪）".into(),
                    available: false,
                    capabilities,
                    detail: Some(err),
                },
            };
        }
        let check = (|| -> Result<(), String> {
            let client = self.client(None)?;
            let models = client.rpc("model/list", json!({"includeHidden": true}))?;
            let model = models["data"]
                .as_array()
                .and_then(|data| data.iter().find(|m| m["model"] == MODEL))
                .ok_or("当前 Codex 账号无法使用 gpt-6.1-sol")?;
            if !model["supportedReasoningEfforts"]
                .as_array()
                .is_some_and(|v| v.iter().any(|e| e["reasoningEffort"] == "medium"))
            {
                return Err("当前账号不支持中等推理".into());
            }
            Ok(())
        })();
        match check {
            Ok(()) => AdapterStatus {
                id: "codex".into(),
                display_name: "Codex · 6.1 Sol · 中等".into(),
                available: true,
                capabilities,
                detail: Some("已连接本机 Codex app-server".into()),
            },
            Err(err) => AdapterStatus {
                id: "codex".into(),
                display_name: "Codex（连接未完成）".into(),
                available: false,
                capabilities,
                detail: Some(err),
            },
        }
    }

    fn start_turn(
        &self,
        req: TurnRequest,
        sink: Arc<dyn EventSink>,
    ) -> Result<StartedTurn, String> {
        let client = self.client(Some(sink))?;
        let thread = if let Some(existing) = &req.thread_id {
            client.rpc(
                "thread/resume",
                json!({
                    "threadId": existing,
                    "cwd": req.cwd,
                    "model": MODEL,
                    "approvalPolicy": "never",
                    "sandbox": "workspace-write",
                    "developerInstructions": req.developer_instructions
                }),
            )?
        } else {
            client.rpc(
                "thread/start",
                json!({
                    "cwd": req.cwd,
                    "model": MODEL,
                    "approvalPolicy": "never",
                    "sandbox": "workspace-write",
                    "developerInstructions": req.developer_instructions,
                    "config": {"model_reasoning_effort": "medium"},
                    "dynamicTools": req.dynamic_tools
                }),
            )?
        };
        let thread_id = thread["thread"]["id"]
            .as_str()
            .ok_or("Codex 未返回任务会话")?
            .to_string();
        let mut input: Vec<Value> = req
            .user_messages
            .into_iter()
            .map(|text| json!({"type": "text", "text": text, "text_elements": []}))
            .collect();
        if let Some((name, path)) = req.skill_attachment {
            if path.is_file() {
                input.insert(0, json!({"type": "skill", "name": name, "path": path}));
            }
        }
        let turn = client.rpc(
            "turn/start",
            json!({
                "threadId": thread_id,
                "model": MODEL,
                "effort": "medium",
                "input": input
            }),
        )?;
        Ok(StartedTurn {
            thread_id,
            turn_id: turn["turn"]["id"].as_str().map(str::to_string),
        })
    }

    fn cancel_turn(&self, thread_id: &str, turn_id: &str) -> Result<(), String> {
        self.client(None)?
            .rpc("turn/interrupt", json!({"threadId": thread_id, "turnId": turn_id}))?;
        Ok(())
    }

    fn shutdown(&self) {
        if let Ok(mut client) = self.client.lock() {
            if let Some(client) = client.take() {
                if let Ok(mut child) = client.child.lock() {
                    let _ = child.kill();
                }
            }
        }
    }
}
