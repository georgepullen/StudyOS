use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStderr, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    ThreadReady {
        thread_id: String,
    },
    ThreadStatusChanged {
        status: String,
    },
    TurnStarted {
        turn_id: String,
    },
    TurnCompleted {
        turn_id: String,
        status: String,
    },
    ItemStarted {
        turn_id: String,
        item: Value,
    },
    ItemCompleted {
        turn_id: String,
        item: Value,
    },
    AgentMessageDelta {
        turn_id: String,
        item_id: String,
        delta: String,
    },
    McpServerStatusUpdated {
        name: String,
        status: String,
    },
    Error {
        message: String,
    },
    Disconnected,
}

type PendingMap = Arc<Mutex<HashMap<u64, Sender<Result<Value, String>>>>>;

pub struct AppServerClient {
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
    next_id: AtomicU64,
    events: Receiver<RuntimeEvent>,
}

impl AppServerClient {
    pub fn spawn() -> Result<Self> {
        let mut child = Command::new("codex")
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to capture app-server stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture app-server stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("failed to capture app-server stderr"))?;

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let (event_tx, event_rx) = mpsc::channel();

        spawn_stdout_reader(stdout, Arc::clone(&pending), event_tx.clone());
        spawn_stderr_reader(stderr, event_tx);

        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            next_id: AtomicU64::new(1),
            events: event_rx,
        })
    }

    pub fn initialize(&self) -> Result<()> {
        let params = json!({
            "clientInfo": {
                "name": "studyos",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": null
        });

        let _ = self.send_request("initialize", params)?;
        self.send_notification("initialized", Value::Null)?;
        Ok(())
    }

    pub fn start_thread(&self, cwd: &Path, developer_instructions: &str) -> Result<String> {
        let params = json!({
            "cwd": cwd.display().to_string(),
            "approvalPolicy": "never",
            "experimentalRawEvents": false,
            "persistExtendedHistory": false,
            "developerInstructions": developer_instructions,
        });

        let result = self.send_request("thread/start", params)?;
        result
            .get("thread")
            .and_then(|thread| thread.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("thread/start response missing thread id"))
    }

    pub fn resume_thread(&self, thread_id: &str, cwd: &Path) -> Result<String> {
        let params = json!({
            "threadId": thread_id,
            "cwd": cwd.display().to_string(),
            "persistExtendedHistory": false,
        });

        let result = self.send_request("thread/resume", params)?;
        result
            .get("thread")
            .and_then(|thread| thread.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("thread/resume response missing thread id"))
    }

    pub fn start_structured_turn(
        &self,
        thread_id: &str,
        prompt: &str,
        output_schema: Value,
        cwd: &Path,
    ) -> Result<String> {
        let params = json!({
            "threadId": thread_id,
            "input": [
                {
                    "type": "text",
                    "text": prompt,
                    "text_elements": [],
                }
            ],
            "cwd": cwd.display().to_string(),
            "outputSchema": output_schema,
        });

        let result = self.send_request("turn/start", params)?;
        result
            .get("turn")
            .and_then(|turn| turn.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("turn/start response missing turn id"))
    }

    pub fn poll_events(&self) -> Vec<RuntimeEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.events.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn run_structured_turn_and_wait(
        &self,
        thread_id: &str,
        prompt: &str,
        output_schema: Value,
        cwd: &Path,
        timeout: Duration,
    ) -> Result<String> {
        let turn_id = self.start_structured_turn(thread_id, prompt, output_schema, cwd)?;
        let started_at = std::time::Instant::now();
        let mut text_buffers: HashMap<String, String> = HashMap::new();
        let mut completed_text: Option<String> = None;

        while started_at.elapsed() < timeout {
            let remaining = timeout.saturating_sub(started_at.elapsed());
            let event = self
                .events
                .recv_timeout(remaining.min(Duration::from_secs(1)))
                .map_err(|_| anyhow!("timed out waiting for structured turn completion"))?;

            match event {
                RuntimeEvent::AgentMessageDelta {
                    turn_id: event_turn_id,
                    item_id,
                    delta,
                } if event_turn_id == turn_id => {
                    text_buffers.entry(item_id).or_default().push_str(&delta);
                }
                RuntimeEvent::ItemCompleted {
                    turn_id: event_turn_id,
                    item,
                } if event_turn_id == turn_id
                    && item.get("type").and_then(Value::as_str) == Some("agentMessage") =>
                {
                    let item_id = item
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let fallback = item
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    completed_text = Some(text_buffers.remove(&item_id).unwrap_or(fallback));
                }
                RuntimeEvent::TurnCompleted {
                    turn_id: event_turn_id,
                    status,
                } if event_turn_id == turn_id => {
                    if status == "failed" {
                        return Err(anyhow!("structured turn {turn_id} failed"));
                    }
                    if let Some(text) = completed_text {
                        return Ok(text);
                    }
                }
                RuntimeEvent::Error { message } => {
                    return Err(anyhow!(message));
                }
                _ => {}
            }
        }

        Err(anyhow!("timed out waiting for structured turn payload"))
    }

    fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| anyhow!("pending request lock poisoned"))?
            .insert(id, tx);

        let message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        {
            let mut stdin = self
                .stdin
                .lock()
                .map_err(|_| anyhow!("app-server stdin lock poisoned"))?;
            writeln!(stdin, "{}", message)?;
            stdin.flush()?;
        }

        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(anyhow!(error)),
            Err(_) => Err(anyhow!(
                "timed out waiting for app-server response to {method}"
            )),
        }
    }

    fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        let message = if params.is_null() {
            json!({
                "jsonrpc": "2.0",
                "method": method,
            })
        } else {
            json!({
                "jsonrpc": "2.0",
                "method": method,
                "params": params,
            })
        };

        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| anyhow!("app-server stdin lock poisoned"))?;
        writeln!(stdin, "{}", message)?;
        stdin.flush()?;
        Ok(())
    }
}

