# StudyOS Reviewer Response and Execution Roadmap

Date: 2026-04-19
Repository: `StudyOS`
Scope: commits `fc25bdb` .. `83c0513`
Companion to: `2026-04-19-comprehensive-build-review.md`

This document exists because:

1. The entire codebase was produced by OpenAI Codex CLI.
2. No human has yet executed `cargo run -p studyos-cli`, `cargo test`, or exercised the tutor loop.
3. The companion review was also authored by the same code-generation pipeline and grades its own work.

The goal here is to (a) separate what is verified from what is asserted, and (b) give the next Codex pass a concrete, gate-driven plan that reaches a genuinely usable app, rather than another round of plausible-looking self-congratulation.

---

## 1. How to read the companion review

The companion review is useful as a product narrative and as a statement of intent. It is **not** reliable evidence that the described behaviours actually occur. Every "the runtime now does X" sentence should be mentally rewritten as "the code contains paths that would do X if it ran and if the surrounding system behaved as the author assumed." Until the first phase of this roadmap is complete, treat the build as a well-structured skeleton with untested joints.

Concrete consequences:

- "Real agent runtime" — untested. The `AppServerClient` has never been observed completing a live turn on this machine.
- "Structured tutor payloads render correctly" — untested. No test exercises the full `stdio -> app-server -> turn -> item/completed -> JSON parse` pipeline.
- "Session recap persistence" — partially tested. The DB round-trip is tested; the recap generation path that calls `run_structured_turn_and_wait` blocks the UI and has no integration coverage.
- "Course-aware startup", "timetable injection", "materials search" — the data layer is tested; the prompt injection that uses it is only asserted by a single string-contains test.

Nothing in the review is obviously dishonest. It is simply confident about behaviours that have never been observed.

---

## 2. Verified findings

All items below are reproducible against the current repo on the user's machine.

### 2.1 The workspace does not build on the pinned toolchain

```
$ cargo check --workspace
error: rustc 1.87.0 is not supported by the following packages:
  darling@0.23.0 requires rustc 1.88.0
  darling_core@0.23.0 requires rustc 1.88.0
  instability@0.3.12 requires rustc 1.88
  time@0.3.47 requires rustc 1.88.0
  time-core@0.1.8 requires rustc 1.88.0
```

`rust-toolchain.toml` says `channel = "stable"` (1.87 on this machine). `Cargo.toml` uses `edition = "2024"` and transitively resolves crate versions that require 1.88+. The README's single-command Quickstart is false as of this commit. The companion review does not mention this at all. This is a **P0 blocker**.

### 2.2 There is no integration test that spawns `codex app-server`

Every test in the repo works on local structs, SQLite, or fabricated `TutorTurnPayload` JSON fed directly into `App::apply_structured_tutor_payload`. The commit that the review describes as validating "that the structured tutor contract was not just aspirational" (`ba75bf6`) does nothing of the sort — it tests the persistence side of a pre-built payload. The actual contract between `studyos-cli` and `codex app-server` has never been exercised.

### 2.3 Schema drift between client and server

I extracted `codex-cli 0.121.0`'s app-server protocol schema via `codex app-server generate-json-schema --out`. Results:

- Notification method names sent by `map_notification` all match the schema (`thread/started`, `thread/status/changed`, `turn/started`, `turn/completed`, `item/started`, `item/completed`, `item/agentMessage/delta`, `mcpServer/startupStatus/updated`, `error`). OK.
- `TurnStartParams` supports `outputSchema`, `input: [{type:"text", text, text_elements}]`, `threadId`, `cwd`. OK.
- `ThreadStartParams` does **not** define `experimentalRawEvents` or `persistExtendedHistory`. The client sends both. They will be silently ignored by most serde configurations, but this is a code smell: it suggests the request shape was invented rather than read from the schema.
- `ThreadResumeParams` does **not** define `persistExtendedHistory`. Same story.
- `InitializeParams.capabilities` is sent as `null`. The schema allows it, but `experimentalApi` and `optOutNotificationMethods` are never set. If any part of the tutor loop needs experimental methods in a future `codex` release, this will quietly regress.
- No sandbox policy is set on `thread/start`. The server default may be `workspace-write`, which can surface approval prompts. The client has **no approval-request handler** wired up (no method handler for `applyPatchApproval`, `commandExecutionRequestApproval`, `execCommandApproval`, `permissionsRequestApproval`, `fileChangeRequestApproval`, or `mcpServerElicitationRequest`). If the model tries any sandboxed operation, the request loop will likely hang until the 60 s timeout.

