# Operability And Testing

## Goal

Make V1 reliable enough for long study sessions and repeated daily use.

## Startup Requirements

Warm launch target:

- under 3 seconds where feasible on an already-installed local setup

Startup should verify:

- SQLite is available
- database opens successfully
- terminal capability mode selected
- app-server process can be spawned
- required local config paths are readable

## Logging

V1 should log:

- startup timing
- app-server initialization timing
- turn timing
- render timing
- submission events
- hint and reveal usage
- failure states

Logs should separate:

- operational logs
- study-content summaries where possible

## Health States

The header or activity panel should expose:

- database healthy or failed
- app-server connected or disconnected
- active turn in progress
- unsaved draft present

## Testing Layers

### Unit tests

For:

- content block parsing
- widget payload validation
- session planning helpers
- scheduling math
- persistence transforms

### Integration tests

For:

- startup and shutdown
- resume-state round trip
- SQLite migrations
- app-server event mapping

### Snapshot or golden tests

For:

- transcript block rendering
- matrix widget layout
- recap and question card rendering

### Manual test scripts

For:

- first launch
- interrupted session resume
- failed turn recovery
- deadline-aware plan change
- misconception resurfacing

## V1 Acceptance Tests

### Study loop

A student can:

1. launch StudyOS
2. see a plan
3. answer a structured question
4. receive feedback
5. end the session
6. reopen later and resume with memory intact

### Structured input

The matrix grid and working forms are fast enough to use without falling back to plain text for common exercises.

### Pedagogy

The app does not default to full reveal and surfaces at least one retrieval or transfer prompt during a normal session.

### Persistence

Attempts, misconceptions, deadlines, and recap summaries survive restart.

### Fallback

If graphics rendering is unavailable, the session remains usable in Unicode or plaintext-safe mode.

## Public Repo Discipline

Because this repository is public:

- no secrets in committed configs
- no personal calendar data checked in
- no real student answers in fixtures
- use synthetic example data only
