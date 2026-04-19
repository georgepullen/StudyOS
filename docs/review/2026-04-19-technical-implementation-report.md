# StudyOS Technical Implementation Report

Date: 2026-04-19
Repository: `StudyOS`
Commit range covered: `fc25bdb` .. `802e352`

Historical scope note: this report captures the implementation state through `802e352`. Later hardening work, including protocol vendoring, misconception staging, per-course runtime threads, course-scoped recap lookup, and the alpha-readiness pass, landed after this report was drafted.

## Purpose of this report

This is a technical write-up of what was implemented in the repository, how the architecture changed over time, what was actually verified, and what remains outside the current build. It is not intended as a product pitch.

The focus here is:

- what code now exists
- what the runtime and data flow look like
- what evidence exists that the system works
- what parts of the original product direction are only partially implemented
- which code paths are most worth external review

The report ends with a Repomix appendix. The appendix is intentionally constrained to a `14,999` token code pack so it fits the requested limit. Because of that cap, it does not include the entire repo. It includes the two files I think are most important for an external reviewer to inspect first:

- `crates/studyos-cli/src/runtime.rs`
- `crates/studyos-core/src/store.rs`

Those two files cover the most failure-prone parts of the current system: the Codex app-server harness and the SQLite-backed study evidence model.

## Repository progression

### 1. Initial bootstrap

The repository started as an empty Rust workspace bootstrap.

Relevant commits:

- `fc25bdb` `Bootstrap StudyOS workspace`
- `595c0d7` `Add V1 implementation planning docs`

What was added at that stage:

- Cargo workspace
- public-repo-safe ignore rules
- CI scaffold
- README and implementation planning docs
- initial crate split between `studyos-cli` and `studyos-core`

At that point the repo was mostly scaffolding and documentation. There was no real tutor harness yet.

### 2. First functioning shell and local context

Relevant commit:

- `eab896b` `Build TUI bootstrap and local study context`

This introduced the first actual application shell:

- full-screen terminal UI
- header, transcript, panels, and structured answer widgets
- initial local data loading for deadlines, timetable, and materials manifest
- SQLite bootstrap and local state persistence

The system still relied heavily on local placeholder content at this stage.

### 3. First Codex app-server integration

Relevant commit:

- `cb2700f` `Integrate Codex app-server tutor runtime`

This was the first point where the client attempted to use Codex app-server as a real turn engine.

What existed after this slice:

- `initialize`
- thread start/resume
- structured tutor output schema
- streamed event handling
- rendering of structured tutor payloads into the TUI

However, this phase still lacked proof that the transport and schema contract actually worked end-to-end.

### 4. Evidence persistence and study-memory loop

Relevant commits:

- `e6c739a` `Persist session evidence and misconception state`
- `ba75bf6` `Test tutor evaluation persistence path`

This phase connected tutor responses to the SQLite evidence model.

What was added:

- attempt persistence
- misconception persistence and re-surfacing
- concept review scheduling updates
- session outcome summaries
- tests for evaluation-to-database persistence

This made the app more than a renderer, but the runtime proof was still incomplete.

### 5. Planning and context routing

Relevant commits:

- `2a70ef4` `Route opening sessions from local study history`
- `32b8e26` `Add session recap persistence and exit review`
- `325ab17` `Improve structured math widget ergonomics`
- `4b773a0` `Add local deadline and course management commands`
- `fca585c` `Make bootstrap sessions course-aware`
- `2a023fa` `Add local materials search and prompt context`
- `83c0513` `Add local timetable commands and prompt context`

This group of changes pushed the system beyond a generic chat shell.

Capabilities added here:

- opening-mode routing (`Study`, `Review`, `Drill`) from local history
- recap flow and exit review
- course-specific startup behavior
- local deadlines CLI
- local timetable CLI
- local materials search and prompt injection
- structured input refinements for matrix and reasoning widgets

At this point the app had a lot of product surface, but the harness still needed hardening.

### 6. Roadmap closure and harness hardening

Relevant commits:

- `e81540f` `Pin toolchain and tighten build gates`
- `3cecd42` `Complete StudyOS runtime harness`
- `802e352` `Add roadmap closure evidence`

This was the most important technical phase.

This is where the code moved from “featureful prototype” to “verified harness.”

The main changes were:

- pinning Rust to `1.88.0`
- making workspace checks reproducible
- introducing a transport seam around Codex app-server
- recording a real runtime fixture
- adding replay and live integration tests
- migrating widget-draft persistence from TOML to versioned JSON
- making exit recap asynchronous
- handling disconnect and reconnect explicitly
- adding runtime log buffering and a runtime-log panel
- moving schema setup to versioned SQL migrations
- adding property tests for mastery/retrieval arithmetic
- implementing incremental materials ingestion from `.studyos/materials/raw/`
- deriving study windows from timetable and deadlines
- adding onboarding and diagnostics commands
- producing reviewer-facing logs and roadmap-closure docs

## Current system anatomy

### Crate layout

The current workspace is centered around two crates.

`crates/studyos-cli`

This contains:

- TUI rendering
- event loop
- keyboard handling
- app-server runtime transport
- CLI commands
- high-level application state transitions

`crates/studyos-core`

This contains:

- typed content blocks and widgets
- session-planning and startup-routing logic
- config and local data paths
- materials ingestion
- SQLite store and migrations
- tutor payload models