### 2.4 `finish_session` blocks the TUI event loop

`App::finish_session` calls `generate_session_recap` which calls `AppServerClient::run_structured_turn_and_wait` with a 60 s timeout inside the synchronous handler for the `q` keystroke. The TUI will freeze for up to a minute on exit while the model generates the recap. There is no progress indicator, no cancel path, and no streaming — if the model is slow, the user sees a hung terminal. Not flagged in the review.

### 2.5 Structured-payload parsing is fragile

`apply_structured_tutor_payload` does `serde_json::from_str::<TutorTurnPayload>(raw)` on the agent-message text. If the model emits any wrapper text, markdown fences, or drifts from the schema (even with `outputSchema` set), parsing fails and the user gets a `WarningBox` with the raw response glued into the `body`. There is:

- no retry,
- no soft-schema fallback (e.g. accept a partial payload without the question and render just the teaching blocks),
- no telemetry to distinguish "model drift" from "transport error" from "our schema is wrong."

### 2.6 Concurrency and lifecycle gaps in `AppServerClient`

- `next_id` increments monotonically; `pending` is never drained on child death. A crashed app-server leaves `pending` senders alive until timeout.
- `child` is wrapped in an `Arc<Mutex<_>>` but `Drop::drop` takes the lock and kills the child without signalling pending requests first — any in-flight `recv_timeout` still has to wait out its 60 s.
- `stderr` is logged via `set_activity` with activity name `"App-server"` overwriting the same slot on every line, so a burst of stderr drops all but the last line.

### 2.7 Persist/restore loop for widget drafts uses TOML but widgets may not TOML-serialise cleanly

`App::persist_resume_state` does `toml::to_string(self.active_widget())`. Widgets contain nested structures (matrix cells as `Vec<Vec<String>>`). TOML is strict about array-of-array typing; empty or heterogeneous cells may fail to round-trip. No test covers this. The code swallows errors in `apply_resume_state` via a silent `contains_key` guard. This is likely to silently lose drafts rather than crash, but the review claims "restore unsent drafts" as a first-class feature.

### 2.8 The "interactive teaching" claim is not yet demonstrable

There is no recorded session transcript, no screenshot, no demo log, no CI artefact showing the TUI rendering a tutor-generated question and receiving a graded answer. The review's section "How well it meets the core goals" gives grades against goals ("strongly met", "partially met") without any evidence of the app having been used. Every positive grade in that section should be read as "would-be-met if the pipeline works as designed."

### 2.9 The companion review references a `repomix` appendix as-if-generated

The review contains a "Codepack appendix" and says Repomix reported a token count. This appears to be an embedded narrative device rather than tool output — there is no commit of the pack artefact and no reason the review author would have needed an external pack to read files in the same repo. This does not affect the code, but it does affect the review's trustworthiness: it invents a provenance story. Treat similar framings with suspicion.

---

## 3. Claim-by-claim verdict on the companion review

