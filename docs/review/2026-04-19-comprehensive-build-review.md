# StudyOS Build Review

Date: 2026-04-19
Repository: `StudyOS`
Review scope: commits `fc25bdb` through `83c0513`
Audience: external technical/product reviewer with no direct repo access

## Bookmark: next implementation step

The next implementation step is now explicitly bookmarked as:

**make session planning use real upcoming study windows and slot pressure, not just counts and summaries of deadlines, timetable entries, materials, and prior errors.**

More concretely, the next phase should convert local time context into planning decisions such as:
- detecting realistic study windows from the local timetable
- distinguishing short opportunistic sessions from deep-work windows
- changing opener shape based on whether the student has 15 minutes, 45 minutes, or 2 hours
- deciding when to repair, drill, recap, or stretch based on the actual window rather than just deadline counts
- carrying unfinished work into the next viable slot rather than only the next app launch

That next step is bookmarked. The rest of this document reviews what has already been built.

## Executive summary

StudyOS has moved from an empty public Rust repo to a functioning terminal-native tutoring runtime with a real Codex app-server loop, structured answer widgets, local study memory, session recap persistence, course-aware startup, and local deadline/materials/timetable tooling.

The work completed so far does **materially support** the target vision of a one-terminal, low-distraction, self-generating teaching OS. It does this best in three ways:
- it keeps the student inside the terminal and inside one interaction surface
- it prioritises active question answering over passive content reading
- it gives the agent local memory, local temporal context, and local academic context to condition session generation

At the same time, the current system does **not yet fully satisfy** the strongest version of the product statement. The biggest gaps are:
- no true study-window-aware planner yet
- no full materials ingestion pipeline for a student’s complete course corpus
- no dedicated “upload all course materials here and let the tutor ingest them” workflow
- no dedicated skill orchestration layer inside app-server yet; most of the harness is prompt/schema-driven rather than skill-driven
- no slide mode or richer teaching mode beyond transcript-plus-widget interaction

My overall view is that the project is already meaningfully beyond a proof of concept, but it is still in the phase where the **harness and memory runtime are strong**, while the **full pedagogical world model** is only partially built.

## Commit-by-commit review

### `fc25bdb` Bootstrap StudyOS workspace

This commit created the public repo foundation:
- Cargo workspace
- CLI/core crate split
- CI, ignore rules, editor config
- initial README and public-safe defaults

Why it matters:
- it established a clean public foundation without leaking private local study state
- it made the repo safe to iterate on quickly and visibly
- it created the basic shape needed for a real product rather than an ad hoc prototype

### `595c0d7` Add V1 implementation planning docs

This commit created a full implementation doc set under `docs/implementation/`.

Why it matters:
- it translated the original broad product spec into an implementation-oriented V1 plan
- it made the project legible to future collaborators
- it kept V1 focused on a usable first release rather than a browser LMS clone

### `eab896b` Build TUI bootstrap and local study context

This was the first major implementation slice. It added:
- a full-screen Rust TUI shell
- transcript, header, panel, answer area, help overlay
- local config paths and example local data
- SQLite bootstrap and local storage setup
- structured widgets for matrix entry, working/final answer, step list, and retrieval response
- course/deadline/materials/timetable loading

Why it matters:
- this commit made the project real as a terminal program
- it established that the core interaction model would be **interactive structured answering**, not plain chat text
- it anchored the “one terminal” premise in actual software rather than docs alone

### `cb2700f` Integrate Codex app-server tutor runtime

This commit connected the client to the actual Codex app-server over `stdio` and added:
- runtime process management
- initialize / thread start / thread resume / turn start flows
- structured JSON tutor payloads
- rendering of tutor-generated teaching blocks and question cards
- resume-thread persistence

Why it matters:
- this is where StudyOS became a real agent runtime rather than just a local shell
- it established the central architecture decision: Codex as conversation runtime, client as renderer and educational interaction surface
- it made the tutor self-generating rather than a static worksheet player

### `e6c739a` Persist session evidence and misconception state

This commit turned the runtime into a study-memory system by adding:
- attempt persistence
- misconception logging/upsert
- review timing updates
- session completion records
- evaluation metadata flow from tutor payloads into SQLite

Why it matters:
- it created the beginnings of a real student model
- it shifted the system from “generate a question” to “build evidence across sessions”
- it enabled later temporal and adaptive planning

### `ba75bf6` Test tutor evaluation persistence path

This commit added direct regression coverage around the graded structured tutor path.

Why it matters:
- it validated that the structured tutor contract was not just aspirational
- it reduced the risk that the evidence layer silently drifts from the runtime

### `2a70ef4` Route opening sessions from local study history

This commit made session opening history-aware by routing startup into `Study`, `Review`, or `Drill` using:
- due reviews
- misconceptions
- deadline pressure
- local rationale text

Why it matters:
- it moved the system closer to adaptive session orchestration
- it gave startup a pedagogical reason, not just a default landing screen
- it made “why now?” part of the generated experience

### `32b8e26` Add session recap persistence and exit review

This commit added:
- structured close-session recap generation
- recap persistence in SQLite
- unfinished objective carry-forward
- a quit-flow recap overlay before exit confirmation

Why it matters:
- it makes the system feel like a continuous study runtime rather than disconnected chats
- it adds friction against casual abandonment without trapping the user
- it helps the next launch begin from a real instructional continuation point

### `325ab17` Improve structured math widget ergonomics

This commit improved core answer surfaces by:
- allowing matrix questions to specify explicit dimensions rather than forcing `2x2`
- making matrix widgets show intended shape
- replacing the old “Shift to type final answer” hack with explicit working/final-answer focus
- hardening widget tests

Why it matters:
- it directly supports the product’s strongest differentiator: interactive structured mathematical answering
- it reduces friction in the answer path, which is essential for “fast as paper” interaction
- it moves the client away from “fancy rendering over chat” and toward “terminal-native worksheet runtime”

### `4b773a0` Add local deadline and course management commands

This commit added:
- `deadlines list/add`
- `courses list/use`
- config persistence helpers
- more realistic local deadline handling

Why it matters:
- it made local context operable rather than static example JSON
- it improved the “no-friction” claim, because students can now change course/deadline state from inside the toolchain
- it began to turn the local harness into a living personal study environment

### `fca585c` Make bootstrap sessions course-aware

This commit made offline/bootstrap content depend on the active course. Probability sessions now boot with stats-flavoured placeholder prompts rather than matrix algebra by default.

Why it matters:
- it fixed a major trust problem: a probability student should not boot into the wrong subject while the runtime warms up
- it made the shell feel like one coherent tutor rather than a hard-coded linear algebra demo

### `2a023fa` Add local materials search and prompt context

This commit added:
- local materials metadata search
- `materials list/search`
- prompt injection of relevant local material snippets into the opening tutor context

Why it matters:
- it created the first real bridge between local course resources and tutor generation
- it supports the self-generating teaching OS goal by giving the agent local topical context to condition its questions
- it does this without relying on verbatim replay of source materials

### `83c0513` Add local timetable commands and prompt context

This commit added:
- `timetable show/today/add`
- timetable sorting and persistence helpers
- timetable summary injection into the opening tutor prompt
- better deadline/timetable visibility in the local panel view

Why it matters:
- it improves temporal awareness
- it gives the agent more grounded context about the student’s academic rhythm
- it is the prerequisite for the bookmarked next step: true study-window-aware planning

## What has been built in product terms

At this point StudyOS has the following concrete product capabilities.

### 1. Terminal-native runtime

The product now runs as a real full-screen Rust TUI with:
- header, transcript, answer pane, side panel, help overlay
- keyboard-first navigation
- safe exit and recap review
- local-only data path under `.studyos/`

This strongly supports the “one-terminal, no-distraction” part of the brief.

### 2. Real agent runtime

The product is not faking AI generation. It now:
- spawns Codex app-server locally over `stdio`
- initializes it correctly
- starts/resumes threads
- starts structured turns
- streams tutor output
- persists thread identity for resume

This supports the “self-generating teaching OS” part of the brief.

### 3. Interactive teaching surface

The strongest implemented differentiator is the structured answer model:
- matrix grid
- working + final answer
- step list
- short retrieval response

This supports the most important product principle: **teaching through interaction rather than through slide-reading**.

### 4. Local memory and evidence

The system now records:
- sessions
- attempts
- misconceptions
- due reviews
- recap payloads
- unfinished objectives

This gives the runtime continuity and enables multi-session adaptation.

### 5. Local academic context

The system now has local tooling for:
- deadlines
- active course selection
- materials metadata search
- timetable inspection and entry

This gives the agent more grounded context than a generic tutor chat would have.

## How well it meets the core goals

### One terminal

This goal is already strongly met.

Why:
- all main workflows run inside the TUI or CLI commands
- course/deadline/materials/timetable management now have local commands
- no browser is required for normal operation

Remaining weakness:
- there is not yet a polished “upload all course materials here” ingestion flow

### No distraction

This goal is partially met and trending well.

Why:
- the product is terminal-first and full-screen
- the interaction model keeps the student in one interface
- the quit recap introduces deliberate but safe friction
- the system emphasizes active answering over browsing resources

Remaining weakness:
- focus mode is still lightweight; there is no OS-level DND integration, shell escape control, or stronger interruption management

### No friction

This goal is partially met.

Why:
- `cargo run -p studyos-cli` is a single-command start
- local data init and doctor flows are straightforward
- local deadlines/courses/materials/timetable are now manageable without hand-editing multiple files

Remaining weakness:
- the product is still developer-shaped rather than student-polished
- there is no packaging/distribution story yet
- there is not yet a single canonical “drop your materials here and start studying” workflow

### Self-generating teaching OS

This goal is meaningfully met, but only to an early-stage degree.

Why:
- the tutor generates plans, teaching blocks, questions, feedback, and recaps live
- sessions are shaped by memory, misconceptions, deadlines, course selection, and local materials metadata
- the system already behaves more like an adaptive runtime than a static content viewer

Remaining weakness:
- the local context is still relatively shallow compared with the full ambition of the product
- there is no dedicated course-corpus ingestion or concept extraction pipeline yet

### Interactive teaching rather than slide reading

This is the area where the implementation is currently most aligned with the vision.

Why:
- the system is question-first
- it blocks blank attempts locally
- it uses structured answer widgets instead of relying on chat prose alone
- it forces the runtime into one-question-at-a-time interaction cycles
- it prefers retrieval, repair, and transfer over long passive exposition

Remaining weakness:
- the tutor still speaks through a transcript, so if future prompts drift, the model could still become too explanatory
- slide mode is not present, but that is actually acceptable at this stage because the current product is much more interactive than slide-centric

