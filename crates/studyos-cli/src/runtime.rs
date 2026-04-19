use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Sender},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const RPC_TIMEOUT: Duration = Duration::from_secs(60);
const RUNTIME_LOG_CAPACITY: usize = 50;
const SERVER_REQUEST_REJECTION: &str =
    "StudyOS does not permit server-driven tool or approval requests.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    Disconnected {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordedServerLine {
    pub line: String,
}

#[derive(Debug)]
enum ParsedServerMessage {
    Response {
        id: u64,
        result: std::result::Result<Value, String>,
    },
    Notification(RuntimeEvent),
    Request {
        id: u64,
        method: String,
    },
    Ignored,
}

pub trait AppServerTransport: Send + Sync {
    fn initialize(&self) -> Result<()>;
    fn start_thread(&self, cwd: &Path, developer_instructions: &str) -> Result<String>;
    fn resume_thread(&self, thread_id: &str, cwd: &Path) -> Result<String>;
    fn start_structured_turn(
        &self,
        thread_id: &str,
        prompt: &str,
        output_schema: Value,
        cwd: &Path,
    ) -> Result<String>;
    fn poll_events(&self) -> Vec<RuntimeEvent>;
    fn runtime_log_lines(&self) -> Vec<String>;
}

type PendingMap = Arc<Mutex<HashMap<u64, Sender<std::result::Result<Value, String>>>>>;
type EventQueue = Arc<Mutex<VecDeque<RuntimeEvent>>>;
type RuntimeLog = Arc<Mutex<VecDeque<String>>>;
type EventLog = Option<Arc<Mutex<File>>>;

pub struct CodexAppServerTransport {
    child: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
    next_id: AtomicU64,
    events: EventQueue,
    runtime_log: RuntimeLog,
}

impl CodexAppServerTransport {
    pub fn spawn() -> Result<Arc<dyn AppServerTransport>> {
        Self::spawn_with_log_path(None)
    }

    pub fn spawn_with_log_path(log_path: Option<PathBuf>) -> Result<Arc<dyn AppServerTransport>> {
        let mut child = Command::new("codex")
            .arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn `codex app-server --listen stdio://`")?;

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
        let events: EventQueue = Arc::new(Mutex::new(VecDeque::new()));
        let runtime_log: RuntimeLog = Arc::new(Mutex::new(VecDeque::new()));
        let event_log = open_event_log(log_path)?;
        let stdin = Arc::new(Mutex::new(stdin));

        spawn_stdout_reader(
            stdout,
            Arc::clone(&stdin),
            Arc::clone(&pending),
            Arc::clone(&events),
            Arc::clone(&runtime_log),
            event_log.clone(),
        );
        spawn_stderr_reader(
            stderr,
            Arc::clone(&events),
            Arc::clone(&runtime_log),
            event_log,
        );

        Ok(Arc::new(Self {
            child: Arc::new(Mutex::new(Some(child))),
            stdin,
            pending,
            next_id: AtomicU64::new(1),
            events,
            runtime_log,
        }))
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
        self.write_message(&message)?;

        match rx.recv_timeout(RPC_TIMEOUT) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(anyhow!(error)),
            Err(_) => {
                let _ = self.pending.lock().map(|mut pending| pending.remove(&id));
                Err(anyhow!(
                    "timed out waiting for app-server response to {method}"
                ))
            }
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

        self.write_message(&message)
    }

    fn write_message(&self, message: &Value) -> Result<()> {
        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| anyhow!("app-server stdin lock poisoned"))?;
        writeln!(stdin, "{message}")?;
        stdin.flush()?;
        Ok(())
    }
}

impl AppServerTransport for CodexAppServerTransport {
    fn initialize(&self) -> Result<()> {
        let params = json!({
            "clientInfo": {
                "name": "studyos",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "experimentalApi": true
            }
        });

        let _ = self.send_request("initialize", params)?;
        self.send_notification("initialized", Value::Null)?;
        Ok(())
    }