| Review claim | Verdict | Note |
|---|---|---|
| "functioning terminal-native tutoring runtime" | **Unverified** | TUI code exists; never observed running. |
| "real Codex app-server loop" | **Unverified** | No integration test; schema drift present. |
| "structured answer widgets" as core | **Plausible** | Widget code and types exist; tests cover handlers. |
| "local study memory" | **Verified (DB layer)** | SQLite schema + tests pass the DB round-trip, assuming build works. |
| "session recap persistence and exit review" | **Partial** | Persistence tested; generation path blocks UI. |
| "course-aware startup" | **Verified (static branch)** | Bootstrap transcript branches on course string; real tutor path untested. |
| "local deadline/materials/timetable tooling" | **Verified (CLI)** | Data-layer commands compile (once toolchain fixed) and have tests. |
| "materially supports the target vision" | **Aspirational** | Vision claim; cannot be graded before first live run. |
| "strongly met: one terminal" | **Aspirational** | See above. |
| "app-server harness is thin" | **False** | `app.rs` is 2,314 lines. The client carries substantial pedagogy + schema building. |
| "bookmarked next step = window-aware planner" | **Premature** | Cannot responsibly prioritise planner work until the loop is proven end-to-end. |

The review's meta-judgment ("meaningfully beyond proof of concept") is not supportable. By the normal bar — has anyone used this thing successfully once? — this is still a proof of concept.

---

## 4. Roadmap

The roadmap is phased and gated. **No phase may be declared complete until its quality gates are independently verifiable.** Each gate is a `cargo` command, a file artefact, or a recorded transcript — not an assertion in prose.

### Phase 0 — Make it buildable and runnable (target: 1 day)

**Goal:** `cargo check`, `cargo test`, and `cargo run -p studyos-cli` all succeed on the user's machine without hand-intervention beyond standard tool install.

Work items:

1. Pin the toolchain explicitly.
   - Change `rust-toolchain.toml` to `channel = "1.88"` (or whichever version satisfies current deps).
   - Update `rust-version` in `Cargo.toml` workspace package to match.
   - Alternatively, `cargo update --precise` the offending deps (`darling`, `instability`, `time`) back to 1.87-compatible versions. Prefer bumping the toolchain.
2. Add a `just` file or `scripts/check.sh` so that the quality gate is a single command.
3. Add a `.github/workflows/ci.yml` (or confirm the existing one) that runs `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, on the pinned toolchain. Fail the build on any warning.
4. Document the exact `codex` CLI version the integration targets (`codex-cli 0.121.0` or later) in `README.md`.
5. Ship a `make doctor` or `cargo run -p studyos-cli -- doctor` example in README that shows expected output on a fresh machine.

Quality gates (all must pass):

- [ ] `cargo check --workspace` exits 0.
- [ ] `cargo test --workspace` exits 0; total count ≥ existing count.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] `cargo run -p studyos-cli -- doctor` runs without panicking on a clean machine with `STUDYOS_DATA_DIR=$(mktemp -d)`.
- [ ] CI green on main.

Exit criteria: a human can clone, build, and see the `doctor` output without editing files.

---

### Phase 1 — Prove the runtime end-to-end (target: 2-3 days)

**Goal:** The app has been observed to drive a live `codex app-server` through at least one full tutor turn, and there is an automated test that asserts this continues to work.

Work items:

1. **Record a canned session.** Build a small helper binary `crates/studyos-cli/examples/record_runtime_session.rs` that:
   - Spawns `codex app-server --listen stdio://`.
   - Runs `initialize`, `thread/start`, `turn/start` with the real opening prompt and `tutor_output_schema()`.
   - Captures every notification line into a JSON Lines file under `tests/fixtures/runtime/opening-turn.jsonl`.
   - Also captures stderr to `tests/fixtures/runtime/opening-turn.stderr.log`.
2. **Add an integration test** `tests/runtime_replay.rs` that:
   - Reads the fixture.
   - Feeds it through a trait-level seam (introduce `trait AppServerTransport` now) so `AppServerClient` is not tied to `Command::new("codex")`.
   - Asserts the client reaches `ItemCompleted` with a parseable `TutorTurnPayload`.