My judgment here is clear: **the project is already much closer to “interactive teaching OS” than to “terminal slideshow viewer.”**

## Temporal awareness: current state

The system is already temporally aware in several real ways.

Current temporal signals implemented:
- current local time at runtime
- session duration target from config
- due review queue
- review scheduling in SQLite
- high-confidence repair carry-forward via recap and unfinished objectives
- upcoming deadline counts
- local deadlines listing and persistence
- local timetable slots and upcoming-slot summaries

What this means in practice:
- the runtime knows whether there is memory pressure
- the runtime knows whether there are deadlines approaching
- the runtime knows what was left unfinished last time
- the runtime now sees timetable context as part of the opening prompt

What is still missing:
- actual study-window estimation
- ranking candidate study windows by length and urgency
- choosing different opener shapes for “10 minutes before class” versus “90 minutes tonight”
- exam-horizon logic richer than deadline counts and prompt hints
- planning across future sessions rather than only shaping the current opener

Conclusion:
- temporal awareness is **real but still shallow-to-moderate**
- it is not yet the fully deadline-aware planning engine described in the original spec

## Adaptation and agency: how much has the agent been allowed to think?

This is the most important architectural question in the build so far.

### What the harness currently gives the agent

The agent currently receives:
- course focus
- session duration
- mode recommendation and local rationale
- due reviews
- misconceptions
- deadlines summary
- local materials summary
- timetable summary
- prior unfinished objectives
- structured answer submissions
- evidence and misconception feedback loops

This is good. It means the model is not operating blind.

### What the harness currently constrains

The harness is also quite strict. It forces:
- schema-constrained JSON tutor output
- exactly one active question per turn
- limited teaching blocks before the question
- explicit widget kinds
- explicit evaluation fields
- explicit recap fields
- blank-attempt rejection locally
- attempt-first session flow
- recap-confirm quit flow

This is also good up to a point, because it preserves UI integrity and pedagogy.

### My judgment on guardrail strictness

The harness has become **moderately complex and fairly strict**.

It is not yet an over-scripted deterministic teaching engine, but it is more constrained than a simple “agent with memory and tools” architecture.

I would characterise the current balance like this:
- the model has **high freedom over content inside a turn**
- the model has **medium freedom over session pedagogy**
- the model has **low freedom over output structure and interaction form**

That means the current system is closer to:
- “here is a fairly strict educational UI contract; generate within it”
than to:
- “here are broad tools/memory/signals; plan freely however you want”

### Does this reduce autonomy?

Yes, somewhat.

In particular, the current harness may reduce autonomy in these ways:
- every turn is forced into a relatively narrow schema
- the agent cannot choose to expand into more complex multi-step lesson structures without fitting them into the same payload shape
- the supported widget list narrows how it can pose questions
- the opening turn requirements are very explicit and may produce repetitive patterns over time

On the other hand, this strictness currently protects the product from collapsing into generic tutoring chat.

My view is:
- **the current strictness is justified for V1** because it enforces interactivity and keeps the UI coherent
- but the project should avoid hard-coding too many pedagogical decisions into the harness as it grows

A good future direction would be:
- keep the UI contract strict
- keep the memory contract strict
- relax the pedagogical contract slightly so the model has more room to choose session arcs, not just single-turn shapes

## Have I forced the model into a narrow workflow?

Yes, to a meaningful degree.

The current implementation does force a relatively narrow workflow:
- open with a concise plan
- show a small number of teaching blocks
- ask exactly one structured question
- accept one structured answer
- evaluate it
- move to one next question

This is intentional and useful for early-stage product discipline.

However, it does mean the current system is not yet leveraging the full intuition of the model in a broad “agent teacher” sense. It is instead leveraging the model as a **high-quality adaptive generator inside a carefully shaped tutoring loop**.

I do not think that is a mistake at this stage, but I do think it is important to name it clearly.

What the system currently has is not “simple tools + memory + let the agent figure it out.”
What it currently has is “a fairly opinionated interactive harness with memory and context, inside which the agent operates.”

That is a sound V1 decision, but it should remain a conscious decision rather than an accidental ratchet toward rigidity.

## How much genuine adaptation to the student exists today?

There is already meaningful adaptation, but it is adaptation to **evidence and context**, not yet to a rich student model.

Current adaptation signals include:
- correctness
- reasoning quality
- misconception type
- latency
- unfinished objectives
- due reviews
- deadline pressure
- course choice
- local materials relevance
- timetable visibility

That means the system adapts to:
- what the student got wrong
- how they reasoned
- what is due for review
- what subject they are studying
- what is approaching in their local study context

What is not yet implemented:
- durable learner preferences beyond a few config settings
- stable modelling of “this student benefits from X questioning style”
- richer behavioural passivity heuristics beyond the basic attempt-first enforcement
- dynamic selection among multiple pedagogical “styles” or teaching personas

My view:
- the current adaptation is **performance-aware and context-aware**
- it is **not yet truly personalised in a deep sense**

That is acceptable for the current stage, but the distinction matters.

## Materials ingestion: the biggest strategic gap

This is the single most important critique in this report.

### What exists now

The product currently has:
- a materials manifest format
- local materials metadata loading
- local materials search CLI
- prompt injection of material snippets as contextual signals

### What does not exist yet

The product does **not yet** have:
- a dedicated folder where the student can drop all course materials
- a true ingest pipeline over that folder
- extraction/indexing over raw PDFs, slides, notes, problem sheets, mark schemes, and past papers
- concept/topic mapping from the raw material corpus
- tutor planning that depends on what has and has not been covered in the uploaded course corpus
- automated synthesis of unique generated questions grounded in the uploaded corpus

### Why this matters

This is not a minor missing feature. It is central to the product’s actual novelty.

The key innovation is not just “the tutor has files.”
The key innovation is:
- the student uploads the full course corpus
- the agent ingests it into a local understanding of scope, sequencing, terminology, and style
- the agent uses that understanding as context
- the agent generates new teaching material and questions rather than replaying the originals

That is exactly how the product avoids two major failure modes:
- running out of useful material
- asking questions on content the student has not yet covered

### Has this been considered?

Yes, conceptually. The materials manifest and prompt-context work were built with that future direction in mind.

### Has it been implemented sufficiently yet?

No.

At the moment the implementation is only a **metadata foothold**, not a true ingestion system.

### My recommendation

The product needs a dedicated local materials pipeline, likely something like:
- `.studyos/materials/raw/`
- `.studyos/materials/index/`
- `.studyos/materials/ingestion-manifest.json`

The pipeline should eventually:
- scan all uploaded files
- extract text where possible
- record source metadata and topical tags
- build concept/topic associations
- keep the source corpus private and local
- surface only distilled context to the agent
- never depend on verbatim replay as the main teaching mode

This should be treated as a first-order product requirement, not a nice-to-have.

## Does the current build satisfy the spirit of the original requirements?

### Yes, in these ways

- It is terminal-native.
- It is already more interactive than explanatory.
- It uses structured answer widgets rather than plain chat.
- It has real continuity across sessions.
- It has a functioning app-server integration rather than mocked intelligence.
- It already uses local temporal and academic context.
- It has started to become a personal study runtime rather than a generic AI front end.

### No, or not yet, in these ways

- It is not yet fully temporally planned from actual study windows.
- It does not yet ingest a student’s full course corpus.
- It does not yet have a rich enough pedagogical planner to claim deep personal adaptation.
- It still leans heavily on a strict single-turn schema harness.
- It has not yet proven that the tutor can track course coverage deeply enough to always stay within the student’s real frontier of understanding.

## My honest assessment for an external reviewer

If an external reviewer asked me whether this work is on the right path, I would say yes.

If they asked whether the current implementation already fully realises the spec, I would say no.

If they asked whether the current implementation is just “vanilla Codex with fancy rendering,” I would say no as well.

Why I think it is already materially different from vanilla Codex:
- structured answer widgets are first-class, not incidental
- memory and misconception persistence are first-class
- recap and unfinished-objective continuity are first-class
- local deadlines/materials/timetable/course context are now part of the runtime
- the UI contract forces interactive teaching rather than passive essay responses

Why I think it still falls short of the full ambition:
- the model still sees a narrower pedagogical space than the full “agent teacher OS” vision
- the materials pipeline is not yet strong enough to ground planning in the real course corpus
- the temporal planner is still contextual rather than truly scheduling-aware

## Specific questions I would ask the external reviewer to focus on

- Is the current balance between strict UI/schema guardrails and agent pedagogical autonomy correct for V1?
- Should the tutor payload contract remain this strict, or should later turns allow looser lesson-arc planning?
- Is the materials manifest approach a sound stepping stone, or is it too shallow and likely to require redesign?
- What is the best local ingestion architecture for a full student course corpus while preserving privacy and speed?
- Should study-window planning be built primarily in Rust client logic, in SQLite-derived heuristics, or delegated more heavily to the agent runtime?
- Is the app-server harness becoming too complex in the client rather than staying thin?
- Are the current structured widgets sufficient, or does the system need a proper scratch-work editor sooner than expected?

## Recommendation

My recommendation is:
- keep the current interaction-first architecture
- keep the structured answer surface as a core invariant
- keep the memory/evidence model growing
- relax pedagogy control only carefully
- prioritise the bookmarked next step of real study-window-aware planning
- but elevate full materials ingestion to a near-term architectural priority, because it is core to the actual product moat

## Codepack appendix

The appendix below was generated with Repomix using documented CLI flags for `--include`, `--output`, `--style`, `--compress`, `--token-count-tree`, and `--no-security-check`.

Selection rationale:
- include the files that define the runtime harness, tutor contract, local memory, local context, and session planning
- exclude secondary CLI plumbing and README/docs to stay under the hard 15k token cap
- preserve enough real code for an external reviewer to inspect the actual control loop and data model

Repomix command used:

```bash
npx repomix \
  --style markdown \
  --compress \
  --no-security-check \
  --token-count-tree 1 \
  --include "crates/studyos-cli/src/app.rs,crates/studyos-cli/src/runtime.rs,crates/studyos-core/src/session.rs,crates/studyos-core/src/store.rs,crates/studyos-core/src/local_data.rs,crates/studyos-core/src/tutor.rs" \
  --output /tmp/repomix-pack.md
```

Repomix reported total token count for the appendix below: `14,402`.

---
This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.
The content has been processed where content has been compressed (code blocks are separated by ⋮---- delimiter), security check has been disabled.

# File Summary

## Purpose
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.

