# StudyOS

StudyOS is a terminal-native adaptive tutor client built on top of Codex app-server for a distraction-minimised study environment.

The first target subjects are:

- Matrix Algebra & Linear Models
- Probability & Statistics for Scientists

## Product Direction

StudyOS is not meant to be a prettier chat window. The goal is a rich, keyboard-first study client that combines:

- structured mathematical presentation
- structured mathematical answer input
- adaptive questioning
- spaced repetition
- misconception tracking
- deadline-aware session planning
- anti-crutch pedagogy

Codex app-server provides the conversation and agent runtime. The client provides the educational UX, local persistence, rendering, and focus-oriented workflow.

## V1 Priorities

The first MVP should prove one complete study loop:

1. launch from the terminal
2. start or resume a session
3. generate a session plan
4. present a structured maths question
5. accept structured input from the student
6. grade the response
7. persist the outcome locally
8. schedule the next review

### Core V1 decisions

- client language: `Rust`
- runtime model: Codex app-server over `stdio`
- primary terminal target: graphics-capable local terminal
- storage: local `SQLite`
- pedagogy default: attempt-first, anti-passivity
- structured input is a core feature, not a later enhancement

### Initial structured input widgets

- matrix entry grid
- working + final answer
- step list
- short retrieval response

## Planned Architecture

```text
Launcher / Focus Wrapper
    -> Rich Tutor Client UI
        -> Codex app-server session runtime
        -> Local renderer
        -> Local study memory store
        -> Deadline / timetable adapters
        -> Materials index
```

## MVP Scope

Included early:

- Rust TUI shell
- Codex app-server session runtime over `stdio`
- streamed transcript rendering
- structured answer widgets
- local memory database
- session planning
- recap and resume flow

Deferred until after the core loop works:

- dedicated custom MCP server suite
- live calendar integrations
- OCR-heavy materials ingestion
- slide-mode polish
- advanced analytics

## Status

This repository now has the first end-to-end tutor loop wired up:

1. launch the Rust TUI shell
2. boot Codex app-server locally
3. start or resume a tutor thread
4. request a schema-constrained opening study step
5. render the returned teaching blocks and structured question
6. submit a structured answer for grading and the next question
7. persist local resume state, session records, attempts, and misconceptions in SQLite

Detailed V1 implementation docs live in:

- [docs/implementation/README.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/README.md)

## Quickstart

Prerequisites:

- Rust stable
- local `codex` CLI installed and authenticated

Run:

```bash
cargo run -p studyos-cli
```

Useful keys:

- `q` safe exit
- `tab` cycle focus regions
- `1` to `6` switch side panels
- `[` and `]` jump between active questions
- `F5` submit the active structured answer
- `?` show help

Initialize local starter files:

```bash
cargo run -p studyos-cli -- init
```

Inspect local setup health:

```bash
cargo run -p studyos-cli -- doctor
```

Example local data files live in:

- [examples/studyos-config.toml](/Users/georgepullen/Documents/StudyOS/examples/studyos-config.toml)
- [examples/deadlines.json](/Users/georgepullen/Documents/StudyOS/examples/deadlines.json)
- [examples/timetable.json](/Users/georgepullen/Documents/StudyOS/examples/timetable.json)
- [examples/materials-manifest.json](/Users/georgepullen/Documents/StudyOS/examples/materials-manifest.json)
- [examples/linear-models.toml](/Users/georgepullen/Documents/StudyOS/examples/linear-models.toml)
- [examples/probability-stats.toml](/Users/georgepullen/Documents/StudyOS/examples/probability-stats.toml)

These should be copied into the local `.studyos/` data directory when you want the shell to load real local context:

```text
.studyos/
  config.toml
  deadlines.json
  timetable.json
  courses/
    linear-models.toml
    probability-stats.toml
  materials/
    manifest.json
```

## Repository Setup

Remote:

- `origin` -> `https://github.com/georgepullen/StudyOS.git`

Current branch:

- `main`

Workspace:

- `crates/studyos-cli`: executable entry point
- `crates/studyos-core`: shared domain types, persistence, and tutor payload models

Public repo safety defaults:

- local databases are gitignored
- `.env` files are gitignored
- generated runtime state is gitignored
- CI only runs repo-safe checks

## Principles

- terminal-first
- local-first
- keyboard-first
- retrieval before explanation
- evidence of understanding over answer-only correctness
- low distraction by default