3. **Add a live smoke test** gated on `STUDYOS_CODEX_AVAILABLE=1` that actually runs against the installed `codex` binary and parses the resulting payload. This runs locally, not in CI.
4. **Fix schema drift.** Remove `experimentalRawEvents` and `persistExtendedHistory` from `thread/start` and `thread/resume` params unless a specific need is documented. Add `capabilities: { experimentalApi: true }` to `initialize` if any experimental method is in use.
5. **Wire an approval-handler stub.** Add a minimal JSON-RPC request handler in `AppServerClient` that auto-rejects every incoming approval request with a clear "StudyOS does not permit server-driven tool calls" error. This prevents the runtime from hanging on unexpected approvals.
6. **Tighten output-schema parsing.**
   - Strip common wrapper artefacts (triple-backtick fences, leading "```json", trailing prose) before `serde_json::from_str`.
   - On parse failure, surface a `WarningBox` with the error **and** re-issue a single automatic retry with a system-style reminder to the tutor to return raw JSON. Bound retries to 1.
   - Emit a structured log line (`app.runtime.payload_parse_failure = 1`) per failure for diagnostics.

Quality gates:

- [ ] `tests/fixtures/runtime/opening-turn.jsonl` exists and is committed.
- [ ] `cargo test --workspace runtime_replay` passes deterministically.
- [ ] `STUDYOS_CODEX_AVAILABLE=1 cargo test -p studyos-cli --test runtime_live -- --ignored` passes on the user's machine once.
- [ ] Manual run `cargo run -p studyos-cli` reaches the first tutor-generated question without a `WarningBox` in the transcript.
- [ ] Recording of one successful session attached to `docs/review/` as `2026-04-19-first-live-run.log` (screen capture or asciinema).

Exit criteria: the user has personally completed one round-trip: open → tutor question → answer → evaluation → next question.

---

### Phase 2 — Harden the session lifecycle (target: 2-3 days)

**Goal:** Exit, resume, and error-recovery work without surprises. The UI never blocks.

Work items:

1. **Make recap generation asynchronous.**
   - Introduce a runtime event `SessionRecapReady { recap, session_id }`.
   - Start the recap turn on `q` keypress, show a non-blocking "preparing recap…" overlay, allow a second `q` to force-quit with the fallback recap.
   - Never call `run_structured_turn_and_wait` from a keystroke handler again. Delete the helper or restrict it to test harnesses.
2. **Harden resume state.**
   - Replace TOML serialisation of widgets with explicit JSON (widgets are richly typed; TOML is the wrong tool).
   - Add a round-trip test per widget variant (matrix, working+final, step list, retrieval).
   - Version the resume-state schema with a `schema_version: u32` field and reject higher versions with a clear error.
3. **Fix `AppServerClient` lifecycle.**
   - On `Drop`, drain `pending` senders with a "transport closed" error before killing the child.
   - Buffer recent stderr lines (ring of ~50) rather than overwriting the single `App-server` activity slot.
   - Expose the buffer via a new panel tab `PanelTab::RuntimeLog`.
4. **Handle disconnect gracefully.**
   - On `RuntimeEvent::Disconnected`, the UI should show a red-status banner, save resume state, and stop issuing new turns.
   - Offer an explicit "reconnect runtime" key (e.g. `Ctrl+R`) that re-spawns the app-server and resumes the current thread.
5. **Bound memory.**
   - `self.structured_buffers`, `self.pending_structured_turns`, and `self.live_message_indices` are all unbounded `HashMap`s keyed by turn/item id. Add eviction on `TurnCompleted` and a capacity cap of e.g. 64.

Quality gates:

- [ ] New unit tests: `resume_state_round_trips_per_widget_variant`, `widget_draft_survives_restart`, `recap_ready_event_does_not_block_key_handler`.
- [ ] Manual test: press `q` during a live session with a slow model — the TUI must remain responsive.
- [ ] Manual test: kill `codex app-server` mid-session — the TUI must report disconnect within 2s and not panic.
- [ ] `cargo test` count increased by at least 10.

Exit criteria: user can open, answer at least three questions across two sessions, quit, resume, and see no lost state.