    fn start_thread(&self, cwd: &Path, developer_instructions: &str) -> Result<String> {
        let params = json!({
            "cwd": cwd.display().to_string(),
            "approvalPolicy": "never",
            "sandbox": "workspace-write",
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

    fn resume_thread(&self, thread_id: &str, cwd: &Path) -> Result<String> {
        let params = json!({
            "threadId": thread_id,
            "cwd": cwd.display().to_string(),
            "approvalPolicy": "never",
            "sandbox": "workspace-write",
        });

        let result = self.send_request("thread/resume", params)?;
        result
            .get("thread")
            .and_then(|thread| thread.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("thread/resume response missing thread id"))
    }

    fn start_structured_turn(
        &self,
        thread_id: &str,
        prompt: &str,
        output_schema: Value,
        cwd: &Path,
    ) -> Result<String> {
        let params = json!({
            "threadId": thread_id,
            "cwd": cwd.display().to_string(),
            "sandboxPolicy": {
                "type": "workspaceWrite",
                "networkAccess": true,
                "excludeTmpdirEnvVar": false,
                "writableRoots": []
            },
            "input": [
                {
                    "type": "text",
                    "text": prompt,
                    "text_elements": [],
                }
            ],
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

    fn poll_events(&self) -> Vec<RuntimeEvent> {
        if let Ok(mut events) = self.events.lock() {
            return events.drain(..).collect();
        }

        vec![RuntimeEvent::Error {
            message: "runtime event queue lock poisoned".to_string(),
        }]
    }

    fn runtime_log_lines(&self) -> Vec<String> {
        self.runtime_log
            .lock()
            .map(|lines| lines.iter().cloned().collect())
            .unwrap_or_else(|_| vec!["runtime log unavailable".to_string()])
    }
}

impl Drop for CodexAppServerTransport {
    fn drop(&mut self) {
        drain_pending_requests(&self.pending, "app-server transport closed");
        if let Ok(mut child_guard) = self.child.lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

pub struct ReplayAppServerTransport {
    lines: Mutex<VecDeque<String>>,
    events: EventQueue,
    runtime_log: RuntimeLog,
}

impl ReplayAppServerTransport {
    pub fn from_fixture(path: &Path) -> Result<Arc<dyn AppServerTransport>> {
        let mut lines = VecDeque::new();
        for raw in fs::read_to_string(path)?.lines() {
            let entry = serde_json::from_str::<RecordedServerLine>(raw)?;
            lines.push_back(entry.line);
        }

        Ok(Arc::new(Self {
            lines: Mutex::new(lines),
            events: Arc::new(Mutex::new(VecDeque::new())),
            runtime_log: Arc::new(Mutex::new(VecDeque::new())),
        }))
    }

    fn consume_until_response(&self, method: &str) -> Result<Value> {
        loop {
            let line = self
                .lines
                .lock()
                .map_err(|_| anyhow!("replay fixture lock poisoned"))?
                .pop_front()
                .ok_or_else(|| anyhow!("replay fixture exhausted before response to {method}"))?;

            match parse_server_message(&line)? {
                ParsedServerMessage::Response { result, .. } => {
                    return result.map_err(anyhow::Error::msg);
                }
                ParsedServerMessage::Notification(event) => {
                    push_event(&self.events, event);
                }
                ParsedServerMessage::Request { method, .. } => {
                    push_runtime_log(
                        &self.runtime_log,
                        format!("replay ignored server request: {method}"),
                    );
                }
                ParsedServerMessage::Ignored => {}
            }
        }
    }

    fn pump_notifications(&self) {
        let mut pumped = 0usize;
        while pumped < 16 {
            let next = self
                .lines
                .lock()
                .ok()
                .and_then(|mut lines| lines.pop_front());
            let Some(line) = next else {
                break;
            };

            match parse_server_message(&line) {
                Ok(ParsedServerMessage::Notification(event)) => {
                    push_event(&self.events, event);
                    pumped += 1;
                }
                Ok(ParsedServerMessage::Request { method, .. }) => {
                    push_runtime_log(
                        &self.runtime_log,
                        format!("replay ignored server request: {method}"),
                    );
                }
                Ok(ParsedServerMessage::Ignored | ParsedServerMessage::Response { .. }) => {}
                Err(error) => {
                    push_event(
                        &self.events,
                        RuntimeEvent::Error {
                            message: format!("failed to parse replay fixture line: {error}"),
                        },
                    );
                    pumped += 1;
                }
            }
        }
    }
}

impl AppServerTransport for ReplayAppServerTransport {
    fn initialize(&self) -> Result<()> {
        let _ = self.consume_until_response("initialize")?;
        Ok(())
    }

    fn start_thread(&self, _cwd: &Path, _developer_instructions: &str) -> Result<String> {
        let result = self.consume_until_response("thread/start")?;
        result
            .get("thread")
            .and_then(|thread| thread.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("replay thread/start response missing thread id"))
    }

    fn resume_thread(&self, _thread_id: &str, _cwd: &Path) -> Result<String> {
        let result = self.consume_until_response("thread/resume")?;
        result
            .get("thread")
            .and_then(|thread| thread.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("replay thread/resume response missing thread id"))
    }

    fn start_structured_turn(
        &self,
        _thread_id: &str,
        _prompt: &str,
        _output_schema: Value,
        _cwd: &Path,
    ) -> Result<String> {
        let result = self.consume_until_response("turn/start")?;
        result
            .get("turn")
            .and_then(|turn| turn.get("id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("replay turn/start response missing turn id"))
    }

    fn poll_events(&self) -> Vec<RuntimeEvent> {
        self.pump_notifications();
        if let Ok(mut events) = self.events.lock() {
            return events.drain(..).collect();
        }

        vec![RuntimeEvent::Error {
            message: "replay event queue lock poisoned".to_string(),
        }]
    }

    fn runtime_log_lines(&self) -> Vec<String> {
        self.runtime_log
            .lock()
            .map(|lines| lines.iter().cloned().collect())
            .unwrap_or_default()
    }
}

pub fn capture_runtime_fixture(
    cwd: &Path,
    developer_instructions: &str,
    opening_prompt: &str,
    output_schema: Value,
    fixture_path: &Path,
    stderr_log_path: &Path,
) -> Result<()> {
    let mut child = Command::new("codex")
        .arg("app-server")
        .arg("--listen")
        .arg("stdio://")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn codex app-server for fixture capture")?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("fixture capture failed to capture stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("fixture capture failed to capture stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("fixture capture failed to capture stderr"))?;

    let stderr_lines = Arc::new(Mutex::new(Vec::<String>::new()));
    {
        let stderr_lines = Arc::clone(&stderr_lines);
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    if let Ok(mut buffer) = stderr_lines.lock() {
                        buffer.push(line);
                    }
                }
            }
        });
    }

    let mut reader = BufReader::new(stdout);
    let mut raw_lines = Vec::new();

    write_rpc_request(
        &mut stdin,
        1,
        "initialize",
        json!({
            "clientInfo": {
                "name": "studyos",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "experimentalApi": true
            }
        }),
    )?;
    raw_lines.push(read_rpc_line(&mut reader)?);
    write_rpc_notification(&mut stdin, "initialized", Value::Null)?;

    write_rpc_request(
        &mut stdin,
        2,
        "thread/start",
        json!({
            "cwd": cwd.display().to_string(),
            "approvalPolicy": "never",
            "sandbox": "workspace-write",
            "developerInstructions": developer_instructions,
        }),
    )?;
    let mut thread_id = None;
    while thread_id.is_none() {
        let line = read_rpc_line(&mut reader)?;
        if let Ok(ParsedServerMessage::Response { id: 2, result }) = parse_server_message(&line) {
            let value = result.map_err(anyhow::Error::msg)?;
            thread_id = value
                .get("thread")
                .and_then(|thread| thread.get("id"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
        }
        raw_lines.push(line);
    }
    let thread_id = thread_id.ok_or_else(|| anyhow!("fixture capture missing thread id"))?;

    write_rpc_request(
        &mut stdin,
        3,
        "turn/start",
        json!({
            "threadId": thread_id,
            "cwd": cwd.display().to_string(),
            "sandboxPolicy": {
                "type": "workspaceWrite",
                "networkAccess": true,
                "excludeTmpdirEnvVar": false,
                "writableRoots": []
            },
            "input": [
                {
                    "type": "text",
                    "text": opening_prompt,
                    "text_elements": [],
                }
            ],
            "outputSchema": output_schema,
        }),
    )?;

    let mut turn_id = None;
    let mut turn_complete = false;
    while !turn_complete {
        let line = read_rpc_line(&mut reader)?;
        if turn_id.is_none() {
            if let Ok(ParsedServerMessage::Response { id: 3, result }) = parse_server_message(&line)
            {
                let value = result.map_err(anyhow::Error::msg)?;
                turn_id = value
                    .get("turn")
                    .and_then(|turn| turn.get("id"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
            }
        } else if let Ok(ParsedServerMessage::Notification(RuntimeEvent::TurnCompleted {
            turn_id: event_turn_id,
            ..
        })) = parse_server_message(&line)
        {
            if Some(event_turn_id.as_str()) == turn_id.as_deref() {
                turn_complete = true;
            }
        }
        raw_lines.push(line);
    }

    if let Some(parent) = fixture_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = stderr_log_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut serialized = String::new();
    for line in raw_lines {
        serialized.push_str(&serde_json::to_string(&RecordedServerLine { line })?);
        serialized.push('\n');
    }
    fs::write(fixture_path, serialized)?;

    let stderr_output = stderr_lines
        .lock()
        .map(|lines| lines.join("\n"))
        .unwrap_or_default();
    fs::write(stderr_log_path, stderr_output)?;

    let _ = child.kill();
    let _ = child.wait();

    Ok(())
}

fn spawn_stdout_reader(
    stdout: std::process::ChildStdout,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
    events: EventQueue,
    runtime_log: RuntimeLog,
    event_log: EventLog,
) {
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    write_event_log(
                        &event_log,
                        json!({
                            "stream": "stdout",
                            "line": line,
                        }),
                    );
                    match parse_server_message(&line) {
                        Ok(ParsedServerMessage::Response { id, result }) => {
                            let sender = pending.lock().ok().and_then(|mut map| map.remove(&id));
                            if let Some(sender) = sender {
                                let _ = sender.send(result);
                            }
                        }
                        Ok(ParsedServerMessage::Notification(event)) => {
                            write_event_log(
                                &event_log,
                                json!({
                                    "stream": "event",
                                    "event": runtime_event_name(&event),
                                    "payload": event,
                                }),
                            );
                            push_event(&events, event);
                        }
                        Ok(ParsedServerMessage::Request { id, method }) => {
                            push_runtime_log(
                                &runtime_log,
                                format!("rejected server request `{method}`"),
                            );
                            write_event_log(
                                &event_log,
                                json!({
                                    "stream": "request",
                                    "id": id,
                                    "method": method,
                                }),
                            );
                            reject_server_request(&stdin, id, &method);
                            push_event(
                                &events,
                                RuntimeEvent::Error {
                                    message: format!("{SERVER_REQUEST_REJECTION} ({method})"),
                                },
                            );
                        }
                        Ok(ParsedServerMessage::Ignored) => {}
                        Err(error) => {
                            push_event(
                                &events,
                                RuntimeEvent::Error {
                                    message: format!("failed to parse app-server message: {error}"),
                                },
                            );
                        }
                    }
                }
                Err(error) => {
                    write_event_log(
                        &event_log,
                        json!({
                            "stream": "stdout_error",
                            "message": error.to_string(),
                        }),
                    );
                    push_event(
                        &events,
                        RuntimeEvent::Error {
                            message: format!("failed to read app-server stdout: {error}"),
                        },
                    );
                    break;
                }
            }
        }

        drain_pending_requests(&pending, "app-server disconnected");
        push_event(
            &events,
            RuntimeEvent::Disconnected {
                message: "Codex app-server stdout closed".to_string(),
            },
        );
        write_event_log(
            &event_log,
            json!({
                "stream": "lifecycle",
                "event": "stdout_closed",
            }),
        );
    });
}

fn spawn_stderr_reader(
    stderr: ChildStderr,
    events: EventQueue,
    runtime_log: RuntimeLog,
    event_log: EventLog,
) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    write_event_log(
                        &event_log,
                        json!({
                            "stream": "stderr",
                            "line": line,
                        }),
                    );
                    push_runtime_log(&runtime_log, line.clone());
                    push_event(
                        &events,
                        RuntimeEvent::Error {
                            message: format!("app-server stderr: {line}"),
                        },
                    );
                }
                Ok(_) => {}
                Err(error) => {
                    write_event_log(
                        &event_log,
                        json!({
                            "stream": "stderr_error",
                            "message": error.to_string(),
                        }),
                    );
                    push_event(
                        &events,
                        RuntimeEvent::Error {
                            message: format!("failed to read app-server stderr: {error}"),
                        },
                    );
                    break;
                }
            }
        }
    });
}