## File Format
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  a. A header with the file path (## File: path/to/file)
  b. The full contents of the file in a code block

## Usage Guidelines
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.

## Notes
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: crates/studyos-cli/src/app.rs, crates/studyos-cli/src/runtime.rs, crates/studyos-core/src/session.rs, crates/studyos-core/src/store.rs, crates/studyos-core/src/local_data.rs, crates/studyos-core/src/tutor.rs
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Content has been compressed - code blocks are separated by ⋮---- delimiter
- Security check has been disabled - content may contain sensitive information
- Files are sorted by Git change count (files with more changes are at the bottom)

# Directory Structure
```
crates/
  studyos-cli/
    src/
      app.rs
      runtime.rs
  studyos-core/
    src/
      local_data.rs
      session.rs
      store.rs
      tutor.rs
```

# Files

## File: crates/studyos-cli/src/runtime.rs
```rust
pub enum RuntimeEvent {
⋮----
type PendingMap = Arc<Mutex<HashMap<u64, Sender<Result<Value, String>>>>>;
⋮----
pub struct AppServerClient {
⋮----
impl AppServerClient {
pub fn spawn() -> Result<Self> {
⋮----
.arg("app-server")
.arg("--listen")
.arg("stdio://")
.stdin(Stdio::piped())
.stdout(Stdio::piped())
.stderr(Stdio::piped())
.spawn()?;
⋮----
.take()
.ok_or_else(|| anyhow!("failed to capture app-server stdin"))?;
⋮----
.ok_or_else(|| anyhow!("failed to capture app-server stdout"))?;
⋮----
.ok_or_else(|| anyhow!("failed to capture app-server stderr"))?;
⋮----
spawn_stdout_reader(stdout, Arc::clone(&pending), event_tx.clone());
spawn_stderr_reader(stderr, event_tx);
⋮----
Ok(Self {
⋮----
pub fn initialize(&self) -> Result<()> {
let params = json!({
⋮----
let _ = self.send_request("initialize", params)?;
self.send_notification("initialized", Value::Null)?;
Ok(())
⋮----
pub fn start_thread(&self, cwd: &Path, developer_instructions: &str) -> Result<String> {
⋮----
let result = self.send_request("thread/start", params)?;
⋮----
.get("thread")
.and_then(|thread| thread.get("id"))
.and_then(Value::as_str)
.map(ToOwned::to_owned)
.ok_or_else(|| anyhow!("thread/start response missing thread id"))
⋮----
pub fn resume_thread(&self, thread_id: &str, cwd: &Path) -> Result<String> {
⋮----
let result = self.send_request("thread/resume", params)?;
⋮----
.ok_or_else(|| anyhow!("thread/resume response missing thread id"))
⋮----
pub fn start_structured_turn(
⋮----
let result = self.send_request("turn/start", params)?;
⋮----
.get("turn")
.and_then(|turn| turn.get("id"))
⋮----
.ok_or_else(|| anyhow!("turn/start response missing turn id"))
⋮----
pub fn poll_events(&self) -> Vec<RuntimeEvent> {
⋮----
while let Ok(event) = self.events.try_recv() {
events.push(event);
⋮----
pub fn run_structured_turn_and_wait(
⋮----
let turn_id = self.start_structured_turn(thread_id, prompt, output_schema, cwd)?;
⋮----
while started_at.elapsed() < timeout {
let remaining = timeout.saturating_sub(started_at.elapsed());
⋮----
.recv_timeout(remaining.min(Duration::from_secs(1)))
.map_err(|_| anyhow!("timed out waiting for structured turn completion"))?;
⋮----
text_buffers.entry(item_id).or_default().push_str(&delta);
⋮----
&& item.get("type").and_then(Value::as_str) == Some("agentMessage") =>
⋮----
.get("id")
⋮----
.unwrap_or("")
.to_string();
⋮----
.get("text")
⋮----
completed_text = Some(text_buffers.remove(&item_id).unwrap_or(fallback));
⋮----
return Err(anyhow!("structured turn {turn_id} failed"));
⋮----
return Ok(text);
⋮----
return Err(anyhow!(message));
⋮----
Err(anyhow!("timed out waiting for structured turn payload"))
⋮----
fn send_request(&self, method: &str, params: Value) -> Result<Value> {
let id = self.next_id.fetch_add(1, Ordering::SeqCst);
⋮----
.lock()
.map_err(|_| anyhow!("pending request lock poisoned"))?
.insert(id, tx);
⋮----
let message = json!({
⋮----
.map_err(|_| anyhow!("app-server stdin lock poisoned"))?;
writeln!(stdin, "{}", message)?;
stdin.flush()?;
⋮----
match rx.recv_timeout(Duration::from_secs(60)) {
Ok(Ok(value)) => Ok(value),
Ok(Err(error)) => Err(anyhow!(error)),
Err(_) => Err(anyhow!(
⋮----
fn send_notification(&self, method: &str, params: Value) -> Result<()> {
let message = if params.is_null() {
json!({
⋮----
impl Drop for AppServerClient {
fn drop(&mut self) {
if let Ok(mut child) = self.child.lock() {
let _ = child.kill();
let _ = child.wait();
⋮----
fn spawn_stdout_reader(
⋮----
for line in reader.lines() {
⋮----
if let Some(id) = message.get("id").and_then(Value::as_u64) {
let sender = pending.lock().ok().and_then(|mut map| map.remove(&id));
⋮----
if let Some(result) = message.get("result") {
let _ = sender.send(Ok(result.clone()));
} else if let Some(error) = message.get("error") {
let _ = sender.send(Err(error.to_string()));
⋮----
if let Some(method) = message.get("method").and_then(Value::as_str) {
let params = message.get("params").cloned().unwrap_or(Value::Null);
if let Some(event) = map_notification(method, params) {
let _ = event_tx.send(event);
⋮----
let _ = event_tx.send(RuntimeEvent::Error {
message: format!("failed to parse app-server message: {error}"),
⋮----
message: format!("failed to read app-server stdout: {error}"),
⋮----
let _ = event_tx.send(RuntimeEvent::Disconnected);
⋮----
fn spawn_stderr_reader(stderr: ChildStderr, event_tx: Sender<RuntimeEvent>) {
⋮----
Ok(line) if !line.trim().is_empty() => {
⋮----
message: format!("app-server stderr: {line}"),
⋮----
message: format!("failed to read app-server stderr: {error}"),
⋮----
fn map_notification(method: &str, params: Value) -> Option<RuntimeEvent> {
⋮----
.map(|thread_id| RuntimeEvent::ThreadReady {
thread_id: thread_id.to_string(),
⋮----
.get("status")
.map(stringify_status)
.map(|status| RuntimeEvent::ThreadStatusChanged { status }),
⋮----
.map(|turn_id| RuntimeEvent::TurnStarted {
turn_id: turn_id.to_string(),
⋮----
let turn = params.get("turn")?;
let turn_id = turn.get("id")?.as_str()?.to_string();
⋮----
.unwrap_or("unknown")
⋮----
Some(RuntimeEvent::TurnCompleted { turn_id, status })
⋮----
"item/started" => Some(RuntimeEvent::ItemStarted {
turn_id: params.get("turnId")?.as_str()?.to_string(),
item: params.get("item")?.clone(),
⋮----
"item/completed" => Some(RuntimeEvent::ItemCompleted {
⋮----
"item/agentMessage/delta" => Some(RuntimeEvent::AgentMessageDelta {
⋮----
item_id: params.get("itemId")?.as_str()?.to_string(),
delta: params.get("delta")?.as_str()?.to_string(),
⋮----
"mcpServer/startupStatus/updated" => Some(RuntimeEvent::McpServerStatusUpdated {
name: params.get("name")?.as_str()?.to_string(),
status: params.get("status")?.as_str()?.to_string(),
⋮----
"error" => Some(RuntimeEvent::Error {
message: params.to_string(),
⋮----
fn stringify_status(value: &Value) -> String {
if let Some(kind) = value.get("type").and_then(Value::as_str) {
return kind.to_string();
⋮----
value.to_string()
```

## File: crates/studyos-core/src/local_data.rs
```rust
use anyhow::Result;
⋮----
pub struct DeadlineEntry {
⋮----
pub struct TimetableSlot {
⋮----
pub struct TimetableData {
⋮----
pub struct MaterialEntry {
⋮----
pub struct LocalContext {
⋮----
impl LocalContext {
pub fn load(paths: &AppPaths) -> Result<Self> {
let deadlines = load_deadlines(&paths.deadlines_path)?;
let timetable = load_timetable(&paths.timetable_path)?;
let materials_manifest = paths.materials_dir.join("manifest.json");
let materials = load_materials(&materials_manifest)?;
⋮----
Ok(Self {
⋮----
pub fn upcoming_deadline_count(&self) -> usize {
⋮----
.iter()
.filter(|deadline| {
⋮----
.map(|due_at| due_at <= horizon)
.unwrap_or(true)
⋮----
.count()
⋮----
pub fn search_materials(
⋮----
.map(|term| term.trim().to_lowercase())
.filter(|term| !term.is_empty())
⋮----
.filter_map(|entry| {
⋮----
.map(|course| entry.course.eq_ignore_ascii_case(course))
.unwrap_or(true);
⋮----
let haystack = format!(
⋮----
.to_lowercase();
⋮----
let mut score = if course_filter.is_some() { 2 } else { 0 };
if normalized_terms.is_empty() {
⋮----
if haystack.contains(term) {
⋮----
(score > 0).then(|| (score, entry.clone()))
⋮----
scored.sort_by(|left, right| {
⋮----
.cmp(&left.0)
.then_with(|| left.1.title.cmp(&right.1.title))
⋮----
.into_iter()
.take(limit)
.map(|(_, entry)| entry)
.collect()
⋮----
pub fn next_timetable_slots(&self, limit: usize) -> Vec<TimetableSlot> {
⋮----
let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
let today_index = weekday_index(now.weekday());
⋮----
.filter_map(|slot| {
let slot_index = weekday_index(parse_weekday(&slot.day)?);
⋮----
Some((day_distance, slot.start.clone(), slot.clone()))
⋮----
slots.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
⋮----
.map(|(_, _, slot)| slot)
⋮----
pub fn today_timetable_slots(&self) -> Vec<TimetableSlot> {
⋮----
timetable.slots_for_weekday(now.weekday())
⋮----
impl TimetableData {
pub fn slots_for_weekday(&self, weekday: Weekday) -> Vec<TimetableSlot> {
let target = weekday_index(weekday);
⋮----
.filter(|slot| {
parse_weekday(&slot.day)
.map(|day| weekday_index(day) == target)
.unwrap_or(false)
⋮----
.cloned()
⋮----
slots.sort_by(|left, right| left.start.cmp(&right.start));
⋮----
pub fn load_deadlines(path: &Path) -> Result<Vec<DeadlineEntry>> {
let mut deadlines = load_json_file::<Vec<DeadlineEntry>>(path)?.unwrap_or_default();
deadlines.sort_by(|left, right| left.due_at.cmp(&right.due_at));
Ok(deadlines)
⋮----
pub fn save_deadlines(path: &Path, deadlines: &[DeadlineEntry]) -> Result<()> {
if let Some(parent) = path.parent() {
⋮----
let mut entries = deadlines.to_vec();
entries.sort_by(|left, right| left.due_at.cmp(&right.due_at));
⋮----
Ok(())
⋮----
pub fn upsert_deadline(path: &Path, entry: DeadlineEntry) -> Result<Vec<DeadlineEntry>> {
let mut deadlines = load_deadlines(path)?;
⋮----
.iter_mut()
.find(|deadline| deadline.id == entry.id)
⋮----
deadlines.push(entry);
⋮----
save_deadlines(path, &deadlines)?;
⋮----
pub fn load_materials(path: &Path) -> Result<Vec<MaterialEntry>> {
let mut materials = load_json_file::<Vec<MaterialEntry>>(path)?.unwrap_or_default();
materials.sort_by(|left, right| left.title.cmp(&right.title));
Ok(materials)
⋮----
pub fn load_timetable(path: &Path) -> Result<Option<TimetableData>> {
⋮----
sort_timetable_slots(&mut data.slots);
⋮----
Ok(timetable)
⋮----
pub fn save_timetable(path: &Path, timetable: &TimetableData) -> Result<()> {
⋮----
let mut data = timetable.clone();
⋮----
pub fn append_timetable_slot(
⋮----
let mut timetable = load_timetable(path)?.unwrap_or(TimetableData {
timezone: timezone.clone(),
⋮----
if timetable.timezone.trim().is_empty() {
⋮----
timetable.slots.push(slot);
save_timetable(path, &timetable)?;
⋮----
fn parse_weekday(day: &str) -> Option<Weekday> {
match day.trim().to_ascii_lowercase().as_str() {
"monday" => Some(Weekday::Monday),
"tuesday" => Some(Weekday::Tuesday),
"wednesday" => Some(Weekday::Wednesday),
"thursday" => Some(Weekday::Thursday),
"friday" => Some(Weekday::Friday),
"saturday" => Some(Weekday::Saturday),
"sunday" => Some(Weekday::Sunday),
⋮----
fn weekday_index(day: Weekday) -> u8 {
⋮----
fn sort_timetable_slots(slots: &mut [TimetableSlot]) {
slots.sort_by(|left, right| {
let left_day = parse_weekday(&left.day)
.map(weekday_index)
.unwrap_or(u8::MAX);
let right_day = parse_weekday(&right.day)
⋮----
.cmp(&right_day)
.then_with(|| left.start.cmp(&right.start))
.then_with(|| left.title.cmp(&right.title))
⋮----
fn load_json_file<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
if !path.exists() {
return Ok(None);
⋮----
Ok(Some(serde_json::from_str(&raw)?))
⋮----
mod tests {
⋮----
fn temp_json_path() -> std::path::PathBuf {
let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
let path = std::env::temp_dir().join(format!(
⋮----
fn deadline_round_trip_sorts_by_due_time() {
let path = temp_json_path();
let deadlines = vec![
⋮----
save_deadlines(&path, &deadlines)
.unwrap_or_else(|err| panic!("save deadlines failed: {err}"));
⋮----
load_deadlines(&path).unwrap_or_else(|err| panic!("load deadlines failed: {err}"));
⋮----
assert_eq!(loaded[0].id, "earlier");
assert_eq!(loaded[1].id, "later");
⋮----
fn upcoming_deadline_count_ignores_far_future_entries() {
⋮----
deadlines: vec![
⋮----
assert_eq!(context.upcoming_deadline_count(), 1);
⋮----
fn search_materials_prefers_course_and_term_matches() {
⋮----
materials: vec![
⋮----
let matches = context.search_materials(
Some("Probability & Statistics for Scientists"),
⋮----
assert_eq!(matches.len(), 1);
assert_eq!(matches[0].id, "variance-notes");
⋮----
fn next_timetable_slots_returns_sorted_upcoming_slots() {
⋮----
timetable: Some(TimetableData {
timezone: "Europe/London".to_string(),
slots: vec![
⋮----
let slots = context.next_timetable_slots(2);
assert_eq!(slots.len(), 2);
assert!(
⋮----
fn timetable_round_trip_sorts_slots() {
⋮----
save_timetable(&path, &timetable)
.unwrap_or_else(|err| panic!("save timetable failed: {err}"));
let loaded = load_timetable(&path)
.unwrap_or_else(|err| panic!("load timetable failed: {err}"))
.unwrap_or_else(|| panic!("timetable should be present"));
⋮----
assert_eq!(loaded.slots[0].day, "monday");
assert_eq!(loaded.slots[1].day, "wednesday");
```

## File: crates/studyos-core/src/store.rs
```rust
use anyhow::Result;
⋮----
use crate::SessionRecapSummary;
⋮----
pub struct AppStats {
⋮----
pub struct ResumeStateRecord {
⋮----
pub struct SessionRecord {
⋮----
pub struct AttemptRecord {
⋮----
pub struct MisconceptionInput {
⋮----
pub struct DueReviewSummary {
⋮----
pub struct MisconceptionSummary {
⋮----
pub struct SessionRecapRecord {
⋮----
pub struct AppDatabase {
⋮----
impl AppDatabase {
pub fn open(path: &Path) -> Result<Self> {
⋮----
database.initialize_schema()?;
Ok(database)
⋮----
pub fn stats(&self) -> Result<AppStats> {
let due_reviews = self.count_query(
⋮----
let upcoming_deadlines = self.count_query(
⋮----
let total_attempts = self.count_query("SELECT COUNT(*) FROM attempts")?;
let total_sessions = self.count_query("SELECT COUNT(*) FROM sessions")?;
⋮----
Ok(AppStats {
⋮----
pub fn load_resume_state(&self) -> Result<Option<ResumeStateRecord>> {
⋮----
.query_row(
⋮----
Ok(ResumeStateRecord {
session_id: row.get(0)?,
runtime_thread_id: row.get(1)?,
active_mode: row.get(2)?,
active_question_id: row.get(3)?,
focused_panel: row.get(4)?,
draft_payload: row.get(5)?,
scratchpad_text: row.get(6)?,
⋮----
.optional()?;
⋮----
Ok(record)
⋮----
pub fn save_resume_state(&self, record: &ResumeStateRecord) -> Result<()> {
self.connection.execute(
⋮----
params![
⋮----
Ok(())
⋮----
pub fn start_session(&self, record: &SessionRecord) -> Result<()> {
⋮----
params![record.id, record.planned_minutes, record.mode],
⋮----
pub fn complete_session(
⋮----
params![session_id, actual_minutes, outcome_summary, aborted_reason],
⋮----
pub fn save_session_recap(&self, record: &SessionRecapRecord) -> Result<()> {
⋮----
pub fn record_attempt(
⋮----
self.ensure_concept_state(&attempt.concept_id)?;
self.update_concept_state(attempt)?;
⋮----
self.upsert_misconception(misconception)?;
⋮----
pub fn resolve_concept_id(&self, candidates: &[String]) -> Result<Option<String>> {
⋮----
params![candidate],
⋮----
if resolved.is_some() {
return Ok(resolved);
⋮----
Ok(None)
⋮----
pub fn list_due_reviews(&self, limit: usize) -> Result<Vec<DueReviewSummary>> {
let mut statement = self.connection.prepare(
⋮----
let rows = statement.query_map(params![limit as i64], |row| {
Ok(DueReviewSummary {
concept_id: row.get(0)?,
concept_name: row.get(1)?,
next_review_at: row.get(2)?,
⋮----
summaries.push(row?);
⋮----
Ok(summaries)
⋮----
pub fn list_recent_misconceptions(&self, limit: usize) -> Result<Vec<MisconceptionSummary>> {
⋮----
Ok(MisconceptionSummary {
concept_name: row.get(0)?,
error_type: row.get(1)?,
description: row.get(2)?,
last_seen_at: row.get(3)?,
⋮----
pub fn latest_session_recap(&self) -> Result<Option<SessionRecapSummary>> {
⋮----
.map(|raw| serde_json::from_str::<SessionRecapSummary>(&raw).map_err(Into::into))
.transpose()
⋮----
fn count_query(&self, sql: &str) -> Result<usize> {
⋮----
.query_row(sql, [], |row| row.get::<_, i64>(0))?;
Ok(count as usize)
⋮----
fn ensure_concept_state(&self, concept_id: &str) -> Result<()> {
⋮----
params![concept_id],
⋮----
fn update_concept_state(&self, attempt: &AttemptRecord) -> Result<()> {
⋮----
self.connection.query_row(
⋮----
params![attempt.concept_id],
⋮----
Ok((
⋮----
attempt.correctness.as_str(),
attempt.reasoning_quality.as_str(),
⋮----
let mastery_estimate = clamp(current_mastery + mastery_delta, 0.0, 1.0);
let retrieval_strength = clamp(current_retrieval + retrieval_delta, 0.0, 1.0);
let stability_days = clamp(current_stability + stability_delta, 0.0, 60.0);
let ease_factor = clamp(current_ease + ease_delta, 1.3, 3.0);
⋮----
Some("datetime('now')")
⋮----
&format!(
⋮----
fn upsert_misconception(&self, misconception: &MisconceptionInput) -> Result<()> {
⋮----
params![id],
⋮----
fn initialize_schema(&self) -> Result<()> {
self.connection.execute_batch(
⋮----
self.migrate_resume_state()?;
self.migrate_sessions_recap_payload()?;
self.seed_default_concepts()?;
⋮----
fn migrate_resume_state(&self) -> Result<()> {
let mut statement = self.connection.prepare("PRAGMA table_info(resume_state)")?;
let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
⋮----
fn migrate_sessions_recap_payload(&self) -> Result<()> {
let mut statement = self.connection.prepare("PRAGMA table_info(sessions)")?;
⋮----
.execute("ALTER TABLE sessions ADD COLUMN recap_payload TEXT", [])?;
⋮----
fn seed_default_concepts(&self) -> Result<()> {
⋮----
params![id, course, name, tags],
⋮----
fn make_record_id(prefix: &str, seed: &str) -> String {
⋮----
seed.hash(&mut hasher);
let seed_hash = hasher.finish();
⋮----
.duration_since(UNIX_EPOCH)
.map(|duration| duration.as_nanos())
.unwrap_or(0);
format!("{prefix}-{nanos:x}-{seed_hash:x}")
⋮----
fn clamp(value: f64, min: f64, max: f64) -> f64 {
value.max(min).min(max)
⋮----
mod tests {
⋮----
fn temp_db_dir() -> std::path::PathBuf {
let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
Ok(duration) => duration.as_nanos(),
⋮----
let dir = env::temp_dir().join(format!("studyos-test-{}-{nanos}", std::process::id()));
⋮----
fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("failed to create temp dir: {err}"));
⋮----
fn database_bootstrap_seeds_initial_stats() {
let dir = temp_db_dir();
let path = dir.join("studyos.db");
⋮----
.unwrap_or_else(|err| panic!("database open failed: {err}"));
⋮----
.stats()
.unwrap_or_else(|err| panic!("stats query failed: {err}"));
⋮----
assert_eq!(stats.due_reviews, 0);
assert_eq!(stats.upcoming_deadlines, 0);
⋮----
fn resume_state_round_trips() {
⋮----
session_id: "test-session".to_string(),
runtime_thread_id: Some("runtime-thread".to_string()),
active_mode: "Study".to_string(),
active_question_id: Some("4".to_string()),
focused_panel: "Scratchpad".to_string(),
draft_payload: "draft = true".to_string(),
scratchpad_text: "rough working".to_string(),
⋮----
.save_resume_state(&record)
.unwrap_or_else(|err| panic!("resume save failed: {err}"));
⋮----
.load_resume_state()
.unwrap_or_else(|err| panic!("resume load failed: {err}"))
.unwrap_or_else(|| panic!("missing resume state"));
⋮----
assert_eq!(loaded.session_id, record.session_id);
assert_eq!(loaded.runtime_thread_id, record.runtime_thread_id);
assert_eq!(loaded.focused_panel, record.focused_panel);
assert_eq!(loaded.scratchpad_text, record.scratchpad_text);
⋮----
fn attempt_logging_updates_reviews_and_misconceptions() {
⋮----
.start_session(&SessionRecord {
id: "session-1".to_string(),
⋮----
mode: "Study".to_string(),
⋮----
.unwrap_or_else(|err| panic!("session start failed: {err}"));
⋮----
.record_attempt(
⋮----
id: "attempt-1".to_string(),
session_id: "session-1".to_string(),
concept_id: "matrix_multiplication_dims".to_string(),
question_type: "retrieval_response".to_string(),
prompt_hash: "abc123".to_string(),
student_answer: "rows and columns mismatched".to_string(),
correctness: "incorrect".to_string(),
⋮----
reasoning_quality: "missing".to_string(),
feedback_summary: "You mixed up inner and outer dimensions.".to_string(),
⋮----
Some(&MisconceptionInput {
⋮----
error_type: "conceptual_misunderstanding".to_string(),
description: "Confused inner and outer dimensions.".to_string(),
⋮----
.unwrap_or_else(|err| panic!("attempt record failed: {err}"));
⋮----
.list_due_reviews(5)
.unwrap_or_else(|err| panic!("due review query failed: {err}"));
⋮----
.list_recent_misconceptions(5)
.unwrap_or_else(|err| panic!("misconception query failed: {err}"));
⋮----
assert_eq!(stats.total_attempts, 1);
assert_eq!(stats.total_sessions, 1);
assert!(!reviews.is_empty());
assert_eq!(reviews[0].concept_id, "matrix_multiplication_dims");
assert_eq!(misconceptions.len(), 1);
assert_eq!(
⋮----
fn session_recap_round_trips() {
⋮----
id: "session-recap".to_string(),
⋮----
outcome_summary: "Recovered the matrix product rule.".to_string(),
demonstrated_concepts: vec!["Matrix multiplication dimensions".to_string()],
weak_concepts: vec!["Explaining why rows dot beta".to_string()],
next_review_items: vec!["Revisit matrix-vector products tomorrow".to_string()],
unfinished_objectives: vec![
⋮----
.save_session_recap(&SessionRecapRecord {
session_id: "session-recap".to_string(),
recap: recap.clone(),
⋮----
.unwrap_or_else(|err| panic!("save recap failed: {err}"));
⋮----
.latest_session_recap()
.unwrap_or_else(|err| panic!("load recap failed: {err}"))
.unwrap_or_else(|| panic!("missing recap"));
⋮----
assert_eq!(loaded, recap);
```

## File: crates/studyos-core/src/tutor.rs
```rust
pub enum TutorBlock {
⋮----
pub struct TutorQuestion {
⋮----
pub enum TutorCorrectness {
⋮----
pub enum TutorReasoningQuality {
⋮----
pub enum TutorErrorType {
⋮----
pub struct TutorMisconception {
⋮----
pub struct TutorEvaluation {
⋮----
pub struct TutorTurnPayload {
⋮----
pub struct TutorSessionClosePayload {
⋮----
impl TutorTurnPayload {
pub fn into_content_blocks(self) -> Vec<ContentBlock> {
⋮----
blocks.push(mapped);
⋮----
blocks.push(ContentBlock::QuestionCard(QuestionCard {
⋮----
mod tests {
⋮----
fn tutor_payload_maps_into_content_blocks() {
⋮----
teaching_blocks: vec![
⋮----
question: Some(TutorQuestion {
title: "Dimension Check".to_string(),
prompt: "What dimensions must match before multiplying AB?".to_string(),
concept_tags: vec!["matrix multiplication".to_string()],
⋮----
let blocks = payload.into_content_blocks();
assert_eq!(blocks.len(), 3);
assert!(matches!(blocks[0], ContentBlock::Paragraph(_)));
assert!(matches!(blocks[1], ContentBlock::MathBlock(_)));
⋮----
assert_eq!(card.matrix_dimensions, None);
⋮----
other => panic!("expected question card, got {other:?}"),
```

## File: crates/studyos-core/src/session.rs
```rust
pub enum SessionMode {
⋮----
impl SessionMode {
pub fn label(self) -> &'static str {
⋮----
pub fn from_label(label: &str) -> Self {
⋮----
pub enum PanelTab {
⋮----
impl PanelTab {
⋮----
pub enum DeadlineUrgency {
⋮----
impl DeadlineUrgency {
⋮----
pub struct SessionPlanSummary {
⋮----
pub struct SessionRecapSummary {
⋮----
pub struct StartupReviewItem {
⋮----
pub struct StartupMisconceptionItem {
⋮----
pub struct BootstrapStudyContext {
⋮----
pub enum ActivityStatus {
⋮----
pub struct ActivityItem {
⋮----
pub struct SessionMetrics {
⋮----
pub struct KeybindingHint {
⋮----
pub struct AppSnapshot {
⋮----
impl AppSnapshot {
pub fn bootstrap(
⋮----
let mode = choose_start_mode(stats, study_context, urgency);
let plan = build_start_plan(config, stats, study_context, mode, urgency);
⋮----
course: config.default_course.clone(),
⋮----
transcript: bootstrap_transcript(&config.default_course),
widget: bootstrap_widget(&config.default_course),
scratchpad: "Use this scratchpad for rough working that should not be submitted.\n- jot down row operations\n- note shortcuts\n- park reminders".to_string(),
activity: vec![
⋮----
keybindings: vec![
⋮----
fn bootstrap_transcript(course: &str) -> Vec<ContentBlock> {
if is_probability_course(course) {
return probability_bootstrap_transcript();
⋮----
linear_algebra_bootstrap_transcript()
⋮----
fn bootstrap_widget(course: &str) -> ResponseWidget {
⋮----
fn is_probability_course(course: &str) -> bool {
let normalized = course.to_lowercase();
normalized.contains("probability") || normalized.contains("statistics")
⋮----
fn linear_algebra_bootstrap_transcript() -> Vec<ContentBlock> {
vec![
⋮----
fn probability_bootstrap_transcript() -> Vec<ContentBlock> {
⋮----
fn choose_start_mode(
⋮----
.iter()
.any(|item| item.error_type == "conceptual_misunderstanding");
⋮----
} else if matches!(urgency, DeadlineUrgency::Urgent) && stats.total_attempts > 0 {
⋮----
fn build_start_plan(
⋮----
recommended_duration_minutes: config.default_session_minutes.min(35),
why_now: if !study_context.recent_misconceptions.is_empty() {
format!(
⋮----
.to_string()
⋮----
.take(2)
.map(|item| format!("Retrieve the key rule for {}.", item.concept_name))
.chain(
⋮----
.take(1)
.map(|item| format!("Explain the mistake behind: {}", item.description)),
⋮----
.collect(),
⋮----
.take(3)
.map(|item| item.concept_name.clone())
⋮----
stretch_target: Some(
"Only move on if the repair question is genuinely secure.".to_string(),
⋮----
recommended_duration_minutes: config.default_session_minutes.min(30),
⋮----
.to_string(),
warm_up_questions: vec![
⋮----
core_targets: vec![
⋮----
stretch_target: Some(match urgency {
⋮----
_ => "Finish with one short transfer prompt.".to_string(),
⋮----
if let Some(objective) = recap.unfinished_objectives.first() {
⋮----
.as_ref()
.and_then(|recap| recap.unfinished_objectives.first().cloned())
.unwrap_or_else(|| {
"Connect determinant zero to linear dependence.".to_string()
⋮----
mod tests {
⋮----
use crate::AppConfig;
⋮----
fn stats(due_reviews: usize, upcoming_deadlines: usize, total_attempts: usize) -> AppStats {
⋮----
fn bootstrap_routes_to_review_when_due_reviews_exist() {
⋮----
&stats(3, 0, 2),
⋮----
due_reviews: vec![StartupReviewItem {
⋮----
assert_eq!(snapshot.mode, SessionMode::Review);
assert_eq!(snapshot.panel_tab, PanelTab::DueReviews);
⋮----
fn bootstrap_routes_to_drill_when_deadline_is_urgent() {
⋮----
AppSnapshot::bootstrap(&config, &stats(0, 2, 4), &BootstrapStudyContext::default());
⋮----
assert_eq!(snapshot.mode, SessionMode::Drill);
assert_eq!(snapshot.panel_tab, PanelTab::Deadlines);
⋮----
fn bootstrap_uses_unfinished_objectives_from_last_session() {
⋮----
&stats(0, 0, 1),
⋮----
last_session_recap: Some(SessionRecapSummary {
outcome_summary: "Stopped mid repair.".to_string(),
⋮----
weak_concepts: vec!["Matrix multiplication".to_string()],
⋮----
unfinished_objectives: vec![
⋮----
assert_eq!(snapshot.mode, SessionMode::Study);
assert!(
⋮----
assert_eq!(
⋮----
fn bootstrap_transcript_matches_probability_course() {
⋮----
default_course: "Probability & Statistics for Scientists".to_string(),
⋮----
AppSnapshot::bootstrap(&config, &stats(0, 0, 0), &BootstrapStudyContext::default());
⋮----
assert!(matches!(
⋮----
.find_map(|block| match block {
ContentBlock::QuestionCard(card) => Some(card),
⋮----
.unwrap_or_else(|| panic!("probability bootstrap should include a question card"));
⋮----
assert!(first_question.title.contains("Expectation"));
```

## File: crates/studyos-cli/src/app.rs
```rust
pub enum FocusRegion {
⋮----
impl FocusRegion {
pub fn label(self) -> &'static str {
⋮----
pub fn next(self) -> Self {
⋮----
pub enum AppAction {
⋮----
pub struct AppBootstrap {
⋮----
struct PendingAttemptContext {
⋮----
struct PendingTurn {
⋮----
pub struct App {
⋮----
impl App {
pub fn new(bootstrap: AppBootstrap) -> Self {
⋮----
let active_question_index = *question_indices.first().unwrap_or(&0);
⋮----
.iter()
.filter_map(|index| {
⋮----
Some((*index, widget_state_for_question(card)))
⋮----
.collect();
⋮----
.map(|index| (*index, Instant::now()))
⋮----
let session_seed = config.default_course.clone();
⋮----
current_session_id: make_id("session", &session_seed),
⋮----
app.apply_resume_state(resume);
⋮----
app.set_activity(
⋮----
"Resume state is now loaded from local SQLite when available.".to_string(),
⋮----
format!(
⋮----
Some(error) => app.set_activity("App-server", error, ActivityStatus::Idle),
None if app.runtime.is_some() => app.set_activity(
⋮----
"Codex app-server process spawned; waiting for initialization.".to_string(),
⋮----
None => app.set_activity(
⋮----
.to_string(),
⋮----
.start_session_record()
.and_then(|_| app.refresh_snapshot_metrics())
⋮----
Ok(()) => app.set_activity(
⋮----
"Local study memory opened, session recorded, and metrics refreshed.".to_string(),
⋮----
Err(error) => app.set_activity(
⋮----
format!("Failed to start session record: {error}"),
⋮----
pub fn bootstrap_runtime(&mut self) -> Result<()> {
if self.runtime.is_none() {
return Ok(());
⋮----
let developer_instructions = self.developer_instructions();
let cwd = self.paths.root_dir.parent().unwrap_or(&self.paths.root_dir);
⋮----
.as_ref()
.ok_or_else(|| anyhow!("runtime unavailable"))?;
runtime.initialize()?;
⋮----
let thread_id = if let Some(existing) = self.runtime_thread_id.as_deref() {
runtime.resume_thread(existing, cwd)?
⋮----
runtime.start_thread(cwd, &developer_instructions)?
⋮----
self.runtime_thread_id = Some(thread_id.clone());
⋮----
let opening_prompt = self.build_opening_prompt();
let turn_id = runtime.start_structured_turn(
⋮----
tutor_output_schema(),
⋮----
self.pending_structured_turns.insert(
⋮----
self.set_activity(
⋮----
"Connected to Codex app-server and started structured tutor turn.".to_string(),
⋮----
self.persist_resume_state()?;
Ok(())
⋮----
pub fn poll_runtime(&mut self) {
⋮----
let events = runtime.poll_events();
⋮----
self.handle_runtime_event(event);
⋮----
pub fn handle_key(&mut self, key: KeyEvent) -> Option<AppAction> {
⋮----
if self.quit_recap_preview.is_some() {
⋮----
"Returned to the active study session without closing.".to_string(),
⋮----
self.open_quit_recap_review();
⋮----
self.focus = self.focus.next();
⋮----
return Some(AppAction::SubmitCurrentAnswer);
⋮----
self.advance_question(1);
⋮----
self.advance_question(-1);
⋮----
FocusRegion::Transcript => self.handle_transcript_key(key),
FocusRegion::Panel => self.handle_panel_key(key),
FocusRegion::Widget => self.handle_widget_key(key),
FocusRegion::Scratchpad => self.handle_scratchpad_key(key),
⋮----
pub fn execute_action(&mut self, action: AppAction) {
if let Err(error) = self.execute_action_inner(action) {
self.push_block(ContentBlock::WarningBox(WarningBox {
title: "Runtime action failed".to_string(),
body: error.to_string(),
⋮----
self.set_activity("App-server", error.to_string(), ActivityStatus::Idle);
⋮----
pub fn finish_session(&mut self) -> Result<()> {
⋮----
let actual_minutes = (self.session_started_at.elapsed().as_secs() / 60) as i64;
let recap = match self.generate_session_recap() {
⋮----
format!("Close recap fell back to local summary: {error}"),
⋮----
self.fallback_session_recap()
⋮----
let outcome_summary = recap.outcome_summary.clone();
⋮----
self.database.complete_session(
⋮----
self.database.save_session_recap(&SessionRecapRecord {
session_id: self.current_session_id.clone(),
⋮----
self.refresh_snapshot_metrics()?;
⋮----
pub fn active_widget(&self) -> Option<&ResponseWidget> {
self.widget_states.get(&self.active_question_index)
⋮----
pub fn quit_recap_preview(&self) -> Option<&SessionRecapSummary> {
self.quit_recap_preview.as_ref()
⋮----
pub fn current_mode_label(&self) -> &'static str {
⋮----
SessionMode::Recap.label()
⋮----
self.snapshot.mode.label()
⋮----
pub fn active_widget_mut(&mut self) -> Option<&mut ResponseWidget> {
self.widget_states.get_mut(&self.active_question_index)
⋮----
pub fn persist_resume_state(&self) -> Result<()> {
⋮----
.active_widget()
.map(toml::to_string)
.transpose()?
.unwrap_or_default();
⋮----
session_id: "study-session".to_string(),
runtime_thread_id: self.runtime_thread_id.clone(),
active_mode: self.snapshot.mode.label().to_string(),
active_question_id: Some(self.active_question_index.to_string()),
focused_panel: self.snapshot.panel_tab.label().to_string(),
⋮----
scratchpad_text: self.snapshot.scratchpad.clone(),
⋮----
self.database.save_resume_state(&record)
⋮----
pub fn active_question_title(&self) -> String {
⋮----
.get(self.active_question_index)
.and_then(|block| match block {
ContentBlock::QuestionCard(card) => Some(card.title.clone()),
⋮----
.unwrap_or_else(|| "Structured Answer".to_string())
⋮----
pub fn active_question_prompt(&self) -> Option<String> {
⋮----
ContentBlock::QuestionCard(card) => Some(card.prompt.clone()),
⋮----
pub fn question_indices(&self) -> Vec<usize> {
⋮----
pub fn status_line(&self) -> String {
⋮----
} else if self.runtime.is_some() {
⋮----
let quit_label = if self.quit_recap_preview.is_some() {
⋮----
pub fn misconceptions_summary(&self) -> Vec<String> {
match self.database.list_recent_misconceptions(4) {
Ok(entries) if !entries.is_empty() => {
let mut lines = vec!["Recent recurring misconceptions:".to_string()];
⋮----
lines.push(format!(
⋮----
lines.push(format!("  {}", entry.description));
⋮----
Ok(_) => vec![
⋮----
Err(error) => vec![format!("Misconception summary unavailable: {error}")],
⋮----
pub fn review_summary(&self) -> Vec<String> {
match self.database.list_due_reviews(4) {
Ok(reviews) if !reviews.is_empty() => {
let mut lines = vec![format!(
⋮----
Err(error) => vec![format!("Review queue unavailable: {error}")],
⋮----
pub fn deadline_summary(&self) -> Vec<String> {
let mut lines = vec![
⋮----
if self.local_context.deadlines.is_empty() {
⋮----
for deadline in self.local_context.deadlines.iter().take(3) {
lines.push(format!("• {} ({})", deadline.title, deadline.due_at));
⋮----
for slot in self.local_context.next_timetable_slots(3) {
⋮----
fn execute_action_inner(&mut self, action: AppAction) -> Result<()> {
⋮----
AppAction::SubmitCurrentAnswer => self.submit_current_answer(),
⋮----
fn generate_session_recap(&self) -> Result<SessionRecapSummary> {
if !self.pending_structured_turns.is_empty() {
return Ok(self.fallback_session_recap());
⋮----
let prompt = self.build_close_prompt();
let raw = runtime.run_structured_turn_and_wait(
⋮----
tutor_close_output_schema(),
⋮----
Ok(payload.recap)
⋮----
fn fallback_session_recap(&self) -> SessionRecapSummary {
⋮----
.list_recent_misconceptions(3)
.unwrap_or_default()
.into_iter()
.map(|item| item.concept_name)
⋮----
weak_concepts.dedup();
⋮----
let unfinished_objectives = match self.active_question_prompt() {
Some(prompt) if !prompt.trim().is_empty() => vec![prompt],
⋮----
outcome_summary: if self.session_outcomes.is_empty() {
"Session ended before any graded evidence was captured.".to_string()
⋮----
self.session_outcomes.join(" | ")
⋮----
.list_due_reviews(3)
⋮----
.collect(),
⋮----
.map(|item| format!("{} at {}", item.concept_name, item.next_review_at))
⋮----
fn start_session_record(&mut self) -> Result<()> {
⋮----
id: self.current_session_id.clone(),
⋮----
mode: self.snapshot.mode.label().to_string(),
⋮----
self.database.start_session(&record)
⋮----
fn refresh_snapshot_metrics(&mut self) -> Result<()> {
let mut stats = self.database.stats()?;
stats.upcoming_deadlines = self.local_context.upcoming_deadline_count();
self.stats = stats.clone();
⋮----
fn submit_current_answer(&mut self) -> Result<()> {
if let Some(warning) = self.active_widget().and_then(widget_validation_warning) {
self.push_block(ContentBlock::WarningBox(warning));
⋮----
.ok_or_else(|| anyhow!("app-server runtime is unavailable"))?;
⋮----
.clone()
.ok_or_else(|| anyhow!("no runtime thread is active"))?;
let prompt = self.build_submission_prompt();
let attempt = self.build_pending_attempt_context();
⋮----
runtime.start_structured_turn(&thread_id, &prompt, tutor_output_schema(), cwd)?;
⋮----
display_user_text: Some(format!("Submitted answer: {}", attempt.question_title)),
attempt: Some(attempt),
⋮----
"Submitted structured student answer for grading and next-step planning.".to_string(),
⋮----
fn build_opening_prompt(&self) -> String {
let deadlines = if self.local_context.deadlines.is_empty() {
"No local deadlines loaded.".to_string()
⋮----
.take(3)
.map(|deadline| format!("{} due {}", deadline.title, deadline.due_at))
⋮----
.join("; ")
⋮----
.map(|course| course.title.as_str())
⋮----
.join(", ");
⋮----
.map(|review| review.concept_name)
⋮----
.map(|item| format!("{}: {}", item.concept_name, item.description))
⋮----
.join("; ");
⋮----
.chain(
⋮----
.map(|item| item.concept_name),
⋮----
.search_materials(Some(&self.snapshot.course), &material_terms, 3)
⋮----
.map(|entry| {
⋮----
.next_timetable_slots(3)
⋮----
.map(|slot| format!("{} {}-{} {}", slot.day, slot.start, slot.end, slot.title))
⋮----
fn build_submission_prompt(&self) -> String {
let answer = self.widget_submission_summary();
let title = self.active_question_title();
⋮----
.active_question_prompt()
.unwrap_or_else(|| "No prompt recorded.".to_string());
⋮----
fn build_close_prompt(&self) -> String {
let evidence = if self.session_outcomes.is_empty() {
"No graded outcomes were captured in this session.".to_string()
⋮----
.list_due_reviews(4)
⋮----
.map(|item| format!("{} due {}", item.concept_name, item.next_review_at))
⋮----
.list_recent_misconceptions(4)
⋮----
.map(|item| {
⋮----
.unwrap_or_else(|| "No active question remained open.".to_string());
⋮----
fn widget_submission_summary(&self) -> String {
match self.active_widget() {
⋮----
.map(|row| {
⋮----
.map(|cell| {
if cell.trim().is_empty() {
"·".to_string()
⋮----
cell.clone()
⋮----
format!("[{}]", values.join(", "))
⋮----
.join("\n");
⋮----
Some(ResponseWidget::WorkingAnswer(state)) => format!(
⋮----
Some(ResponseWidget::StepList(state)) => format!(
⋮----
format!("widget: retrieval_response\n{}", state.response)
⋮----
None => "widget: none\nNo active widget state.".to_string(),
⋮----
fn open_quit_recap_review(&mut self) {
let recap = self.fallback_session_recap();
self.quit_recap_preview = Some(recap);
⋮----
fn build_pending_attempt_context(&self) -> PendingAttemptContext {
let question = self.snapshot.transcript.get(self.active_question_index);
⋮----
card.title.clone(),
card.prompt.clone(),
card.concept_tags.clone(),
⋮----
self.active_question_title(),
self.active_question_prompt()
.unwrap_or_else(|| "No prompt recorded.".to_string()),
⋮----
self.active_widget()
.map(ResponseWidget::kind)
.unwrap_or(ResponseWidgetKind::RetrievalResponse),
⋮----
.get(&self.active_question_index)
.map(|started| started.elapsed().as_millis() as i64)
.unwrap_or(0);
⋮----
student_answer: self.widget_submission_summary(),
⋮----
fn persist_evaluation(
⋮----
let concept_id = self.resolve_concept_id(&context.concept_tags);
let correctness = correctness_label(&evaluation.correctness);
let reasoning_quality = reasoning_quality_label(&evaluation.reasoning_quality);
let feedback_summary = evaluation.feedback_summary.trim().to_string();
let prompt_hash = stable_hash(&context.question_prompt);
⋮----
id: make_id("attempt", &context.question_prompt),
⋮----
concept_id: concept_id.clone(),
question_type: widget_kind_label(context.widget_kind).to_string(),
⋮----
student_answer: context.student_answer.clone(),
correctness: correctness.to_string(),
⋮----
reasoning_quality: reasoning_quality.to_string(),
feedback_summary: feedback_summary.clone(),
⋮----
.map(|item| MisconceptionInput {
⋮----
error_type: error_type_label(&item.error_type).to_string(),
description: item.description.clone(),
⋮----
.record_attempt(&attempt, misconception.as_ref())?;
⋮----
.unwrap_or_else(|| format!("{}: {}", context.question_title, feedback_summary));
self.session_outcomes.push(outcome.clone());
self.set_activity("Evidence", outcome, ActivityStatus::Healthy);
⋮----
fn handle_runtime_event(&mut self, event: RuntimeEvent) {
⋮----
format!("Thread ready: {}", thread_id),
⋮----
format!("Thread status changed: {status}"),
⋮----
format!("Turn started: {turn_id}"),
⋮----
self.pending_structured_turns.remove(&turn_id);
⋮----
format!("Turn completed with status: {status}"),
⋮----
self.handle_runtime_item_started(&turn_id, item);
⋮----
if self.pending_structured_turns.contains_key(&turn_id) {
⋮----
.entry(item_id)
.or_default()
.push_str(&delta);
} else if let Some(index) = self.live_message_indices.get(&item_id).copied()
⋮----
self.snapshot.transcript.get_mut(index)
⋮----
paragraph.text.push_str(&delta);
⋮----
self.handle_runtime_item_completed(&turn_id, item);
⋮----
&format!("MCP {name}"),
format!("startup status: {status}"),
⋮----
self.set_activity("App-server", message.clone(), ActivityStatus::Idle);
if message.contains("stderr") {
⋮----
title: "Runtime notice".to_string(),
⋮----
fn handle_runtime_item_started(&mut self, turn_id: &str, item: Value) {
let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
⋮----
.get("id")
.and_then(Value::as_str)
.unwrap_or("")
.to_string();
⋮----
if item_type == "agentMessage" && !self.pending_structured_turns.contains_key(turn_id) {
let index = self.snapshot.transcript.len();
⋮----
.push(ContentBlock::Paragraph(studyos_core::ParagraphBlock {
text: "Tutor: ".to_string(),
⋮----
self.live_message_indices.insert(item_id, index);
⋮----
if item_type == "agentMessage" && self.pending_structured_turns.contains_key(turn_id) {
⋮----
"Streaming structured tutor payload...".to_string(),
⋮----
fn handle_runtime_item_completed(&mut self, turn_id: &str, item: Value) {
⋮----
.get(turn_id)
.and_then(|pending| pending.display_user_text.clone())
.or_else(|| {
item.get("content")
.and_then(Value::as_array)
.and_then(|content| content.first())
.and_then(|entry| entry.get("text"))
⋮----
.map(ToOwned::to_owned)
⋮----
self.push_block(ContentBlock::Paragraph(studyos_core::ParagraphBlock {
text: format!("You: {text}"),
⋮----
.get("text")
⋮----
if self.pending_structured_turns.contains_key(turn_id) {
let structured_text = self.structured_buffers.remove(&item_id).unwrap_or(text);
self.apply_structured_tutor_payload(turn_id, &structured_text);
self.pending_structured_turns.remove(turn_id);
} else if let Some(index) = self.live_message_indices.remove(&item_id) {
⋮----
paragraph.text = format!("Tutor: {text}");
⋮----
text: format!("Tutor: {text}"),
⋮----
if let Some(text) = item.get("text").and_then(Value::as_str) {
⋮----
text: format!("Plan: {text}"),
⋮----
fn apply_structured_tutor_payload(&mut self, turn_id: &str, raw: &str) {
⋮----
.and_then(|pending| pending.attempt.clone());
⋮----
(payload.evaluation.as_ref(), evaluation_context.as_ref())
⋮----
if let Err(error) = self.persist_evaluation(context, evaluation) {
⋮----
title: "Evidence logging failed".to_string(),
⋮----
if let Some(plan) = payload.session_plan.clone() {
⋮----
self.snapshot.transcript.clear();
self.widget_states.clear();
⋮----
self.push_block(ContentBlock::Divider);
⋮----
let blocks = payload.into_content_blocks();
let previous_len = self.snapshot.transcript.len();
⋮----
self.push_block(block);
⋮----
self.rebuild_widget_state_from(previous_len);
if let Err(error) = self.refresh_snapshot_metrics() {
self.set_activity("SQLite", error.to_string(), ActivityStatus::Idle);
⋮----
"Structured tutor payload rendered successfully.".to_string(),
⋮----
title: "Structured payload parse failed".to_string(),
body: format!("{} | Raw response: {}", error, raw),
⋮----
fn rebuild_widget_state_from(&mut self, start_index: usize) {
for index in start_index..self.snapshot.transcript.len() {
if let Some(ContentBlock::QuestionCard(card)) = self.snapshot.transcript.get(index) {
⋮----
.insert(index, widget_state_for_question(card));
self.question_presented_at.insert(index, Instant::now());
⋮----
fn push_block(&mut self, block: ContentBlock) {
self.snapshot.transcript.push(block);
⋮----
fn set_activity(&mut self, name: &str, detail: String, status: ActivityStatus) {
⋮----
.iter_mut()
.find(|item| item.name == name)
⋮----
self.snapshot.activity.push(ActivityItem {
name: name.to_string(),
⋮----
fn question_indices_from(transcript: &[ContentBlock]) -> Vec<usize> {
⋮----
.enumerate()
.filter_map(|(index, block)| {
matches!(block, ContentBlock::QuestionCard(_)).then_some(index)
⋮----
.collect()
⋮----
fn advance_question(&mut self, direction: isize) {
let indices = self.question_indices();
if indices.is_empty() {
⋮----
.position(|index| *index == self.active_question_index)
⋮----
let next = if direction.is_negative() {
current.checked_sub(1).unwrap_or(indices.len() - 1)
⋮----
(current + 1) % indices.len()
⋮----
fn handle_transcript_key(&mut self, key: KeyEvent) {
⋮----
self.transcript_scroll = self.transcript_scroll.saturating_sub(1);
⋮----
self.transcript_scroll = self.transcript_scroll.saturating_add(1);
⋮----
fn handle_panel_key(&mut self, key: KeyEvent) {
⋮----
self.snapshot.panel_tab = next_panel_tab(self.snapshot.panel_tab);
⋮----
self.snapshot.panel_tab = previous_panel_tab(self.snapshot.panel_tab);
⋮----
fn handle_scratchpad_key(&mut self, key: KeyEvent) {
⋮----
self.snapshot.scratchpad.pop();
⋮----
KeyCode::Enter => self.snapshot.scratchpad.push('\n'),
KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
self.snapshot.scratchpad.push(c);
⋮----
fn handle_widget_key(&mut self, key: KeyEvent) {
let Some(widget) = self.active_widget_mut() else {
⋮----
ResponseWidget::MatrixGrid(state) => handle_matrix_widget(state, key),
ResponseWidget::WorkingAnswer(state) => handle_working_widget(state, key),
ResponseWidget::StepList(state) => handle_step_list_widget(state, key),
ResponseWidget::RetrievalResponse(state) => handle_retrieval_widget(state, key),
⋮----
fn developer_instructions(&self) -> String {
"You are the StudyOS tutor runtime. Prioritize retrieval before explanation, ask for mathematical reasoning rather than spoon-feeding, and stay concise. When the client provides an output schema, obey it strictly. Prefer one active question at a time and choose widget kinds that match the task precisely.".to_string()
⋮----
fn resolve_concept_id(&self, concept_tags: &[String]) -> String {
if let Ok(Some(concept_id)) = self.database.resolve_concept_id(concept_tags) {
⋮----
if let Some(first) = concept_tags.first() {
return normalize_identifier(first);
⋮----
"general_study_skill".to_string()
⋮----
fn apply_resume_state(&mut self, resume: ResumeStateRecord) {
⋮----
if !resume.scratchpad_text.trim().is_empty() {
⋮----
if self.widget_states.contains_key(&index) {
⋮----
if !resume.draft_payload.trim().is_empty() {
⋮----
.insert(self.active_question_index, widget);
⋮----
fn widget_state_for_question(card: &QuestionCard) -> ResponseWidget {
⋮----
.unwrap_or(MatrixDimensions { rows: 2, cols: 2 });
⋮----
steps: vec!["".to_string()],
⋮----
fn next_panel_tab(current: PanelTab) -> PanelTab {
⋮----
fn previous_panel_tab(current: PanelTab) -> PanelTab {
⋮----
fn handle_matrix_widget(state: &mut MatrixGridState, key: KeyEvent) {
⋮----
KeyCode::Left => state.selected_col = state.selected_col.saturating_sub(1),
⋮----
(state.selected_col + 1).min(state.dimensions.cols.saturating_sub(1));
⋮----
KeyCode::Up => state.selected_row = state.selected_row.saturating_sub(1),
⋮----
(state.selected_row + 1).min(state.dimensions.rows.saturating_sub(1));
⋮----
state.selected_col = (state.selected_col + 1) % state.dimensions.cols.max(1);
⋮----
state.cells[state.selected_row][state.selected_col].pop();
⋮----
state.cells[state.selected_row][state.selected_col].push(c);
⋮----
fn handle_working_widget(state: &mut WorkingAnswerState, key: KeyEvent) {
⋮----
state.working.pop();
⋮----
state.final_answer.pop();
⋮----
if matches!(state.active_field, WorkingAnswerField::Working) {
state.working.push('\n');
⋮----
WorkingAnswerField::Working => state.working.push(c),
WorkingAnswerField::FinalAnswer => state.final_answer.push(c),
⋮----
fn handle_step_list_widget(state: &mut StepListState, key: KeyEvent) {
⋮----
state.selected_step = state.selected_step.saturating_sub(1);
⋮----
(state.selected_step + 1).min(state.steps.len().saturating_sub(1));
⋮----
let insert_at = (state.selected_step + 1).min(state.steps.len());
state.steps.insert(insert_at, String::new());
⋮----
if let Some(current) = state.steps.get_mut(state.selected_step) {
if !current.is_empty() {
current.pop();
} else if state.steps.len() > 1 {
state.steps.remove(state.selected_step);
⋮----
current.push(c);
⋮----
fn handle_retrieval_widget(state: &mut RetrievalResponseState, key: KeyEvent) {
⋮----
state.response.pop();
⋮----
state.response.push(c);
⋮----
pub fn widget_validation_warning(widget: &ResponseWidget) -> Option<WarningBox> {
⋮----
.flat_map(|row| row.iter())
.any(|cell| !cell.trim().is_empty());
(!filled).then(|| WarningBox {
title: "Blank Attempt".to_string(),
⋮----
ResponseWidget::WorkingAnswer(state) => (state.working.trim().is_empty()
&& !state.final_answer.trim().is_empty())
.then(|| WarningBox {
title: "Method Missing".to_string(),
body: "This question expects working as well as a final answer.".to_string(),
⋮----
.all(|step| step.trim().is_empty())
⋮----
title: "No Reasoning Logged".to_string(),
⋮----
state.response.trim().is_empty().then(|| WarningBox {
title: "No Retrieval Attempt".to_string(),
body: "Write a short answer before asking for help or reveal.".to_string(),
⋮----
fn widget_kind_label(kind: ResponseWidgetKind) -> &'static str {
⋮----
fn correctness_label(correctness: &TutorCorrectness) -> &'static str {
⋮----
fn reasoning_quality_label(reasoning_quality: &TutorReasoningQuality) -> &'static str {
⋮----
fn error_type_label(error_type: &TutorErrorType) -> &'static str {
⋮----
fn stable_hash(text: &str) -> String {
⋮----
text.hash(&mut hasher);
format!("{:x}", hasher.finish())
⋮----
fn make_id(prefix: &str, seed: &str) -> String {
⋮----
fn normalize_identifier(text: &str) -> String {
text.chars()
.map(|character| {
if character.is_ascii_alphanumeric() {
character.to_ascii_lowercase()
⋮----
.trim_matches('_')
.to_string()
⋮----
fn tutor_output_schema() -> Value {
json!({
⋮----
fn tutor_paragraph_block_schema() -> Value {
⋮----
fn tutor_hint_block_schema() -> Value {
⋮----
fn tutor_warning_block_schema() -> Value {
⋮----
fn tutor_math_block_schema() -> Value {
⋮----
fn tutor_matrix_block_schema() -> Value {
⋮----
fn tutor_bullet_list_block_schema() -> Value {
⋮----
fn tutor_recap_block_schema() -> Value {
⋮----
fn tutor_close_output_schema() -> Value {
⋮----
mod tests {
⋮----
fn temp_data_root() -> std::path::PathBuf {
let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
let path = env::temp_dir().join(format!(
⋮----
fs::create_dir_all(&path).unwrap_or_else(|err| panic!("temp dir create failed: {err}"));
⋮----
fn structured_payload_persists_attempt_evidence() {
let base = temp_data_root();
⋮----
.ensure()
.unwrap_or_else(|err| panic!("path ensure failed: {err}"));
⋮----
.unwrap_or_else(|err| panic!("database open failed: {err}"));
⋮----
.stats()
.unwrap_or_else(|err| panic!("stats query failed: {err}"));
⋮----
paths: paths.clone(),
⋮----
if let Some(ResponseWidget::MatrixGrid(state)) = app.active_widget_mut() {
state.cells[0][0] = "1".to_string();
⋮----
let attempt = app.build_pending_attempt_context();
app.pending_structured_turns.insert(
"turn-test".to_string(),
⋮----
display_user_text: Some("Submitted answer".to_string()),
⋮----
session_plan: Some(SessionPlanSummary {
⋮----
why_now: "Repair matrix product recall.".to_string(),
warm_up_questions: vec!["When is AB defined?".to_string()],
core_targets: vec!["Matrix multiplication dimensions".to_string()],
⋮----
teaching_blocks: vec![TutorBlock::Paragraph {
⋮----
question: Some(TutorQuestion {
title: "Dimension Repair".to_string(),
prompt: "State the inner-dimension rule.".to_string(),
concept_tags: vec!["matrix_multiplication".to_string()],
⋮----
evaluation: Some(TutorEvaluation {
⋮----
misconception: Some(TutorMisconception {
⋮----
description: "Confused what product the grid was asking for.".to_string(),
⋮----
outcome_summary: Some("Matrix product recall needs repair.".to_string()),
⋮----
.unwrap_or_else(|err| panic!("payload serialization failed: {err}"));
app.apply_structured_tutor_payload("turn-test", &raw);
⋮----
.list_recent_misconceptions(5)
.unwrap_or_else(|err| panic!("misconception query failed: {err}"));
⋮----
assert_eq!(stats.total_attempts, 1);
assert_eq!(misconceptions.len(), 1);
assert_eq!(
⋮----
fn quit_review_opens_before_exit_and_can_be_cancelled() {
⋮----
app.handle_key(KeyEvent::from(KeyCode::Char('q')));
⋮----
assert!(!app.should_quit);
assert_eq!(app.current_mode_label(), "Recap");
⋮----
.quit_recap_preview()
.unwrap_or_else(|| panic!("quit recap preview should be open"));
assert!(
⋮----
assert_eq!(recap.unfinished_objectives.len(), 1);
⋮----
app.handle_key(KeyEvent::from(KeyCode::Esc));
⋮----
assert!(app.quit_recap_preview().is_none());
assert_eq!(app.current_mode_label(), "Study");
⋮----
fn structured_matrix_question_uses_declared_dimensions() {
⋮----
why_now: "Practice a rectangular matrix product.".to_string(),
warm_up_questions: vec!["What is the shape of the output?".to_string()],
core_targets: vec!["Matrix multiplication".to_string()],
⋮----
title: "Rectangular Product".to_string(),
prompt: "Enter the 2 by 3 output matrix.".to_string(),
⋮----
matrix_dimensions: Some(MatrixDimensions { rows: 2, cols: 3 }),
⋮----
app.apply_structured_tutor_payload("turn-open", &raw);
⋮----
.unwrap_or_else(|| panic!("matrix widget should be active"));
⋮----
assert_eq!(state.dimensions.rows, 2);
assert_eq!(state.dimensions.cols, 3);
assert_eq!(state.cells.len(), 2);
assert_eq!(state.cells[0].len(), 3);
⋮----
other => panic!("expected matrix widget, got {other:?}"),
⋮----
fn working_answer_widget_switches_between_fields() {
⋮----
.find_map(|(index, block)| match block {
⋮----
if matches!(card.widget_kind, ResponseWidgetKind::WorkingAnswer) =>
⋮----
Some(index)
⋮----
.unwrap_or_else(|| panic!("working-answer question should exist in bootstrap"));
⋮----
app.handle_key(KeyEvent::from(KeyCode::Char('x')));
app.handle_key(KeyEvent::from(KeyCode::Down));
app.handle_key(KeyEvent::from(KeyCode::Char('7')));
⋮----
.unwrap_or_else(|| panic!("working-answer widget should be active"));
⋮----
assert_eq!(state.working, "x");
assert_eq!(state.final_answer, "7");
assert_eq!(state.active_field, WorkingAnswerField::FinalAnswer);
⋮----
other => panic!("expected working-answer widget, got {other:?}"),
⋮----
fn opening_prompt_includes_relevant_local_materials() {
⋮----
materials: vec![MaterialEntry {
⋮----
let prompt = app.build_opening_prompt();
assert!(prompt.contains("Relevant local materials"));
assert!(prompt.contains("Matrix Multiplication Worksheet"));
```
