# Launcher And Configuration

## Goal

Let a student get into a study session with near-zero friction while keeping local settings explicit and safe.

## Launch Experience

Target command:

```bash
studyos
```

Before that command exists as an installed binary, the repo-local equivalent is:

```bash
cargo run -p studyos-cli
```

## V1 Startup Behavior

On launch, the app should:

1. load local config
2. open local database
3. detect terminal capability
4. load course metadata
5. load deadlines and timetable files if present
6. connect to app-server
7. resume or start session

## Config Scope

V1 config should be local and file-backed.

Suggested config fields:

- default course
- strictness mode
- default session length
- theme
- reduced motion
- renderer mode override
- data directory path

## Suggested Config Path

```text
.studyos/config.toml
```

## Focus Mode

V1 focus mode should be intentionally lightweight.

Included:

- full-screen TUI
- visible session timer
- reduced chrome
- safe quit confirmation

Deferred:

- OS-level do-not-disturb control
- hard blocking of shell escapes
- external app blocking

## Resume Behavior

Default behavior:

- if there is a recent unfinished session, offer immediate resume
- otherwise start a new session using current local context

V1 can implement this as:

- auto-resume with a visible banner
- or a simple choose-resume-or-new prompt before entering the full session

## Command Surface

Expected future subcommands:

- `studyos`
- `studyos review`
- `studyos drill`
- `studyos doctor`

Only the main interactive launch is required in V1, but command parsing should leave room for later additions.

## Failure Behavior

If startup fails before TUI entry:

- print a readable terminal error
- include the failing subsystem
- suggest a next action where possible

If startup fails after TUI entry:

- show an in-app error state and recovery option

## Public Repo Safety

Because the repo is public, defaults should assume:

- no committed personal config
- no committed private calendar files
- no committed local database
- example config files use synthetic data only