fn open_event_log(path: Option<PathBuf>) -> Result<EventLog> {
    let Some(path) = path else {
        return Ok(None);
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    Ok(Some(Arc::new(Mutex::new(File::create(path)?))))
}

fn write_event_log(event_log: &EventLog, payload: Value) {
    let Some(file) = event_log else {
        return;
    };

    let envelope = json!({
        "ts_unix_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0),
        "payload": payload,
    });

    if let Ok(mut file) = file.lock() {
        let _ = writeln!(file, "{envelope}");
    }
}

fn runtime_event_name(event: &RuntimeEvent) -> &'static str {
    match event {
        RuntimeEvent::ThreadReady { .. } => "thread_ready",
        RuntimeEvent::ThreadStatusChanged { .. } => "thread_status_changed",
        RuntimeEvent::TurnStarted { .. } => "turn_started",
        RuntimeEvent::TurnCompleted { .. } => "turn_completed",
        RuntimeEvent::ItemStarted { .. } => "item_started",
        RuntimeEvent::ItemCompleted { .. } => "item_completed",
        RuntimeEvent::AgentMessageDelta { .. } => "agent_message_delta",
        RuntimeEvent::McpServerStatusUpdated { .. } => "mcp_server_status_updated",
        RuntimeEvent::Error { .. } => "error",
        RuntimeEvent::Disconnected { .. } => "disconnected",
    }
}

