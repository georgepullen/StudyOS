# StudyOS Roadmap Execution Status

Date: 2026-04-19
Roadmap source: [2026-04-19-reviewer-response-and-roadmap.md](/Users/georgepullen/Documents/StudyOS/docs/review/2026-04-19-reviewer-response-and-roadmap.md)

This note records the state of the repo after a full roadmap implementation pass and subsequent reviewer-led hardening. It is intentionally narrower than a blanket claim of "closure."

## Outcome

The repo is no longer in the state described by the reviewer response. The original blockers have been addressed:

- the workspace builds on a pinned Rust `1.88.0` toolchain
- `just check` and `./scripts/check.sh` are real one-command quality gates
- `doctor` runs cleanly on a fresh temp data dir
- the client has a real `codex app-server` transport seam with replay and live integration coverage
- the live runtime path now reaches both the first tutor question and a graded round-trip submission
- exit/recap is asynchronous rather than blocking the TUI key handler
- resume-state drafts use versioned JSON rather than TOML
- disconnects save resume state, block accidental submission, and can be repaired with `Ctrl+R`
- the SQLite layer now uses versioned migrations plus invariant/property coverage
- the materials ingestion path exists, is incremental, and only forwards distilled snippets/tags
- startup mode selection is history-aware and window-aware
- a low-friction onboarding path exists via `studyos-cli tour`
- runtime JSONL logging exists via `cargo run -p studyos-cli -- --log-json`

## Phase-by-phase status

### Phase 0

Substantially completed.

Evidence:

- [justfile](/Users/georgepullen/Documents/StudyOS/justfile)
- [rust-toolchain.toml](/Users/georgepullen/Documents/StudyOS/rust-toolchain.toml)
- [.github/workflows/ci.yml](/Users/georgepullen/Documents/StudyOS/.github/workflows/ci.yml)
- [2026-04-19-validation.log](/Users/georgepullen/Documents/StudyOS/docs/review/run-logs/2026-04-19-validation.log)

### Phase 1

Substantially completed.

Evidence:

- [crates/studyos-cli/src/runtime.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/runtime.rs)
- [crates/studyos-cli/tests/runtime_replay.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/tests/runtime_replay.rs)
- [crates/studyos-cli/tests/runtime_live.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/tests/runtime_live.rs)
- [crates/studyos-cli/tests/fixtures/runtime/opening-turn.jsonl](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/tests/fixtures/runtime/opening-turn.jsonl)
- [2026-04-19-first-live-run.log](/Users/georgepullen/Documents/StudyOS/docs/review/run-logs/2026-04-19-first-live-run.log)

### Phase 2

Substantially completed.

Evidence:

- async recap state in [crates/studyos-cli/src/app.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/app.rs)
- runtime log panel in [crates/studyos-cli/src/tui.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/tui.rs)
- reconnect shortcut and resume persistence in [crates/studyos-cli/src/app.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/app.rs)
- regression tests:
  - `recap_ready_event_does_not_block_key_handler`
  - `disconnect_persists_resume_and_blocks_submission`
  - `ctrl_r_reconnects_runtime_after_disconnect`
  - `mismatched_runtime_turn_id_still_persists_structured_payload`

### Phase 3

Substantially completed.

Evidence:

- migrations in [crates/studyos-core/migrations](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/migrations)
- invariants and audit commands in [crates/studyos-core/src/store.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/store.rs) and [crates/studyos-cli/src/main.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/main.rs)
- property and migration tests recorded in [2026-04-19-validation.log](/Users/georgepullen/Documents/StudyOS/docs/review/run-logs/2026-04-19-validation.log)

### Phase 4

Substantially completed.

Evidence:

- ingestion pipeline in [crates/studyos-core/src/local_data.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/local_data.rs)
- CLI entry point in [crates/studyos-cli/src/main.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/main.rs)
- fixture PDF in [crates/studyos-core/tests/fixtures/materials/raw/linear-models.pdf](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/tests/fixtures/materials/raw/linear-models.pdf)
- end-to-end ingestion test recorded in [2026-04-19-validation.log](/Users/georgepullen/Documents/StudyOS/docs/review/run-logs/2026-04-19-validation.log)

### Phase 5

Substantially completed.

Evidence:

- study-window planning in [crates/studyos-core/src/session.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/session.rs)
- timetable/deadline window derivation in [crates/studyos-core/src/local_data.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/local_data.rs)
- prompt coverage in [crates/studyos-cli/tests/opening_prompt_uses_window.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/tests/opening_prompt_uses_window.rs)
- clean `doctor` output includes `study_window`

### Phase 6

Materially addressed, though it remains open-ended by design.

Implemented polish items:

- onboarding tour
- runtime JSONL logging
- improved README quickstart
- reviewer run logs

Not implemented because they were optional drumbeat items rather than hard gates:

- packaging beyond `cargo install --path crates/studyos-cli`
- accessibility/theme follow-up beyond the current reduced-motion/theme config surface

## Caveats

- The live runtime now proves the structured round-trip through the programmatic harness. That is stronger evidence than earlier claims, but it is still test-harness evidence rather than a polished demo video.
- A `script(1)`-style PTY capture was not used as the primary artefact because it distorted `codex app-server` invocation on this machine. The committed proof uses live integration tests plus the recorded runtime fixture instead.

## Verdict

Against the roadmap’s own bar, the project has crossed out of proof-of-concept territory:

1. fresh-clone validation commands exist and pass
2. the tutor can ask, accept, grade, and persist a structured answer through the live runtime
3. resume/recap continuity exists and is tested
4. disconnect/reconnect behaviour is exercised by regression tests
5. `doctor` reports the operational state needed for bug reports
6. clippy is clean under `-D warnings`
7. reviewer-facing live-run artefacts now exist under [docs/review/run-logs](/Users/georgepullen/Documents/StudyOS/docs/review/run-logs)

The remaining work is no longer "make the harness real." It is refinement, extension, and reviewer-driven tightening on top of a functioning harness. Earlier versions of this note overstated that as full closure; the more accurate description is that the core harness is now real, tested, and materially harder to regress.
