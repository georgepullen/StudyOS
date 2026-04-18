# V1 Scope

## Goal

Define the real first usable version of StudyOS.

V1 should be strong enough that a student could use it as their daily terminal study environment for the target maths modules, even if some integrations and polish are still deferred.

## Target Courses

- Matrix Algebra & Linear Models
- Probability & Statistics for Scientists

## V1 User Promise

If a student opens StudyOS for a 20 to 90 minute session, the app should:

- recover what they studied recently
- show what is due now and what is urgent
- choose a sensible focus
- present questions with proper mathematical formatting
- let them answer using worksheet-like structured input
- resist passive reveal-first behavior
- store outcome evidence for future review

## V1 Features

### Core runtime

- local launcher command
- full-screen TUI shell
- app-server session connection over `stdio`
- thread start and resume
- streamed turn rendering
- graceful shutdown and resume

### Study UX

- session plan shown at session start
- transcript pane for tutor interaction
- toggleable session plan panel
- due review panel
- deadline urgency display
- scratchpad pane
- recap flow at session end

### Rich presentation

- headings and paragraph blocks
- block and inline math
- matrix display blocks
- question cards
- hint cards
- solution reveal cards
- warning and misconception boxes
- recap boxes

### Structured answering

- matrix entry grid
- working plus final answer form
- short retrieval response input
- step list input
- keyboard-first submission flow

### Study intelligence

- local memory-backed due review selection
- misconception resurfacing
- deadline-aware prioritization using local stored deadlines
- retrieval-first opening questions
- transfer question after success
- delayed full solutions

### Persistence

- SQLite database for concepts, attempts, sessions, misconceptions, deadlines
- local configuration files for course and calendar context
- resume state persistence

### Reliability

- launch health checks
- crash-safe persistence
- fallback rendering when graphics are unavailable
- explicit approval and error display when agent runtime requests it

## Common-Sense MVP Additions

These were not all explicitly called out earlier, but they belong in V1 because the product would feel incomplete without them.

- command palette or key help overlay
- persistent scratchpad text area
- session timer and visible progress state
- restore unsent draft input after accidental exit
- explicit loading and error states
- local config file for strictness and theme options
- import path for deadlines and materials metadata

## Out Of Scope For V1

- dedicated custom MCP servers
- live Google Calendar integration
- ICS sync
- OCR ingestion
- presentation-mode slide system
- browser-dependent workflow
- collaborative or multi-user features
- mobile support

## Deferred But Pre-Planned

These are out of V1, but V1 should leave space for them.

- confidence inference from behavioral signals
- dedicated math/stat helper services or MCP servers
- slide mode
- richer local material extraction from PDFs
- dynamic tools for client-native timers or modal forms
- analytics dashboard beyond logs and summaries

## V1 Success Conditions

V1 is successful when all of the following are true:

- a real student can use it repeatedly across multiple days
- the answer path is materially better than plain terminal chat
- the app remembers misconceptions and due reviews
- the session plan changes when deadlines or prior errors change
- full solutions are not the default interaction mode
- resume feels continuous rather than starting from scratch
