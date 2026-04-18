# App-Server Integration

## Goal

Use Codex app-server as the agent runtime while keeping the client responsible for educational UX and local state.

## Transport

V1 transport:

- `stdio`

Reason:

- simplest local reliability
- least moving parts
- easiest bootstrap and debugging path

## Startup Flow

1. load local config
2. open or initialize SQLite
3. detect terminal capabilities
4. spawn app-server process
5. perform protocol initialization
6. health-check essential runtime assumptions
7. open existing thread or start a new study session

## Thread Behavior

V1 must support:

- start new thread
- resume prior thread
- list resumable threads later if exposed cleanly

Primary UX behavior:

- by default resume the latest unfinished study thread if it is still relevant
- otherwise start a new thread with fresh opening context

## Turn Flow

For each turn:

1. prepare turn context
2. include structured session state and local study context
3. start turn
4. stream items into transcript state
5. map approvals or tool activity into UI overlays
6. validate and render final blocks
7. persist resulting study evidence

## Opening Context Payload

V1 opening turns should include:

- local date and time
- course or active topic
- available session minutes
- due review summary
- recent misconception summary
- deadline summary
- current strictness policy
- rendering capability hints
- structured input availability

## Skills

V1 should be ready to inject these core skill intents even if their implementation is initially prompt-driven:

- diagnose-understanding
- teach-linear-models
- teach-probability-statistics
- generate-questions
- socratic-repair
- timed-exam-drill
- close-session-and-schedule

## Structured Output Contract

Use schema-shaped outputs where precision matters most.

Priority objects:

- session plan
- question card
- grading feedback
- misconception report
- session recap

Free rich text remains allowed for explanation as long as the directive parser can render it safely.

## Event Mapping

The client should convert raw app-server events into internal app events.

Examples:

- `turn_started`
- `stream_chunk_received`
- `message_completed`
- `approval_requested`
- `tool_activity_started`
- `turn_completed`
- `turn_failed`

This keeps transport details out of the widget and renderer layers.

## Approvals

V1 must surface:

- command approvals
- file-change approvals
- tool-input requests

The UI should present approvals in an interruptible but clear way, without losing the active question context.

## Failure Handling

Examples:

- stream interrupted mid-turn
- malformed structured output
- app-server process exited
- timeout waiting for initialization

Required behavior:

- show clear status
- keep transcript history intact
- allow retry or resume
- avoid corrupting session memory

## V1 Constraint

V1 should not require custom MCP servers to function end to end.

Where tool behavior is needed early, the system may rely on:

- Codex core runtime capabilities
- local Python scripts invoked intentionally later
- client-side persistence and scheduling logic