This separation is useful. The CLI crate is still large, but there is a meaningful boundary between UI/runtime and durable study-state logic.

### Runtime contract

The current runtime path is:

1. spawn `codex app-server`
2. `initialize`
3. `thread/start` or `thread/resume`
4. `turn/start` with a schema-constrained output request
5. stream notifications into `RuntimeEvent`
6. assemble agent-message payloads
7. parse `TutorTurnPayload`
8. render content blocks and widgets
9. persist any evaluation into SQLite

The app no longer depends on the old assumption that the transport will behave exactly as requested.

The important hardening changes are:

- a trait seam: `AppServerTransport`
- a real live transport: `CodexAppServerTransport`
- a replay transport for deterministic tests: `ReplayAppServerTransport`
- explicit rejection of server-driven approvals/tool calls
- retry path for structured payload parse failures
- non-blocking recap handling
- reconnect support after disconnect
- runtime JSONL logging support via `--log-json`

### Structured answering

V1 is no longer free-text only.

The implemented structured answer types are:

- matrix grid
- working + final answer
- step list
- short retrieval response

The widget layer is native client state, not just prompt formatting. That means:

- keyboard routing lives in the client
- widgets have validation rules
- draft state is resumable
- answer submission is serialized into structured prompt context

The report is deliberately not claiming these widgets are yet “paper-fast” in every case. What is true is that the code now treats structured answering as a first-class part of the tutor harness.

### Persistence and evidence model

The SQLite layer now has:

- versioned migrations
- a `meta` schema version table
- resume-state persistence
- sessions table
- attempts table
- misconceptions table
- concept-state table
- recap persistence

The current evidence loop is:

1. a question is shown
2. a structured answer is submitted
3. tutor evaluation is requested in schema-constrained JSON
4. evaluation is stored as an attempt
5. misconception is upserted if present
6. concept-state metrics are updated
7. due review urgency is affected by the outcome

There are now explicit tests for:

- legacy schema upgrade
- rejecting newer unsupported DB versions
- misconception deduplication
- mastery/ease invariants under random attempt sequences
- attempt logging updates reviews and misconceptions

### Materials ingestion

The materials system is no longer only a stub manifest.

Current ingestion contract:

- raw user files live under `.studyos/materials/raw/`
- generated artifacts live under `.studyos/materials/index/`
- manifest is written to `.studyos/materials/manifest.json`
- concept tags are written to `.studyos/materials/concepts.json`

Current supported inputs:

- `.md`
- `.txt`
- `.tex`
- `.pdf`

Current unsupported-but-explicitly-skipped inputs:

- `.docx`
- `.pptx`
- `.odt`

Important design choice:

The tutor prompt is not fed raw uploaded material wholesale. Instead, ingestion produces distilled metadata:

- title
- course inference
- topic tags
- snippet
- source hash
- source modification time

That means the current system already follows the design principle that course materials are conditioning context, not content to be replayed verbatim.

### Temporal awareness

Temporal awareness exists at a basic but real level.

The system currently uses:

- current local time
- local deadlines
- local timetable slots
- due reviews from SQLite
- recent misconceptions
- a derived `StudyWindow`

Current window sources:

- `timetable_gap`
- `before_deadline`
- `evening_block`

This is enough for the opening planner to produce materially different openings for short and long study windows.

What it does not do yet:

- full calendar integration
- sophisticated long-horizon scheduling
- automatic session-booking across a real calendar backend
- dynamic re-planning mid-session based on a moving external schedule

So temporal awareness exists, but it is local and heuristic, not yet a general scheduling engine.

## What has been verified

The most important distinction at this point is between code existence and runtime proof.

### Verified by repeatable local commands

These commands were run successfully on the current code:

- `./scripts/check.sh`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `STUDYOS_DATA_DIR=$(mktemp -d) cargo run -p studyos-cli -- doctor`
- `cargo run -p studyos-cli -- tour`
- `STUDYOS_CODEX_AVAILABLE=1 cargo test -p studyos-cli --test runtime_live -- --ignored --nocapture`
- `cargo run -p studyos-cli --example record_runtime_session`

### Verified by live runtime tests

The live runtime tests now prove two separate things:

1. the app reaches the first tutor-generated structured question through a real local `codex app-server`
2. a structured submission can be sent back, graded, and persisted into SQLite as attempt evidence

That second proof matters more than the first one. It verifies that the system is not just streaming tutor text; it is closing the evidence loop.

### Verified by recorded runtime fixture

A real fixture exists at:

- `crates/studyos-cli/tests/fixtures/runtime/opening-turn.jsonl`

That fixture is used by replay tests so the transport and parser contract can be regression-tested without requiring a live model each time.

## What is still incomplete or limited

This is not a claim of completion against the full original product specification. It is a claim that the harness is now functioning.

### Not yet implemented

The following remain out of the current build or only partially addressed:

- dedicated MCP server suite
- OCR or image-only PDF support
- full slide mode / teaching deck mode
- live calendar integrations
- rich graphics math rendering beyond the current TUI path
- OS-level focus-mode enforcement
- embeddings-based materials retrieval
- dedicated scratch-work editor beyond current widget/scratchpad model

### Structural weakness that still exists

The biggest remaining code-quality issue is that `crates/studyos-cli/src/app.rs` is still too large. A lot of harness behavior is correct now, but that file remains a concentration point for:

- TUI state
- runtime event interpretation
- tutor-prompt construction
- evidence persistence triggers
- quit/reconnect behavior

