# StudyOS Alpha Readiness

Date: 2026-04-19
Scope: internal alpha / dogfooding pass after commit `00aaf03`

## Purpose

This note records the alpha-readiness pass that followed the question "is the repo ready?".

The goal was not to produce a polished launch note. The goal was to:

1. resolve the remaining dirty-repo state
2. run several live end-to-end study sessions
3. treat any friction discovered during those sessions as product bugs, not user error
4. tighten the harness until the repo was in a better state for real dogfooding

## What was exercised

The pass used a new live runner:

- [crates/studyos-cli/examples/alpha_readiness.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/examples/alpha_readiness.rs)

That runner:

- seeds a realistic local `.studyos` workspace under `target/alpha-readiness/`
- loads courses, deadlines, timetable slots, and raw study materials
- ingests materials into the distilled local manifest
- runs five live `codex app-server` sessions in sequence
- submits structured answers
- requests recap/close turns
- records a summary of what each session actually produced

The final live run produced:

1. `matrix-baseline` -> `Study`
2. `matrix-deadline-pressure` -> `Study`
3. `probability-switch` -> `Study`
4. `repair-followup` -> `Review`
5. `probability-review-followup` -> `Review`

All five sessions completed without parse warnings, and each persisted one graded attempt.

## Bugs found during the alpha pass

### 1. Placeholder widget could be submitted before the first live tutor payload

Symptom:

- the app exposed the bootstrap placeholder question as an active widget even while the opening runtime turn was still in flight
- a student could therefore submit an answer before the live tutor question actually arrived

Fix:

- widget/question interactivity is now gated until the first live runtime payload is applied
- startup status now explicitly reports that the app is waiting for the live tutor question

Code:

- [crates/studyos-cli/src/app.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/app.rs)

Regression coverage:

- `runtime_bootstrap_hides_placeholder_widget_until_live_payload_arrives`

### 2. Subject context leaked across course switches

Symptom:

- switching from matrix algebra to probability could still inherit the wrong tutor thread context

Fix:

- per-course runtime thread persistence was added
- the single global resume thread is no longer the only source of truth

Code:

- [crates/studyos-core/src/store.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/store.rs)
- [crates/studyos-core/migrations/0004_course_runtime_threads.sql](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/migrations/0004_course_runtime_threads.sql)
- [crates/studyos-cli/src/app.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/app.rs)

Regression coverage:

- `course_runtime_threads_round_trip_per_course`

### 3. Last-session recap contaminated the next course's opening plan

Symptom:

- even after thread isolation, probability sessions could still open on matrix repair tasks
- the root cause was that startup planning consumed the latest recap globally rather than the latest recap for the active course

Fix:

- sessions are now course-scoped in storage
- latest recap lookup is filtered by course

Code:

- [crates/studyos-core/src/store.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/store.rs)
- [crates/studyos-core/migrations/0005_session_course_scope.sql](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/migrations/0005_session_course_scope.sql)
- [crates/studyos-cli/src/main.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/main.rs)
- [crates/studyos-cli/examples/alpha_readiness.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/examples/alpha_readiness.rs)

### 4. Startup mode routing was too flat

Symptom:

- live follow-up sessions remained in `Study` even when they were clearly deadline-driven or repair-driven

Fix:

- deadline run-up windows now count as drill pressure
- unfinished objectives in the last same-course recap now count as review pressure

Code:

- [crates/studyos-core/src/session.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/session.rs)

Regression coverage:

- `bootstrap_routes_to_drill_for_deadline_run_up_window`
- `bootstrap_uses_unfinished_objectives_from_last_session`

### 5. Course-scoped study memory and deadline pressure were still leaking

Symptom:

- after the first round of fixes, probability sessions still inherited matrix repair pressure
- the root cause was broader than runtime thread state: due reviews, repair signals, deadline counts, and study-window selection were still read globally in several startup paths

Fix:

- startup due-review and repair-signal reads are now filtered by active course
- course-scoped due-review counts now feed startup metrics
- deadline counts and study-window selection are now course-aware
- app refresh and recap prompts now pull course-scoped memory instead of global memory

Code:

- [crates/studyos-core/src/store.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/store.rs)
- [crates/studyos-core/src/local_data.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-core/src/local_data.rs)
- [crates/studyos-cli/src/main.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/main.rs)
- [crates/studyos-cli/src/app.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/src/app.rs)
- [crates/studyos-cli/examples/alpha_readiness.rs](/Users/georgepullen/Documents/StudyOS/crates/studyos-cli/examples/alpha_readiness.rs)

Regression coverage:

- `course_scoped_review_and_repair_queries_do_not_leak_between_courses`

## Result of the final live alpha run

The final pass was materially better than the first one.

What improved:

- probability sessions stayed on probability content
- repair follow-ups moved into `Review` instead of staying generic
- course switches no longer dragged matrix repair state into probability startup
- structured answer submission remained stable across all five sessions
- recap generation worked for all five sessions

What still remains limited:

- the first two matrix sessions still opened in `Study` rather than `Drill`; that is currently defensible because one near deadline only counts as `Upcoming`, but it may still not be the best student-facing behavior
- misconception promotion remains conservative, so `recent_repair_signals` may under-report after a small number of sessions
- `due_reviews` remained `0` in this pass because the scheduler did not yet advance those items into immediate review windows

## Current readiness judgment

After this alpha pass, the repo is in a better state than when the pass started.

It is ready for:

- internal dogfooding
- repeated live study sessions
- targeted UX tuning from real use

It is still not yet "finished" in the sense of broad release readiness.

The remaining work is now more product-facing than harness-facing:

- tune scheduler urgency so failed same-day retrieval produces clearer immediate review pressure
- continue improving question quality and adaptation depth from live study logs
- observe whether structured widget ergonomics feel fast enough over longer sessions

## Validation commands

The code and tests backing this pass were validated with:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
STUDYOS_CODEX_AVAILABLE=1 cargo test -p studyos-cli --test runtime_live -- --ignored --nocapture
STUDYOS_CODEX_AVAILABLE=1 cargo run -p studyos-cli --example alpha_readiness
```
