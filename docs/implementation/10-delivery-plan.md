# Delivery Plan

## Goal

Sequence implementation so the riskiest product assumptions are tested early.

## Phase 1: Foundations

Deliver:

- Rust workspace skeleton
- basic runtime entrypoint
- app config loading
- SQLite bootstrap
- ratatui shell skeleton

Exit criteria:

- app opens as a full-screen TUI
- app can load config and open local database

## Phase 2: Internal Contracts

Deliver:

- content block schemas
- question and grading payload schemas
- app event model
- resume-state model

Exit criteria:

- core domain types are stable enough to unblock UI and runtime work

## Phase 3: App-Server Runtime

Deliver:

- stdio app-server process management
- initialization handshake
- turn start and streaming event handling
- thread resume path

Exit criteria:

- a live streamed tutor message can appear in the transcript reliably

## Phase 4: Rich Transcript Rendering

Deliver:

- paragraph and heading blocks
- math block path
- question and hint card rendering
- renderer fallbacks

Exit criteria:

- a session plan and a maths question can be shown cleanly in the terminal

## Phase 5: Structured Answer Widgets

Deliver:

- matrix entry grid
- short retrieval response
- working plus final answer form
- step list widget

Exit criteria:

- a student can complete the main answer flows without using plain chat input

## Phase 6: Persistence And Resume

Deliver:

- attempts table writes
- sessions and misconception persistence
- resume snapshots
- scratchpad autosave

Exit criteria:

- restarting the app preserves active study continuity

## Phase 7: Session Planning And Pedagogy

Deliver:

- opening context assembly
- due review selection
- strict attempt-first local rules
- recap generation pipeline

Exit criteria:

- session behavior feels intentionally educational rather than generic

## Phase 8: Local Deadlines And Materials

Deliver:

- deadlines and timetable file ingestion
- urgency scoring
- materials manifest search

Exit criteria:

- session plan changes based on local deadline pressure and available materials

## Phase 9: Reliability Pass

Deliver:

- startup health checks
- improved error states
- test coverage for critical flows
- profiling and render improvements

Exit criteria:

- V1 is stable for repeated personal daily use

## First Release Checklist

- launch and resume work reliably
- app-server integration is stable over long sessions
- structured answering is clearly better than plain terminal chat
- memory updates persist correctly
- local deadline context affects planning
- fallback mode is usable
- public repo docs and setup remain clean