fn parse_server_message(line: &str) -> Result<ParsedServerMessage> {
    let message = serde_json::from_str::<Value>(line)?;
    if let Some(id) = message.get("id").and_then(Value::as_u64) {
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            return Ok(ParsedServerMessage::Request {
                id,
                method: method.to_string(),
            });
        }

        if let Some(result) = message.get("result") {
            return Ok(ParsedServerMessage::Response {
                id,
                result: Ok(result.clone()),
            });
        }

        if let Some(error) = message.get("error") {
            let detail = error
                .get("message")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| error.to_string());
            return Ok(ParsedServerMessage::Response {
                id,
                result: Err(detail),
            });
        }
    }

    if let Some(method) = message.get("method").and_then(Value::as_str) {
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        return map_notification(method, params)
            .map(ParsedServerMessage::Notification)
            .ok_or_else(|| anyhow!("unrecognized notification method `{method}`"));
    }

    Ok(ParsedServerMessage::Ignored)
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

fn reject_server_request(stdin: &Arc<Mutex<ChildStdin>>, id: u64, method: &str) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32000,
            "message": format!("{SERVER_REQUEST_REJECTION} ({method})"),
        }
    });

    if let Ok(mut stdin) = stdin.lock() {
        let _ = writeln!(stdin, "{response}");
        let _ = stdin.flush();
    }
}

