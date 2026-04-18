# System Architecture

## Architectural Principle

StudyOS is a custom terminal client layered on top of Codex app-server.

Codex app-server owns:

- conversation threads
- streamed turns
- tool invocation
- skills
- approvals
- thread persistence semantics

StudyOS owns:

- terminal rendering
- structured answer widgets
- local study memory
- local calendar and materials state
- focus-oriented navigation
- pedagogical client policy enforcement

## V1 Runtime Diagram

```text
studyos-cli
    -> app runtime
        -> tui shell
        -> app-server client
        -> renderer
        -> input widget system
        -> session orchestrator
        -> local sqlite store
        -> local config/calendar/materials adapters
```

## Crate Responsibilities

### `studyos-cli`

Owns:

- process startup
- config loading
- opening the TUI
- top-level command parsing
- runtime boot and shutdown

### `studyos-core`

Owns:

- domain types
- state models
- content block schemas
- session planner inputs and outputs
- persistence interfaces
- rendering contracts
- educational policy helpers

### Expected future crates

These do not need to exist immediately, but V1 code should make room for them.

- `studyos-app-server`: protocol client and event stream handling
- `studyos-ui`: ratatui widgets, layout, and interaction state
- `studyos-store`: SQLite access and migrations
- `studyos-render`: block rendering and terminal graphics helpers

## Main Runtime Subsystems

### App runtime

Coordinates the entire application.

Responsibilities:

- initialize dependencies in the right order
- own the main event loop
- route user input to active view or widget
- reconcile streamed app-server events into UI state
- persist critical state before exit

### Session orchestrator

Owns study-session behavior, not low-level rendering.

Responsibilities:

- assemble session opening context
- request or validate session plans
- decide which study phase is active
- enforce local pedagogical policies
- trigger recap and scheduling updates

### TUI shell

Owns screen layout and navigation.

Responsibilities:

- pane layout
- mode switching
- focus control
- modal overlays
- keyboard shortcut handling

### Renderer

Transforms structured blocks into terminal output.

Responsibilities:

- text layout
- math block rendering
- widget display
- fallback strategies for unsupported terminals

### Persistence layer

Owns local durable state.

Responsibilities:

- schema migrations
- session records
- attempts and misconceptions
- deadlines and materials metadata
- resume snapshots

## State Ownership

V1 should be explicit about who owns what state.

### Durable state

Stored in SQLite or file-backed config:

- concepts and mastery state
- attempts
- misconceptions
- sessions
- deadlines
- materials metadata
- user settings

### Ephemeral state

Lives in memory during runtime:

- active transcript items
- current plan panel contents
- widget focus and cursor positions
- active input drafts
- app-server turn stream progress

### Recoverable transient state

Persisted frequently enough to survive crashes:

- active session id
- current mode
- unsent draft input
- currently open question widget state
- resume anchor into transcript

## Dependency Boundaries

V1 should avoid letting the app-server protocol leak into all layers.

Rules:

- UI widgets should consume internal domain events, not raw transport events
- renderer should consume content blocks, not raw markdown strings
- persistence should store normalized session outcomes, not arbitrary agent transcripts only
- local pedagogy logic should remain effective even if a turn fails or a stream is interrupted

## Future Compatibility

V1 must keep extension seams for:

- swapping in dedicated MCP servers later
- adding local OCR/material extraction workers
- enabling slide mode without restructuring transcript rendering
- supporting additional courses and content taxonomies