---

### Phase 3 — Evidence model sanity pass (target: 2 days)

**Goal:** The "student model" in SQLite is internally consistent, inspectable, and tested against adversarial inputs.

Work items:

1. **Schema audit.**
   - Read `store.rs` carefully; add `CREATE TABLE IF NOT EXISTS` scripts to a versioned migrations directory (`crates/studyos-core/migrations/NNNN_description.sql`). Stop mutating schema through `execute_batch` inside `initialize_schema` + ad-hoc `migrate_*` methods.
   - Add a `schema_version` row in a `meta` table. Refuse to open a DB whose schema is newer than the client.
2. **Mastery/retrieval arithmetic invariants.**
   - `update_concept_state` applies deltas via `clamp`. Add property tests (via `proptest`): for random sequences of attempts, `mastery_estimate`, `retrieval_strength`, `stability_days`, `ease_factor` must always be in range.
   - Add a test that a long run of "correct" attempts asymptotically raises mastery and a long run of "incorrect" attempts lowers it.
3. **Misconception uniqueness.**
   - Confirm `upsert_misconception` genuinely deduplicates on `(concept_id, error_type, description)`. Add a test that a repeated identical misconception does not create a second row.
4. **Attempt auditability.**
   - Add a `studyos-cli -- attempts list --session <id>` command that prints the attempt audit trail. Useful for debugging and for the user's own confidence.

Quality gates:

- [ ] Migration files present and checked in.
- [ ] `proptest`-based property tests pass.
- [ ] `cargo run -p studyos-cli -- attempts list --session <id>` produces stable output.
- [ ] DB opened from a past schema version still works (forward-compat test).

Exit criteria: the student model can be audited and trusted; no "magic numbers" in mastery updates go unexplained.

---

### Phase 4 — Materials ingestion foundation (target: 5-7 days)

**Goal:** A student can drop a folder of course PDFs, slides, and notes into `.studyos/materials/raw/`, run one command, and have the tutor condition on real content.

This is the biggest strategic gap flagged (correctly) by the companion review. It deserves a full slice of work.

Work items:

1. **Define the ingestion contract.**
   - `.studyos/materials/raw/` — user drops files here.
   - `.studyos/materials/index/` — generated artefacts (never hand-edit).
   - `.studyos/materials/manifest.json` — today's manifest; regenerated by ingestion.
   - `.studyos/materials/concepts.json` — extracted topic/concept tags per file.
2. **Implement `studyos-cli -- materials ingest`.**
   - Walk `raw/`.
   - For `.md`, `.txt`, `.tex`: read text directly.
   - For `.pdf`: use `pdf-extract` crate for first pass; log and skip files it can't handle.
   - For `.docx`, `.pptx`, `.odt`: skip with a warning (out of scope for this phase).
   - Emit one `MaterialEntry` per file with a best-effort `snippet` (first ~500 chars) and `topic_tags` derived from a simple keyword match against the current `CourseDefinition.concepts`.
3. **Do not send raw text to the tutor.** Extend the prompt builder to include distilled summaries (title + tags + snippet) only. Never verbatim replay slide text.
4. **Guard privacy.**
   - Ensure `.studyos/materials/raw/` is gitignored at repo root (add if missing).
   - Add a `doctor` check that warns if raw materials are inside a git-tracked directory.
5. **Make ingestion incremental.**
   - Track `mtime` and file hash per raw file. Skip unchanged files.

Explicitly out of scope for Phase 4:

- OCR on image-only PDFs.
- Embedding-based retrieval.
- Automatic problem-sheet vs slide-deck classification.

Quality gates:

- [ ] Unit tests: walking a mixed `raw/` dir produces expected `manifest.json`.
- [ ] End-to-end test: a sample PDF (committed as a fixture) yields a non-empty snippet.
- [ ] `doctor` prints "materials ingested: N files, last run: <timestamp>".
- [ ] Opening prompt integration test asserts that a matched material title appears in the prompt when the course matches.