That does not block correctness, but it is the next obvious refactor target.

### Schema sensitivity still matters

Even with stricter schemas and retries, the system still depends on the model conforming to a fairly rigid structured output contract. The harness is better now at diagnosing and containing drift, but it still has a meaningful coupling to model behavior.

## Assessment of the harness

### Guardrails versus autonomy

The current harness is not a fully open-ended agent shell.

It constrains the model in several ways:

- opening turns must fit a typed session-plan/question schema
- submission turns must return a non-null evaluation object
- recap turns use a separate recap schema
- the client enforces structured answer modes
- the runtime auto-rejects server-driven tool/approval requests
- local memory and temporal context are supplied in a fixed shape

That means the model is not being asked to invent the workflow from scratch. It is operating inside a fairly opinionated tutor runtime.

I think that is the correct tradeoff for this project.

The system is not trying to maximize model freedom. It is trying to make a terminal tutor behave consistently enough to be useful for repeated self-study.

At the same time, the harness is not fully over-scripted. The model still has meaningful discretion over:

- question selection
- the exact wording of feedback
- when to repair versus transfer
- how to use the distilled local materials context
- how to shape the session plan within the provided window and mode

So the current state is neither “fully autonomous agent” nor “hard-coded lesson tree.” It is a constrained generative runtime.

### Temporal planning quality

The system now has enough temporal context to alter behavior based on the student’s local circumstances. That is materially different from plain Codex chat.

What is currently real:

- deadline pressure influences startup mode
- due reviews influence startup mode
- timetable gaps and local evening blocks produce different window descriptions
- those window descriptions are injected into the opening tutor prompt

What is not yet real:

- fine-grained adaptive rescheduling during a long session
- automatic weekly study-plan construction across all course commitments

So the current temporal layer is useful, but not yet sophisticated.

### Materials-awareness quality

The repo now has the correct high-level shape for the “upload all study materials, then generate fresh work from them” idea.

That statement is only partly true in current implementation, so to be precise:

What exists now:

- a dedicated raw materials folder
- incremental ingestion
- course inference and concept-tag derivation
- prompt injection of distilled snippets rather than raw text

What still needs work for the full vision:

- stronger concept extraction
- better linking from current tutor question back to ingested course coverage
- more reliable handling of mixed-format lecture slides and notes
- better guarantees that generated questions stay aligned with what the student has already covered

The current build establishes the pipeline. It does not yet solve the curriculum-alignment problem completely.

## Files most worth external review

If an external reviewer is time-constrained, I would start them with these files in roughly this order:

1. `crates/studyos-cli/src/runtime.rs`
   Why: this is the trust boundary with `codex app-server`

2. `crates/studyos-core/src/store.rs`
   Why: this is the trust boundary for evidence persistence and review scheduling

3. `crates/studyos-cli/src/app.rs`
   Why: this is where a lot of runtime behavior is orchestrated, but it is too large to fit cleanly in the appendix under the token cap

4. `crates/studyos-core/src/local_data.rs`
   Why: this contains the materials ingestion and local context path

5. `crates/studyos-core/src/session.rs`
   Why: this contains the startup routing and study-window logic

Because of the 15k token ceiling, only the first two are included in the code pack below.

## Repomix appendix methodology

Repomix reference used:

- Context7 library: `/yamadashy/repomix`

Command used:

```bash
npx repomix \
  --style markdown \
  -o /tmp/studyos-codepack.md \
  --include "crates/studyos-cli/src/runtime.rs,crates/studyos-core/src/store.rs" \
  --token-count-tree \
  --remove-comments \
  --remove-empty-lines \
  --no-file-summary \
  --no-directory-structure
```

Repomix output summary:

- total files: `2`
- total tokens: `14,999`
- security check: passed

## Repomix Appendix

# Files

## File: crates/studyos-cli/src/runtime.rs
```rust
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
```