impl Drop for AppServerClient {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn spawn_stdout_reader(
    stdout: std::process::ChildStdout,
    pending: PendingMap,
    event_tx: Sender<RuntimeEvent>,
) {
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => match serde_json::from_str::<Value>(&line) {
                    Ok(message) => {
                        if let Some(id) = message.get("id").and_then(Value::as_u64) {
                            let sender = pending.lock().ok().and_then(|mut map| map.remove(&id));
                            if let Some(sender) = sender {
                                if let Some(result) = message.get("result") {
                                    let _ = sender.send(Ok(result.clone()));
                                } else if let Some(error) = message.get("error") {
                                    let _ = sender.send(Err(error.to_string()));
                                }
                            }
                            continue;
                        }

                        if let Some(method) = message.get("method").and_then(Value::as_str) {
                            let params = message.get("params").cloned().unwrap_or(Value::Null);
                            if let Some(event) = map_notification(method, params) {
                                let _ = event_tx.send(event);
                            }
                        }
                    }
                    Err(error) => {
                        let _ = event_tx.send(RuntimeEvent::Error {
                            message: format!("failed to parse app-server message: {error}"),
                        });
                    }
                },
                Err(error) => {
                    let _ = event_tx.send(RuntimeEvent::Error {
                        message: format!("failed to read app-server stdout: {error}"),
                    });
                    break;
                }
            }
        }

        let _ = event_tx.send(RuntimeEvent::Disconnected);
    });
}

fn spawn_stderr_reader(stderr: ChildStderr, event_tx: Sender<RuntimeEvent>) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    let _ = event_tx.send(RuntimeEvent::Error {
                        message: format!("app-server stderr: {line}"),
                    });
                }
                Ok(_) => {}
                Err(error) => {
                    let _ = event_tx.send(RuntimeEvent::Error {
                        message: format!("failed to read app-server stderr: {error}"),
                    });
                    break;
                }
            }
        }
    });
}

fn map_notification(method: &str, params: Value) -> Option<RuntimeEvent> {
    match method {
        "thread/started" => params
            .get("thread")
            .and_then(|thread| thread.get("id"))
            .and_then(Value::as_str)
            .map(|thread_id| RuntimeEvent::ThreadReady {
                thread_id: thread_id.to_string(),
            }),
        "thread/status/changed" => params
            .get("status")
            .map(stringify_status)
            .map(|status| RuntimeEvent::ThreadStatusChanged { status }),
        "turn/started" => params
            .get("turn")
            .and_then(|turn| turn.get("id"))
            .and_then(Value::as_str)
            .map(|turn_id| RuntimeEvent::TurnStarted {
                turn_id: turn_id.to_string(),
            }),
        "turn/completed" => {
            let turn = params.get("turn")?;
            let turn_id = turn.get("id")?.as_str()?.to_string();
            let status = turn
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            Some(RuntimeEvent::TurnCompleted { turn_id, status })
        }
        "item/started" => Some(RuntimeEvent::ItemStarted {
            turn_id: params.get("turnId")?.as_str()?.to_string(),
            item: params.get("item")?.clone(),
        }),
        "item/completed" => Some(RuntimeEvent::ItemCompleted {
            turn_id: params.get("turnId")?.as_str()?.to_string(),
            item: params.get("item")?.clone(),
        }),
        "item/agentMessage/delta" => Some(RuntimeEvent::AgentMessageDelta {
            turn_id: params.get("turnId")?.as_str()?.to_string(),
            item_id: params.get("itemId")?.as_str()?.to_string(),
            delta: params.get("delta")?.as_str()?.to_string(),
        }),
        "mcpServer/startupStatus/updated" => Some(RuntimeEvent::McpServerStatusUpdated {
            name: params.get("name")?.as_str()?.to_string(),
            status: params.get("status")?.as_str()?.to_string(),
        }),
        "error" => Some(RuntimeEvent::Error {
            message: params.to_string(),
        }),
        _ => None,
    }
}

fn stringify_status(value: &Value) -> String {
    if let Some(kind) = value.get("type").and_then(Value::as_str) {
        return kind.to_string();
    }

    value.to_string()
}