Exit criteria: the user can replace the example manifest with a real folder of their own course PDFs and run a tutor session whose prompt demonstrably references their files.

---

### Phase 5 — Window-aware session planning (target: 3-4 days)

This is the bookmark from the companion review. It is appropriate to tackle it **only after Phases 0-4**, because until then, the "planner" would be conditioning on a runtime that does not actually work.

Work items:

1. **Define a `StudyWindow` type** in `studyos-core`.
   - `start: OffsetDateTime`, `duration_minutes: u16`, `source: WindowSource { TimetableGap, BeforeDeadline, EveningBlock }`.
2. **Derive candidate windows from timetable + deadlines.**
   - Use `LocalContext::timetable` to compute gaps today and tomorrow.
   - Combine with deadline pressure to score each window.
3. **Pass the current window into `SessionPlanSummary`.**
   - Add `window: Option<StudyWindow>`.
   - Adjust `recommended_duration_minutes` to fit the window.
4. **Teach the tutor about the window.**
   - Extend `build_opening_prompt` to describe the window explicitly ("you have 18 minutes before the next lecture; design an opportunistic repair session").
   - Add tests that verify the prompt differs measurably between a 15-minute window and a 90-minute window.
5. **Don't over-engineer.** No scheduler, no calendar sync, no notifications. This phase is about conditioning the tutor, not replacing the user's calendar.

Quality gates:

- [ ] Snapshot test: plan summary for short-window vs long-window fixtures.
- [ ] Prompt test: 15-min and 90-min windows produce distinguishable opening prompts.
- [ ] Manual test: running at 14:55 before a 15:00 lecture produces a visibly different opener than running at 21:00.

Exit criteria: the companion review's bookmarked goal is met, with tests proving it.

---

### Phase 6 — Daily-use polish (ongoing)

Everything the review softly acknowledges as weak:

- Packaging: ship a `cargo install --path crates/studyos-cli` target; consider `brew` later.
- Onboarding: a 90-second `studyos tour` that walks a new user through `init`, `doctor`, `deadlines add`, `timetable add`, first session.
- Observability: a `--log-json` flag that writes one JSON line per runtime event to `.studyos/logs/`.
- Keybinding cheatsheet always-on toggle.
- Accessibility pass on colours / reduced motion.

No hard gate here — this is the drumbeat after the foundations are sound.

---

## 5. Test matrix

Unit tests (target 100% of these added before Phase 2 exit):

| Module | Test | Purpose |
|---|---|---|
| `config.rs` | `load_or_default_returns_defaults_for_missing_file` | already exists |
| `config.rs` | `save_round_trips_config` | already exists |
| `store.rs` | property test: mastery stays in `[0,1]` | Phase 3 |
| `store.rs` | property test: ease stays in `[1.3, 3.0]` | Phase 3 |
| `store.rs` | repeated misconception does not duplicate | Phase 3 |
| `store.rs` | `schema_version` refuses to open newer DB | Phase 3 |
| `widgets.rs` | JSON round-trip per variant | Phase 2 |
| `local_data.rs` | ingest walks `raw/` correctly | Phase 4 |
| `local_data.rs` | ingest is incremental on unchanged file | Phase 4 |
| `session.rs` | window-aware plan differs by duration | Phase 5 |
| `tutor.rs` | `TutorTurnPayload` parses from representative fixture | Phase 1 |

Integration tests:

| Name | Purpose | Phase |
|---|---|---|
| `runtime_replay` | Drive `AppServerTransport` from a JSONL fixture; assert payload parse | 1 |
| `runtime_live` (`#[ignore]`) | Spawn real `codex app-server`, complete one turn | 1 |
| `ingest_end_to_end` | `materials ingest` on a fixture `raw/` dir produces expected manifest | 4 |
| `opening_prompt_uses_window` | Plan + opening prompt reflect window size | 5 |