fn drain_pending_requests(pending: &PendingMap, message: &str) {
    if let Ok(mut pending) = pending.lock() {
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err(message.to_string()));
        }
    }
}

fn push_event(events: &EventQueue, event: RuntimeEvent) {
    if let Ok(mut events) = events.lock() {
        events.push_back(event);
    }
}

fn push_runtime_log(runtime_log: &RuntimeLog, line: String) {
    if let Ok(mut runtime_log) = runtime_log.lock() {
        if runtime_log.len() >= RUNTIME_LOG_CAPACITY {
            runtime_log.pop_front();
        }
        runtime_log.push_back(line);
    }
}

fn write_rpc_request(stdin: &mut ChildStdin, id: u64, method: &str, params: Value) -> Result<()> {
    let message = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    writeln!(stdin, "{message}")?;
    stdin.flush()?;
    Ok(())
}

fn write_rpc_notification(stdin: &mut ChildStdin, method: &str, params: Value) -> Result<()> {
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
    writeln!(stdin, "{message}")?;
    stdin.flush()?;
    Ok(())
}

fn read_rpc_line(reader: &mut BufReader<std::process::ChildStdout>) -> Result<String> {
    let mut line = String::new();
    let bytes = reader.read_line(&mut line)?;
    if bytes == 0 {
        return Err(anyhow!("app-server closed stdout during fixture capture"));
    }

    Ok(line.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::{ParsedServerMessage, RuntimeEvent, parse_server_message};

    #[test]
    fn parse_notification_line_maps_into_runtime_event() {
        let line = r#"{"jsonrpc":"2.0","method":"turn/completed","params":{"turn":{"id":"turn_123","status":"completed"}}}"#;
        let parsed = parse_server_message(line).unwrap_or_else(|err| panic!("parse failed: {err}"));

        match parsed {
            ParsedServerMessage::Notification(RuntimeEvent::TurnCompleted { turn_id, status }) => {
                assert_eq!(turn_id, "turn_123");
                assert_eq!(status, "completed");
            }
            other => panic!("unexpected parsed message: {other:?}"),
        }
    }
}