## File: crates/studyos-core/src/store.rs
```rust
use std::{
    hash::{Hash, Hasher},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};
use crate::SessionRecapSummary;
const LATEST_SCHEMA_VERSION: i64 = 2;
const META_SCHEMA_VERSION_KEY: &str = "schema_version";
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../migrations/0001_initial.sql")),
    (
        2,
        include_str!("../migrations/0002_resume_thread_and_recap.sql"),
    ),
];
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppStats {
    pub due_reviews: usize,
    pub upcoming_deadlines: usize,
    pub total_attempts: usize,
    pub total_sessions: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeStateRecord {
    pub session_id: String,
    pub runtime_thread_id: Option<String>,
    pub active_mode: String,
    pub active_question_id: Option<String>,
    pub focused_panel: String,
    pub draft_payload: String,
    pub scratchpad_text: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: String,
    pub planned_minutes: u16,
    pub mode: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptRecord {
    pub id: String,
    pub session_id: String,
    pub concept_id: String,
    pub question_type: String,
    pub prompt_hash: String,
    pub student_answer: String,
    pub correctness: String,
    pub latency_ms: i64,
    pub reasoning_quality: String,
    pub feedback_summary: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptAuditRecord {
    pub id: String,
    pub session_id: String,
    pub concept_id: String,
    pub question_type: String,
    pub correctness: String,
    pub reasoning_quality: String,
    pub latency_ms: i64,
    pub feedback_summary: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MisconceptionInput {
    pub concept_id: String,
    pub error_type: String,
    pub description: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueReviewSummary {
    pub concept_id: String,
    pub concept_name: String,
    pub next_review_at: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MisconceptionSummary {
    pub concept_name: String,
    pub error_type: String,
    pub description: String,
    pub last_seen_at: String,
    pub evidence_count: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecapRecord {
    pub session_id: String,
    pub recap: SessionRecapSummary,
}
#[derive(Debug, Clone, Copy)]
struct ConceptStateSnapshot {
    mastery_estimate: f64,
    retrieval_strength: f64,
    stability_days: f64,
    ease_factor: f64,
}
#[derive(Debug, Clone, Copy)]
struct ConceptStateTransition {
    next_state: ConceptStateSnapshot,
    review_modifier: &'static str,
    success: bool,
}
#[derive(Debug)]
pub struct AppDatabase {
    connection: Connection,
}
impl AppDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let database = Self { connection };
        database.initialize_schema()?;
        Ok(database)
    }
    pub fn stats(&self) -> Result<AppStats> {
        let due_reviews = self.count_query(
            "SELECT COUNT(*) FROM concept_state WHERE next_review_at IS NOT NULL AND next_review_at <= datetime('now')",
        )?;
        let upcoming_deadlines = self.count_query(
            "SELECT COUNT(*) FROM deadlines WHERE due_at IS NOT NULL AND due_at <= datetime('now', '+14 day')",
        )?;
        let total_attempts = self.count_query("SELECT COUNT(*) FROM attempts")?;
        let total_sessions = self.count_query("SELECT COUNT(*) FROM sessions")?;
        Ok(AppStats {
            due_reviews,
            upcoming_deadlines,
            total_attempts,
            total_sessions,
        })
    }
    pub fn load_resume_state(&self) -> Result<Option<ResumeStateRecord>> {
        let record = self
            .connection
            .query_row(
                "
                SELECT session_id, runtime_thread_id, active_mode, active_question_id, focused_panel, draft_payload, scratchpad_text
                FROM resume_state
                ORDER BY saved_at DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok(ResumeStateRecord {
                        session_id: row.get(0)?,
                        runtime_thread_id: row.get(1)?,
                        active_mode: row.get(2)?,
                        active_question_id: row.get(3)?,
                        focused_panel: row.get(4)?,
                        draft_payload: row.get(5)?,
                        scratchpad_text: row.get(6)?,
                    })
                },
            )
            .optional()?;
        Ok(record)
    }
    pub fn save_resume_state(&self, record: &ResumeStateRecord) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO resume_state (
                session_id, runtime_thread_id, saved_at, active_mode, active_question_id, focused_panel, draft_payload, scratchpad_text
            )
            VALUES (?1, ?2, datetime('now'), ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(session_id) DO UPDATE SET
                runtime_thread_id = excluded.runtime_thread_id,
                saved_at = excluded.saved_at,
                active_mode = excluded.active_mode,
                active_question_id = excluded.active_question_id,
                focused_panel = excluded.focused_panel,
                draft_payload = excluded.draft_payload,
                scratchpad_text = excluded.scratchpad_text
            ",
            params![
                record.session_id,
                record.runtime_thread_id,
                record.active_mode,
                record.active_question_id,
                record.focused_panel,
                record.draft_payload,
                record.scratchpad_text,
            ],
        )?;
        Ok(())
    }
    pub fn start_session(&self, record: &SessionRecord) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO sessions (id, started_at, planned_minutes, mode)
            VALUES (?1, datetime('now'), ?2, ?3)
            ON CONFLICT(id) DO NOTHING
            ",
            params![record.id, record.planned_minutes, record.mode],
        )?;
        Ok(())
    }
    pub fn complete_session(
        &self,
        session_id: &str,
        actual_minutes: i64,
        outcome_summary: &str,
        aborted_reason: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            "
            UPDATE sessions
            SET ended_at = datetime('now'),
                actual_minutes = ?2,
                outcome_summary = ?3,
                aborted_reason = ?4
            WHERE id = ?1
            ",
            params![session_id, actual_minutes, outcome_summary, aborted_reason],
        )?;
        Ok(())
    }
    pub fn save_session_recap(&self, record: &SessionRecapRecord) -> Result<()> {
        self.connection.execute(
            "
            UPDATE sessions
            SET outcome_summary = ?2,
                recap_payload = ?3
            WHERE id = ?1
            ",
            params![
                record.session_id,
                record.recap.outcome_summary,
                serde_json::to_string(&record.recap)?,
            ],
        )?;
        Ok(())
    }
    pub fn record_attempt(
        &self,
        attempt: &AttemptRecord,
        misconception: Option<&MisconceptionInput>,
    ) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO attempts (
                id, session_id, concept_id, question_type, prompt_hash, student_answer,
                correctness, latency_ms, reasoning_quality, feedback_summary
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                attempt.id,
                attempt.session_id,
                attempt.concept_id,
                attempt.question_type,
                attempt.prompt_hash,
                attempt.student_answer,
                attempt.correctness,
                attempt.latency_ms,
                attempt.reasoning_quality,
                attempt.feedback_summary,
            ],
        )?;
        self.ensure_concept_state(&attempt.concept_id)?;
        self.update_concept_state(attempt)?;
        if let Some(misconception) = misconception {
            self.upsert_misconception(misconception)?;
        }
        Ok(())
    }
    pub fn list_attempts_for_session(&self, session_id: &str) -> Result<Vec<AttemptAuditRecord>> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, session_id, concept_id, question_type, correctness,
                   reasoning_quality, latency_ms, feedback_summary
            FROM attempts
            WHERE session_id = ?1
            ORDER BY rowid ASC
            ",
        )?;
        let rows = statement.query_map(params![session_id], |row| {
            Ok(AttemptAuditRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                concept_id: row.get(2)?,
                question_type: row.get(3)?,
                correctness: row.get(4)?,
                reasoning_quality: row.get(5)?,
                latency_ms: row.get(6)?,
                feedback_summary: row.get(7)?,
            })
        })?;
        let mut attempts = Vec::new();
        for row in rows {
            attempts.push(row?);
        }
        Ok(attempts)
    }
    pub fn resolve_concept_id(&self, candidates: &[String]) -> Result<Option<String>> {
        for candidate in candidates {
            let resolved = self
                .connection
                .query_row(
                    "
                    SELECT id
                    FROM concepts
                    WHERE lower(id) = lower(?1)
                       OR lower(name) = lower(?1)
                       OR lower(tags) LIKE '%' || lower(?1) || '%'
                    LIMIT 1
                    ",
                    params![candidate],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if resolved.is_some() {
                return Ok(resolved);
            }
        }
        Ok(None)
    }
    pub fn list_due_reviews(&self, limit: usize) -> Result<Vec<DueReviewSummary>> {
        let mut statement = self.connection.prepare(
            "
            SELECT concepts.id, concepts.name, concept_state.next_review_at
            FROM concept_state
            INNER JOIN concepts ON concepts.id = concept_state.concept_id
            WHERE concept_state.next_review_at IS NOT NULL
            ORDER BY
                CASE
                    WHEN concept_state.next_review_at <= datetime('now') THEN 0
                    ELSE 1
                END,
                concept_state.next_review_at ASC
            LIMIT ?1
            ",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(DueReviewSummary {
                concept_id: row.get(0)?,
                concept_name: row.get(1)?,
                next_review_at: row.get(2)?,
            })
        })?;
        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }
    pub fn list_recent_misconceptions(&self, limit: usize) -> Result<Vec<MisconceptionSummary>> {
        let mut statement = self.connection.prepare(
            "
            SELECT concepts.name, misconceptions.error_type, misconceptions.description,
                   misconceptions.last_seen_at, misconceptions.evidence_count
            FROM misconceptions
            INNER JOIN concepts ON concepts.id = misconceptions.concept_id
            WHERE misconceptions.resolved_at IS NULL
            ORDER BY misconceptions.last_seen_at DESC
            LIMIT ?1
            ",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(MisconceptionSummary {
                concept_name: row.get(0)?,
                error_type: row.get(1)?,
                description: row.get(2)?,
                last_seen_at: row.get(3)?,
                evidence_count: row.get::<_, i64>(4)? as usize,
            })
        })?;
        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }
    pub fn latest_session_recap(&self) -> Result<Option<SessionRecapSummary>> {
        let recap = self
            .connection
            .query_row(
                "
                SELECT recap_payload
                FROM sessions
                WHERE recap_payload IS NOT NULL AND recap_payload != ''
                ORDER BY started_at DESC
                LIMIT 1
                ",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        recap
            .map(|raw| serde_json::from_str::<SessionRecapSummary>(&raw).map_err(Into::into))
            .transpose()
    }
    fn initialize_schema(&self) -> Result<()> {
        self.ensure_meta_table()?;
        let current_version = self.detect_schema_version()?;
        if current_version > LATEST_SCHEMA_VERSION {
            return Err(anyhow!(
                "database schema version {current_version} is newer than supported version {LATEST_SCHEMA_VERSION}"
            ));
        }
        for (version, sql) in MIGRATIONS {
            if *version > current_version {
                self.connection.execute_batch(sql)?;
                self.set_schema_version(*version)?;
            }
        }
        self.seed_default_concepts()?;
        Ok(())
    }
    fn ensure_meta_table(&self) -> Result<()> {
        self.connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }
    fn detect_schema_version(&self) -> Result<i64> {
        if let Some(version) = self
            .connection
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![META_SCHEMA_VERSION_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            return version
                .parse::<i64>()
                .map_err(|error| anyhow!("invalid schema version `{version}`: {error}"));
        }
        if !self.table_exists("sessions")? {
            return Ok(0);
        }
        if self.column_exists("sessions", "recap_payload")?
            && self.column_exists("resume_state", "runtime_thread_id")?
        {
            self.set_schema_version(LATEST_SCHEMA_VERSION)?;
            return Ok(LATEST_SCHEMA_VERSION);
        }
        Ok(1)
    }
    fn set_schema_version(&self, version: i64) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO meta (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            ",
            params![META_SCHEMA_VERSION_KEY, version.to_string()],
        )?;
        Ok(())
    }
    fn table_exists(&self, name: &str) -> Result<bool> {
        let exists = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![name],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(exists == 1)
    }
    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        let pragma = format!("PRAGMA table_info({table})");
        let mut statement = self.connection.prepare(&pragma)?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        for entry in columns {
            if entry? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn count_query(&self, sql: &str) -> Result<usize> {
        let count = self
            .connection
            .query_row(sql, [], |row| row.get::<_, i64>(0))?;
        Ok(count as usize)
    }
    fn ensure_concept_state(&self, concept_id: &str) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO concept_state (concept_id)
            VALUES (?1)
            ON CONFLICT(concept_id) DO NOTHING
            ",
            params![concept_id],
        )?;
        Ok(())
    }
    fn concept_state_for(&self, concept_id: &str) -> Result<ConceptStateSnapshot> {
        self.connection
            .query_row(
                "
            SELECT mastery_estimate, retrieval_strength, stability_days, ease_factor
            FROM concept_state
            WHERE concept_id = ?1
            ",
                params![concept_id],
                |row| {
                    Ok(ConceptStateSnapshot {
                        mastery_estimate: row.get(0)?,
                        retrieval_strength: row.get(1)?,
                        stability_days: row.get(2)?,
                        ease_factor: row.get(3)?,
                    })
                },
            )
            .map_err(Into::into)
    }
    fn update_concept_state(&self, attempt: &AttemptRecord) -> Result<()> {
        let current = self.concept_state_for(&attempt.concept_id)?;
        let transition = concept_state_after_attempt(
            current,
            attempt.correctness.as_str(),
            attempt.reasoning_quality.as_str(),
        );
        let success_timestamp = if transition.success {
            Some("datetime('now')")
        } else {
            None
        };
        let failure_timestamp = if transition.success {
            None
        } else {
            Some("datetime('now')")
        };
        self.connection.execute(
            &format!(
                "
                UPDATE concept_state
                SET mastery_estimate = ?2,
                    retrieval_strength = ?3,
                    last_seen_at = datetime('now'),
                    last_success_at = {},
                    last_failure_at = {},
                    next_review_at = datetime('now', ?4),
                    stability_days = ?5,
                    ease_factor = ?6
                WHERE concept_id = ?1
                ",
                success_timestamp.unwrap_or("last_success_at"),
                failure_timestamp.unwrap_or("last_failure_at"),
            ),
            params![
                attempt.concept_id,
                transition.next_state.mastery_estimate,
                transition.next_state.retrieval_strength,
                transition.review_modifier,
                transition.next_state.stability_days,
                transition.next_state.ease_factor,
            ],
        )?;
        Ok(())
    }
    fn upsert_misconception(&self, misconception: &MisconceptionInput) -> Result<()> {
        let existing = self
            .connection
            .query_row(
                "
                SELECT id
                FROM misconceptions
                WHERE concept_id = ?1
                  AND error_type = ?2
                  AND description = ?3
                  AND resolved_at IS NULL
                ORDER BY last_seen_at DESC
                LIMIT 1
                ",
                params![
                    misconception.concept_id,
                    misconception.error_type,
                    misconception.description
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(id) = existing {
            self.connection.execute(
                "
                UPDATE misconceptions
                SET last_seen_at = datetime('now'),
                    evidence_count = evidence_count + 1
                WHERE id = ?1
                ",
                params![id],
            )?;
        } else {
            self.connection.execute(
                "
                INSERT INTO misconceptions (
                    id, concept_id, error_type, description, first_seen_at, last_seen_at, resolved_at, evidence_count
                )
                VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'), NULL, 1)
                ",
                params![
                    make_record_id("misconception", &misconception.description),
                    misconception.concept_id,
                    misconception.error_type,
                    misconception.description,
                ],
            )?;
        }
        Ok(())
    }
    fn seed_default_concepts(&self) -> Result<()> {
        let concepts = [
            (
                "matrix_multiplication_dims",
                "Matrix Algebra & Linear Models",
                "Matrix multiplication dimensions",
                "[\"matrix_multiplication\"]",
            ),
            (
                "determinant_singularity",
                "Matrix Algebra & Linear Models",
                "Determinant zero implies singularity",
                "[\"determinant\", \"invertibility\"]",
            ),
            (
                "variance_definition",
                "Probability & Statistics for Scientists",
                "Variance as expected squared deviation",
                "[\"variance\", \"expectation\"]",
            ),
        ];
        for (id, course, name, tags) in concepts {
            self.connection.execute(
                "
                INSERT INTO concepts (id, course, name, tags)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(id) DO NOTHING
                ",
                params![id, course, name, tags],
            )?;
        }
        Ok(())
    }
}
fn concept_state_after_attempt(
    current: ConceptStateSnapshot,
    correctness: &str,
    reasoning_quality: &str,
) -> ConceptStateTransition {
    let (mastery_delta, retrieval_delta, stability_delta, ease_delta, review_modifier, success) =
        match (correctness, reasoning_quality) {
            ("correct", "strong") => (0.18, 0.22, 2.0, 0.06, "+5 day", true),
            ("correct", "adequate") => (0.12, 0.16, 1.2, 0.03, "+3 day", true),
            ("correct", _) => (0.07, 0.08, 0.6, 0.0, "+1 day", true),
            ("partial", "adequate") => (0.03, -0.02, 0.2, -0.04, "+12 hour", false),
            ("partial", _) => (0.01, -0.05, 0.0, -0.06, "+8 hour", false),
            _ => (-0.08, -0.14, -0.4, -0.1, "+4 hour", false),
        };
    ConceptStateTransition {
        next_state: ConceptStateSnapshot {
            mastery_estimate: clamp(current.mastery_estimate + mastery_delta, 0.0, 1.0),
            retrieval_strength: clamp(current.retrieval_strength + retrieval_delta, 0.0, 1.0),
            stability_days: clamp(current.stability_days + stability_delta, 0.0, 60.0),
            ease_factor: clamp(current.ease_factor + ease_delta, 1.3, 3.0),
        },
        review_modifier,
        success,
    }
}
fn make_record_id(prefix: &str, seed: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    let seed_hash = hasher.finish();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{nanos:x}-{seed_hash:x}")
}
fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}
#[cfg(test)]
mod tests {
    use std::{env, fs};
    use proptest::prelude::*;
    use rusqlite::{Connection, params};
    use crate::SessionRecapSummary;
    use super::{
        AppDatabase, AttemptRecord, ConceptStateSnapshot, LATEST_SCHEMA_VERSION,
        META_SCHEMA_VERSION_KEY, MisconceptionInput, ResumeStateRecord, SessionRecapRecord,
        SessionRecord, concept_state_after_attempt,
    };
    fn temp_db_dir() -> std::path::PathBuf {
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };
        let dir = env::temp_dir().join(format!("studyos-test-{}-{nanos}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("failed to create temp dir: {err}"));
        dir
    }
    fn attempt_case_strategy() -> impl Strategy<Value = (&'static str, &'static str)> {
        prop_oneof![
            Just(("correct", "strong")),
            Just(("correct", "adequate")),
            Just(("correct", "weak")),
            Just(("partial", "adequate")),
            Just(("partial", "missing")),
            Just(("incorrect", "missing")),
        ]
    }
    #[test]
    fn database_bootstrap_seeds_initial_stats() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        {
            let database = AppDatabase::open(&path)
                .unwrap_or_else(|err| panic!("database open failed: {err}"));
            let stats = database
                .stats()
                .unwrap_or_else(|err| panic!("stats query failed: {err}"));
            assert_eq!(stats.due_reviews, 0);
            assert_eq!(stats.upcoming_deadlines, 0);
        }
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn resume_state_round_trips() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        {
            let database = AppDatabase::open(&path)
                .unwrap_or_else(|err| panic!("database open failed: {err}"));
            let record = ResumeStateRecord {
                session_id: "test-session".to_string(),
                runtime_thread_id: Some("runtime-thread".to_string()),
                active_mode: "Study".to_string(),
                active_question_id: Some("4".to_string()),
                focused_panel: "Scratchpad".to_string(),
                draft_payload: "{\"draft\":true}".to_string(),
                scratchpad_text: "rough working".to_string(),
            };
            database
                .save_resume_state(&record)
                .unwrap_or_else(|err| panic!("resume save failed: {err}"));
            let loaded = database
                .load_resume_state()
                .unwrap_or_else(|err| panic!("resume load failed: {err}"))
                .unwrap_or_else(|| panic!("missing resume state"));
            assert_eq!(loaded.session_id, record.session_id);
            assert_eq!(loaded.runtime_thread_id, record.runtime_thread_id);
            assert_eq!(loaded.focused_panel, record.focused_panel);
            assert_eq!(loaded.scratchpad_text, record.scratchpad_text);
        }
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn attempt_logging_updates_reviews_and_misconceptions() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        {
            let database = AppDatabase::open(&path)
                .unwrap_or_else(|err| panic!("database open failed: {err}"));
            database
                .start_session(&SessionRecord {
                    id: "session-1".to_string(),
                    planned_minutes: 45,
                    mode: "Study".to_string(),
                })
                .unwrap_or_else(|err| panic!("session start failed: {err}"));
            database
                .record_attempt(
                    &AttemptRecord {
                        id: "attempt-1".to_string(),
                        session_id: "session-1".to_string(),
                        concept_id: "matrix_multiplication_dims".to_string(),
                        question_type: "retrieval_response".to_string(),
                        prompt_hash: "abc123".to_string(),
                        student_answer: "rows and columns mismatched".to_string(),
                        correctness: "incorrect".to_string(),
                        latency_ms: 1200,
                        reasoning_quality: "missing".to_string(),
                        feedback_summary: "You mixed up inner and outer dimensions.".to_string(),
                    },
                    Some(&MisconceptionInput {
                        concept_id: "matrix_multiplication_dims".to_string(),
                        error_type: "conceptual_misunderstanding".to_string(),
                        description: "Confused inner and outer dimensions.".to_string(),
                    }),
                )
                .unwrap_or_else(|err| panic!("attempt record failed: {err}"));
            let reviews = database
                .list_due_reviews(5)
                .unwrap_or_else(|err| panic!("due review query failed: {err}"));
            let misconceptions = database
                .list_recent_misconceptions(5)
                .unwrap_or_else(|err| panic!("misconception query failed: {err}"));
            let stats = database
                .stats()
                .unwrap_or_else(|err| panic!("stats query failed: {err}"));
            assert_eq!(stats.total_attempts, 1);
            assert_eq!(stats.total_sessions, 1);
            assert!(!reviews.is_empty());
            assert_eq!(reviews[0].concept_id, "matrix_multiplication_dims");
            assert_eq!(misconceptions.len(), 1);
            assert_eq!(
                misconceptions[0].error_type,
                "conceptual_misunderstanding".to_string()
            );
        }
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn session_recap_round_trips() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        {
            let database = AppDatabase::open(&path)
                .unwrap_or_else(|err| panic!("database open failed: {err}"));
            database
                .start_session(&SessionRecord {
                    id: "session-recap".to_string(),
                    planned_minutes: 30,
                    mode: "Study".to_string(),
                })
                .unwrap_or_else(|err| panic!("session start failed: {err}"));
            let recap = SessionRecapSummary {
                outcome_summary: "Recovered the matrix product rule.".to_string(),
                demonstrated_concepts: vec!["Matrix multiplication dimensions".to_string()],
                weak_concepts: vec!["Explaining why rows dot beta".to_string()],
                next_review_items: vec!["Revisit matrix-vector products tomorrow".to_string()],
                unfinished_objectives: vec![
                    "Explain why each entry of X beta is a row dot product.".to_string(),
                ],
            };
            database
                .save_session_recap(&SessionRecapRecord {
                    session_id: "session-recap".to_string(),
                    recap: recap.clone(),
                })
                .unwrap_or_else(|err| panic!("save recap failed: {err}"));
            let loaded = database
                .latest_session_recap()
                .unwrap_or_else(|err| panic!("load recap failed: {err}"))
                .unwrap_or_else(|| panic!("missing recap"));
            assert_eq!(loaded, recap);
        }
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn repeated_identical_misconception_does_not_duplicate() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        let database =
            AppDatabase::open(&path).unwrap_or_else(|err| panic!("database open failed: {err}"));
        database
            .start_session(&SessionRecord {
                id: "session-1".to_string(),
                planned_minutes: 30,
                mode: "Study".to_string(),
            })
            .unwrap_or_else(|err| panic!("session start failed: {err}"));
        for index in 0..2 {
            database
                .record_attempt(
                    &AttemptRecord {
                        id: format!("attempt-{index}"),
                        session_id: "session-1".to_string(),
                        concept_id: "matrix_multiplication_dims".to_string(),
                        question_type: "retrieval_response".to_string(),
                        prompt_hash: format!("hash-{index}"),
                        student_answer: "wrong".to_string(),
                        correctness: "incorrect".to_string(),
                        latency_ms: 500,
                        reasoning_quality: "missing".to_string(),
                        feedback_summary: "Still confused.".to_string(),
                    },
                    Some(&MisconceptionInput {
                        concept_id: "matrix_multiplication_dims".to_string(),
                        error_type: "conceptual_misunderstanding".to_string(),
                        description: "Confused inner and outer dimensions.".to_string(),
                    }),
                )
                .unwrap_or_else(|err| panic!("attempt record failed: {err}"));
        }
        let misconceptions = database
            .list_recent_misconceptions(5)
            .unwrap_or_else(|err| panic!("misconception query failed: {err}"));
        assert_eq!(misconceptions.len(), 1);
        assert_eq!(misconceptions[0].evidence_count, 2);
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn schema_version_refuses_newer_db() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        let connection =
            Connection::open(&path).unwrap_or_else(|err| panic!("sqlite open failed: {err}"));
        connection
            .execute_batch(
                "
                CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                INSERT INTO meta (key, value) VALUES ('schema_version', '999');
                ",
            )
            .unwrap_or_else(|err| panic!("meta seed failed: {err}"));
        let error = AppDatabase::open(&path).expect_err("newer schema should be rejected");
        assert!(error.to_string().contains("newer than supported"));
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn legacy_schema_upgrades_forward() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        let connection =
            Connection::open(&path).unwrap_or_else(|err| panic!("sqlite open failed: {err}"));
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .unwrap_or_else(|err| panic!("initial migration seed failed: {err}"));
        let database =
            AppDatabase::open(&path).unwrap_or_else(|err| panic!("database open failed: {err}"));
        let loaded = database
            .load_resume_state()
            .unwrap_or_else(|err| panic!("resume load failed: {err}"));
        assert!(loaded.is_none());
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT value FROM meta WHERE key = ?1",
                    params![META_SCHEMA_VERSION_KEY],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_else(|err| panic!("schema version read failed: {err}")),
            LATEST_SCHEMA_VERSION.to_string()
        );
        let _ = fs::remove_dir_all(dir);
    }
    #[test]
    fn correct_attempts_raise_mastery_and_incorrect_attempts_lower_it() {
        let mut state = ConceptStateSnapshot {
            mastery_estimate: 0.5,
            retrieval_strength: 0.5,
            stability_days: 3.0,
            ease_factor: 2.5,
        };
        for _ in 0..8 {
            state = concept_state_after_attempt(state, "correct", "strong").next_state;
        }
        assert!(state.mastery_estimate > 0.9);
        for _ in 0..8 {
            state = concept_state_after_attempt(state, "incorrect", "missing").next_state;
        }
        assert!(state.mastery_estimate < 0.5);
    }
    proptest! {
        #[test]
        fn mastery_retrieval_and_ease_stay_in_range(sequence in prop::collection::vec(attempt_case_strategy(), 1..64)) {
            let mut state = ConceptStateSnapshot {
                mastery_estimate: 0.0,
                retrieval_strength: 0.0,
                stability_days: 0.0,
                ease_factor: 2.5,
            };
            for (correctness, reasoning_quality) in sequence {
                state = concept_state_after_attempt(state, correctness, reasoning_quality).next_state;
                prop_assert!((0.0..=1.0).contains(&state.mastery_estimate));
                prop_assert!((0.0..=1.0).contains(&state.retrieval_strength));
                prop_assert!((0.0..=60.0).contains(&state.stability_days));
                prop_assert!((1.3..=3.0).contains(&state.ease_factor));
            }
        }
    }
}
```