Manual / recorded tests (document outcomes under `docs/review/run-logs/`):

| Scenario | Pass condition | Phase |
|---|---|---|
| Cold start, first tutor question appears | ≤ 15 s, no warning box | 1 |
| Mid-session disconnect | TUI recovers within 2 s | 2 |
| Exit during recap | UI remains responsive; recap appears when ready | 2 |
| Drop 3 PDFs, run session | Opening prompt references at least one | 4 |
| Start before 15:00 lecture vs 21:00 evening | Openers differ | 5 |

---

## 6. Directives for the next Codex pass

Because Codex will likely be the hands on the keyboard for most of this work, these directives are written to it.

**Do:**

1. Start with Phase 0. Do not begin any pedagogical work until `cargo check && cargo test && cargo clippy` pass green.
2. Treat the app-server protocol schema as ground truth. Regenerate it with `codex app-server generate-json-schema --out target/codex-schema` and read the relevant `.json` files before touching `runtime.rs`.
3. Introduce `trait AppServerTransport` early. All production code takes a `&dyn AppServerTransport`; tests inject a fixture-backed impl. This is the single most important refactor for unlocking reliable integration tests.
4. For every claim of new behaviour, add a test that fails before the change and passes after. If a claim cannot be tested, say so explicitly in the commit message.
5. Prefer deletion over abstraction. If code is not exercised by any test and not on a phase plan, remove it.
6. Keep commits small and topic-focused. One quality gate per commit where possible. Never bundle Phase 2 work into a Phase 0 commit.

**Do not:**

1. Do not add new pedagogical surface area (skills layer, slide mode, new widgets) before Phase 4 exits.
2. Do not write a fresh self-congratulatory review. The next review document must cite concrete evidence: test counts, commit hashes, recorded transcripts. If there is no evidence, there is nothing to review.
3. Do not widen the prompt's injected context without a test that compares before/after behaviour. "More context" is not automatically better.
4. Do not use `unwrap` or `expect` in paths reachable from `main`. Every fallible call returns `Result`.
5. Do not silently swallow errors in the resume/restore path. If a draft cannot be restored, tell the user.

---

## 7. Explicit non-goals (for now)

These are **not** part of the roadmap and should not be started until all phases above are done:

- Skills-layer orchestration inside app-server.
- Slide mode / richer teaching surfaces.
- Calendar integrations (Google / Outlook).
- Embedding-based material retrieval.
- OCR / image-PDF handling.
- OS-level DND / focus enforcement.
- Dedicated MCP server suite.
- Multi-user / multi-device sync.

Every item on this list is something the companion review gestures at. Each is premature.

---

## 8. Exit criteria for "genuinely usable app"

The project can be called genuinely usable when **all** of these are true:

1. A fresh-clone developer can run `cargo test && cargo run -p studyos-cli -- doctor` and see green + healthy output with zero manual intervention beyond installing Rust and `codex`.
2. A student can open the TUI, receive a tutor-generated structured question that references their own materials, answer it, see a graded response, and see that evidence in SQLite.
3. A student can quit mid-session and return the next day with continuity (resume state, recap from last session, review queue).
4. The runtime has survived at least one observed disconnect + reconnect without data loss.
5. The `doctor` command's output can serve as a bug report; every field it prints is accurate.
6. `cargo clippy --workspace --all-targets -- -D warnings` is clean.
7. There is at least one recorded live-session transcript in `docs/review/run-logs/` demonstrating a full loop.

Until **all seven** hold, the project remains in proof-of-concept territory, irrespective of how elegant the code reads.

---

## 9. Summary for the user

- The companion review is a well-structured narrative of intent. It is not evidence.
- The most important thing the next pass can do is make the code run and then prove it runs.
- After that, harden the lifecycle, tighten the evidence model, ship a real ingestion path, and only then work on the window-aware planner the original review bookmarked.
- Resist the temptation to accept another confident self-review. Require evidence. The test matrix in §5 is that evidence.
