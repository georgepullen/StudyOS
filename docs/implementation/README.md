# Implementation Plan

This directory contains the working V1 implementation spec for StudyOS.

These docs define a usable first release for real student study, not a thin proof of concept. V1 is intentionally scoped, but it must still support a complete study loop with structured mathematical interaction, persistent memory, and reliable session continuity.

## V1 Product Boundary

V1 must let a student:

1. launch from the terminal without setup friction
2. resume prior study context
3. receive a deadline-aware session plan
4. answer structured mathematical questions quickly
5. get graded feedback without defaulting to spoon-feeding
6. persist progress, misconceptions, and review schedule locally
7. exit and resume without losing context

V1 is not required to include:

- dedicated custom MCP servers
- live Google Calendar or ICS integrations
- OCR-heavy materials ingestion
- multi-user support
- polished slide mode

## Document Map

- [01-v1-scope.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/01-v1-scope.md): feature inventory, MVP boundary, in/out decisions
- [02-system-architecture.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/02-system-architecture.md): runtime architecture and crate responsibilities
- [03-tui-ui-spec.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/03-tui-ui-spec.md): terminal UI layout, navigation, and state model
- [04-rich-content-and-rendering.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/04-rich-content-and-rendering.md): transcript blocks, math rendering, and renderer fallbacks
- [05-structured-answering.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/05-structured-answering.md): V1 answer widgets and submission contracts
- [06-session-orchestration-and-pedagogy.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/06-session-orchestration-and-pedagogy.md): study flow, anti-crutch rules, and lesson behavior
- [07-storage-calendar-and-materials.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/07-storage-calendar-and-materials.md): SQLite schema, local deadlines, and materials indexing
- [08-app-server-integration.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/08-app-server-integration.md): Codex app-server transport, event handling, and thread/turn contracts
- [09-operability-and-testing.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/09-operability-and-testing.md): startup behavior, observability, testing, and acceptance criteria
- [10-delivery-plan.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/10-delivery-plan.md): implementation sequence and milestone plan
- [11-course-content-model.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/11-course-content-model.md): course metadata, concept graph, and topic tagging
- [12-launcher-and-configuration.md](/Users/georgepullen/Documents/StudyOS/docs/implementation/12-launcher-and-configuration.md): local launch commands, config files, and focus-mode behavior

## Cross-Cutting Rules

- V1 is Rust-first and terminal-first.
- Local-first storage is the default.
- The core answer path is structured input, not plain text chat.
- Rich rendering is required, but plaintext fallback must remain usable.
- Codex app-server is the agent runtime, not the UI renderer.
- The pedagogical default is attempt-first and anti-passivity.
- Anything deferred from V1 should still have a clear interface seam so it can be added later without a rewrite.
