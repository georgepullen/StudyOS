# External Patterns to Borrow — agentic-stack & gstack

**Date:** 2026-04-19
**Purpose:** Read `/tmp/agentic-stack` (codejunkie99/agentic-stack) and `/tmp/gstack`
(garrytan/gstack), extract patterns that apply to StudyOS, and map each onto a
concrete file or subsystem so the next Codex pass can land them.
**Scope:** This is a design document, not a plan. It names *what* to borrow and
*where* it plugs in. Sequencing is handled in
`2026-04-19-reviewer-response-and-roadmap.md`.

---

## How to read this document

Each section follows the shape:

1. **The pattern** — a one-paragraph description drawn from the source repo,
   with a path into that repo so the reader can verify.
2. **Why it maps to StudyOS** — the specific StudyOS problem this solves. If
   the mapping is weak, the section says so and stops.
3. **Where it lands** — the concrete StudyOS file, table, or module that would
   absorb the pattern.
4. **Risks / don't-copy notes** — the bits of the source design that are wrong
   for a single-user adaptive tutor and should be dropped on the floor.

Two sources, two different lessons. **agentic-stack** is primarily about
*long-lived learned behaviour* — memory layers, skills, review protocols, how
an agent gets smarter across sessions. **gstack** is primarily about *gated
workflow discipline* — sprint roles as commands, completeness as default, CI
that keeps docs honest. StudyOS needs both: the tutor is the agent that must
learn the student (agentic-stack territory), and the build itself is currently
undisciplined AI slop that needs gated sprints to become trustworthy (gstack
territory).

---

## Part 1 — Patterns from agentic-stack

### 1.1 Four-layer memory mapped onto student modelling

**The pattern.** agentic-stack separates memory into four directories with
different retention policies (`/tmp/agentic-stack/.agent/memory/`):

- `working/` — live task state, archived after 2 days
- `episodic/` — JSONL log of what happened, scored by salience
- `semantic/` — distilled patterns that outlive episodes
- `personal/` — user-specific preferences, *never* merged into semantic

The invariant that matters is the last one: personal preferences *never*
graduate into the shared brain. Preferences are inputs, not evidence.

**Why it maps to StudyOS.** StudyOS today has a single `attempts` table and a
loose notion of `misconceptions`. That is too flat. A tutor needs to
distinguish:

- What is the student doing *right now in this session* (working)
- What happened in prior attempts (episodic)
- What repeated patterns across attempts imply about the student's model
  (semantic — the real misconception ledger)
- Stable student-level facts: exam board, target grade, preferred pace,
  ADHD accommodations, which subjects are revision vs. first-exposure
  (personal)

Conflating episodic with semantic is exactly how existing AI-tutors devolve
into "you got one question wrong, so you have a misconception" noise. The
four-layer model gives us a disciplined place to put evidence vs. conclusions.

**Where it lands.** `crates/studyos-core/src/store.rs` + SQLite schema:

- `working` → rename the current in-memory `AppState` scratch as a durable
  `session_working` row keyed by session id, so a crash mid-session is
  recoverable. 2-day retention matches agentic-stack.
- `episodic` → the existing `attempts` table *is* this layer. Already there;
  it just needs renaming in docs so its role is clear.
- `semantic` → promote the `misconceptions` table into this role, but gate
  insertion behind a review protocol (see §1.3). Today it's populated
  directly from a single attempt; that is the bug.
- `personal` → new `student_profile` table (or upgrade the existing
  `StudyConfig`) with fields: exam board, grade target, session length
  preference, "I struggle with X" self-reports. **Never derived from
  attempts** — only written by the student via `studyos preferences` or the
  onboarding wizard.

**Risks / don't-copy notes.**
- Don't copy agentic-stack's *file-per-memory* layout. StudyOS already has
  SQLite; tables with columns are better than JSONL plus Markdown for an app
  that will query by (student, topic, time-window). Keep the *concepts*, use
  the existing store.
- Don't copy "archive after 2 days" blindly for working memory. Revision
  sessions can span weeks — a dormant session should probably expire after
  14 days, not 2.

---

### 1.2 Progressive-disclosure skills for pedagogical strategies

**The pattern.** `_index.md` and `_manifest.jsonl` are always loaded; a full
`SKILL.md` is only pulled in when its triggers match the current task
(`/tmp/agentic-stack/docs/architecture.md` §Skills). Every skill declares
`triggers`, `preconditions`, `constraints`, and ends with a self-rewrite hook.

**Why it maps to StudyOS.** The structured-output schemas
(`crates/studyos-core/src/tutor.rs`) currently bake one pedagogical recipe —
"explain, then Socratic question, then feedback" — into a single prompt. A
competent tutor has *many* recipes: scaffolded worked example, error analysis,
spaced-retrieval drill, stretch extension, analogy bridge, metacognitive
prompt. We need to load the *right recipe* per turn based on the student's
current state (fresh topic vs. repair after an error vs. overlearning drill)
without inflating the system prompt with all seven recipes every turn.

**Where it lands.**

- New directory `crates/studyos-core/strategies/` with one Markdown file per
  strategy (e.g. `scaffold.md`, `error_repair.md`, `retrieval_drill.md`,
  `stretch.md`). Each file has YAML frontmatter: `triggers`, `preconditions`,
  `output_schema` (name of the schema variant to apply), `self_rewrite_after`
  (usage count or failure count threshold).
- New field in `Session` (see `crates/studyos-core/src/session.rs`):
  `active_strategy_id: StrategyId`. Chosen at turn planning time by a
  strategy-selector function that reads session state + recent episodic log.
- The Codex prompt sent from `runtime.rs::start_structured_turn` includes
  only the selected strategy's body, not all of them.

**Risks / don't-copy notes.**
- Don't let the student see this machinery. Strategy switches should be
  invisible; what they see is "the tutor chose to start with a worked
  example."
- Don't ship all seven recipes at once. Seed with two (scaffold and
  error-repair) and add only when real attempt logs show a gap. gstack's
  "build for yourself" ethos applies: add the third strategy when *you* hit
  the situation it's for, not before.

---

### 1.3 Review-with-rationale before evidence becomes belief

**The pattern.** agentic-stack's `auto_dream.py` stages candidate lessons
mechanically — cluster recurring patterns, pre-filter for length and exact
duplicates — but **does not promote**. A human (or host agent) runs
`graduate.py <id> --rationale "..."` or `reject.py <id> --reason "..."`.
Graduation *requires* a rationale, so rubber-stamping is structurally
impossible (`/tmp/agentic-stack/README.md` lines 103–126). Rejected
candidates keep their full decision history, so recurring churn is visible.

**Why it maps to StudyOS.** The single most dangerous failure mode of an
AI-tutor is *false misconception attribution*: the student slipped, the tutor
encoded "student doesn't understand integration by parts", and now every
future session over-remediates a skill they already have. This creates
learned helplessness and erodes trust faster than any UI bug.

The fix is structural: an attempted error becomes a *candidate* misconception,
not a confirmed one, and only graduates into the student model after either
(a) multiple corroborating attempts or (b) explicit student acknowledgement.

**Where it lands.** Add two tables to `crates/studyos-core/src/store.rs`:

- `misconception_candidates` — id, student_id, claim (short text), evidence
  (JSON list of attempt_ids), first_seen, last_seen, status (`pending` |
  `graduated` | `rejected`), rationale (required on graduate/reject).
- `misconception_decisions` — id, candidate_id, decided_at, decided_by
  (`auto_threshold` | `student_confirmed` | `tutor_ruled_out`), rationale.

Promotion rules (encoded in `crates/studyos-core/src/session.rs`):

- Auto-graduate when the same candidate fires on ≥3 attempts within 14 days
  and confidence from the structured-output grader is ≥0.7 on each.
- Student can graduate or reject at end-of-session recap with a single
  keystroke: "Does this sound right? [Y] yes, I get confused by X / [N] no,
  that was a slip." The `[N]` path writes `rejected` with rationale = "student
  reported slip."
- Rejected candidates stay in the table. If the same claim re-appears three
  more times after rejection, the decision log makes the churn visible and
  the tutor escalates differently.

**Risks / don't-copy notes.**
- Don't copy the `auto_dream.py` *scheduling* model (nightly cron) for v1.
  A single-user study app has a natural moment to run the stage cycle: at
  session close, inside `finish_session`. No cron, no background process.
- Don't require the student to type free-form rationale. That is friction
  that kills consent flows. Use enumerated options plus an optional note.

---

### 1.4 `permissions.md` as an enforced contract for the tutor

**The pattern.** `/tmp/agentic-stack/.agent/protocols/permissions.md` is a
human-readable allow / approval-required / never-allowed list, enforced by
the pre-tool-call hook. Skills cannot modify this file. That one constraint —
"the agent cannot edit the rules it is judged by" — is what keeps the system
honest over time.

**Why it maps to StudyOS.** The tutor today has no formal contract about what
it can or cannot do. Can it claim the student has a misconception after one
attempt? Can it mark a topic "mastered" without a timed retrieval check? Can
it escalate difficulty without student consent? These are implicit
assumptions scattered across prompts. They should be explicit, versioned, and
read into the prompt context every turn.

**Where it lands.**

- New file `crates/studyos-core/contracts/tutor_rules.md` — committed,
  human-readable. Sections: *always-allowed* (ask a probing question,
  generate a worked example), *requires-student-consent* (mark topic
  mastered, end session early, add a new topic to the plan),
  *never-allowed* (silently change grade target, override an explicit
  "come back to this later" request, claim mastery without a retrieval
  check).
- `runtime.rs::start_structured_turn` injects this file verbatim into the
  system portion of the Codex prompt. Because it is loaded every turn, every
  response is judged against the current rules.
- CI check: `cargo test contract_rules_present` reads the file and fails the
  build if any of the three section headers are missing or empty. Cheap
  insurance against the file being accidentally gutted.

**Risks / don't-copy notes.**
- Don't put this file in a place the tutor's own output pipeline can edit.
  agentic-stack's rule — "only humans edit permissions.md" — should apply
  here too. The student can edit it; the Codex agent behind the tutor must
  not be able to.

---

### 1.5 Self-rewrite hooks on strategy failure

**The pattern.** Every agentic-stack skill ends with:

> After every 5 uses OR on any failure: read the last N skill-specific
> episodic entries; if a new failure mode has appeared, append to
> `KNOWLEDGE.md`; if a constraint was violated, escalate to
> `semantic/LESSONS.md`; commit.

(`/tmp/agentic-stack/docs/writing-skills.md` §Self-rewrite hook.)

**Why it maps to StudyOS.** A pedagogical strategy that keeps failing on a
particular student (scaffold always over-helps them; drill always under-tests
them) should either be adjusted or retired for that student. The current
StudyOS has no loop back from "this turn went badly" to "use a different
strategy next time."

**Where it lands.** A new `strategy_health` view, computed from the attempts
table, tracked per (student_id, strategy_id):

- Success rate over last N uses
- Time-since-abandon (did the student drop the session mid-strategy?)
- Grader confidence trajectory

When success rate on a strategy drops below threshold for a given student,
the strategy selector (§1.2) deprioritises it and emits a `strategy_flagged`
episodic entry. A nightly (on-next-launch, actually) job reads flagged
strategies and writes a short advisory into the student's personal layer:
"`scaffold` has a low hit-rate for you on calculus topics; trying
`error_repair` first." This is the StudyOS analogue of agentic-stack's
KNOWLEDGE.md accumulation.

**Risks / don't-copy notes.**
- Don't commit the advisory to git. Student-specific. `personal/` layer, not
  `semantic/`.
- Don't retire a strategy after one bad session. Strategies fail for reasons
  unrelated to the strategy (bad night, topic-specific difficulty). Require
  a rolling window.

---

### 1.6 The `AGENTS.md` map (harness-agnostic brain)

**The pattern.** agentic-stack's `AGENTS.md` is a short, load-every-session
map that tells the agent where everything lives. It exists because the brain
is portable across eight harnesses and the agent has to find its own notes
(`/tmp/agentic-stack/adapters/claude-code/CLAUDE.md` lines 5–10).

**Why it maps to StudyOS.** We have none of this today. The tutor's prompt
context is assembled ad-hoc per turn inside `bootstrap_study_context` and
similar helpers. It works, but it is fragile: a new contributor cannot see
at a glance what the tutor is guaranteed to know. A map file fixes that.

**Where it lands.** `crates/studyos-core/contracts/TUTOR_MAP.md` — loaded
verbatim into the system prompt on every turn. Sections: where the student
profile is; where rules are (§1.4); where the candidate misconceptions live;
where recent session recaps live; where strategies are; *what to do first*
(read profile → read rules → read active strategy → start turn).

**Risks / don't-copy notes.**
- Don't ship a multi-harness adapter layer. StudyOS is not harness-agnostic
  and pretending it is adds weight for no benefit. One binary, one brain.

---

### 1.7 Things from agentic-stack *not* to borrow

- **Cron-driven dream cycles.** Single-user app, no need. Stage at
  session-close.
- **FTS5 search over memory.** StudyOS already has SQLite; a structured query
  (topic, last_seen, strategy_id) is more useful than keyword search and
  does not need a separate index.
- **Onboarding wizard that writes 6+ preference files.** StudyOS's onboarding
  (exam board, target grade, pace) is enough. More questions up front means
  fewer students who finish setup.
- **Eight adapter shims.** Zero.

---

## Part 2 — Patterns from gstack

### 2.1 "Boil the Lake" as the default for the rebuild pass

**The pattern.** gstack's `ETHOS.md` argues that AI-assisted coding makes
completeness near-zero marginal cost. A 150-LOC full implementation is
preferred over an 80-LOC 90% implementation because the 70-line delta costs
seconds with AI, and the 10% gap costs trust later
(`/tmp/gstack/ETHOS.md` §1).

**Why it maps to StudyOS.** The companion review document
(`2026-04-19-comprehensive-build-review.md`) is full of "we deferred X to a
follow-up" admissions: no integration test against the live Codex app-server,
no schema-validated request shape, no resume recovery test, no recap failure
handling, no end-to-end CLI smoke. Each is exactly the kind of "90% ship"
that gstack's ethos warns against. The next pass should not ship another
90%. Phase 0 and Phase 1 in the companion roadmap already embed this — but
the ethos itself is worth lifting as a stated project value, not just a
checklist item.

**Where it lands.**

- Add `docs/ETHOS.md` (short, <60 lines) stating the three rules: *boil the
  lake* for any module under the user's fingers (TUI, tutor schema, runtime
  transport), *search before building* for anything touching Codex protocol
  surface, *user sovereignty* for pedagogical decisions.
- Referenced from the top of `docs/review/2026-04-19-reviewer-response-and-roadmap.md`
  as the ethical frame; each phase exit criterion can then be written as
  "what a boiled lake looks like here."

**Risks / don't-copy notes.**
- "Boil the lake" does not mean "write tests for trivial getters." It means
  "if you are touching the tutor output parser, handle the malformed JSON
  branch, not just the happy branch." Aim it at quality, not volume.

---

### 2.2 "Search Before Building" applied to the Codex app-server contract

**The pattern.** Three layers of knowledge (`/tmp/gstack/ETHOS.md` §2):
Layer 1 (tried-and-true — you know these, but verify), Layer 2 (new and
popular — search, but scrutinise), Layer 3 (first-principles — prize these,
they are the eureka moments). The anti-pattern explicitly called out is
"rolling a custom solution when the runtime has a built-in."

**Why it maps to StudyOS.** `crates/studyos-cli/src/runtime.rs` currently
sends `experimentalRawEvents` and `persistExtendedHistory` fields in
`thread/start` and `thread/resume` parameters. Neither appears in the
generated schema under `/tmp/codex-schema/v2/`. This is exactly the
anti-pattern: hand-invented request shapes when the runtime has a schema.

**Where it lands.**

- Vendor the Codex v2 schema into `crates/studyos-core/schema/` at a pinned
  version. `build.rs` uses `typify` or `schemars` to generate Rust types
  from the JSON Schemas.
- Replace every hand-rolled struct in `runtime.rs` with the generated type.
  If the field does not exist in the schema, it cannot be sent.
- CI check: `cargo test runtime_requests_schema_valid` serialises the app's
  canonical `thread/start` / `thread/resume` / `turn/start` payloads and
  validates them against the vendored schemas. Fails the build on drift.

**Risks / don't-copy notes.**
- The schema itself is an input, not truth. If a Codex field we need is
  genuinely missing, log it, vendor a typed extension in a separate file,
  and note the divergence in `docs/review/`. Don't silently hand-roll.

---

### 2.3 Skill-template + CI freshness check for the tutor strategies

**The pattern.** gstack writes `SKILL.md.tmpl` (human-written prose with
placeholders), runs `gen-skill-docs.ts` at build time to fill the
placeholders from source-code metadata, and commits the result. CI runs
`gen:skill-docs --dry-run` + `git diff --exit-code` to catch stale docs
before merge (`/tmp/gstack/ARCHITECTURE.md` §"SKILL.md template system").
The load-bearing claim: *if a command exists in code, it appears in docs;
if it doesn't exist, it can't appear.*

**Why it maps to StudyOS.** The strategies proposed in §1.2 each reference a
structured-output schema variant. If the schema drifts, the strategy prompt
drifts with it or stops working silently. The template pattern keeps the
strategy Markdown honest about what fields it actually gets.

**Where it lands.**

- `crates/studyos-core/strategies/scaffold.md.tmpl` — prose plus
  `{{OUTPUT_SCHEMA_FIELDS}}` placeholder.
- `xtask gen-strategies` (a `cargo xtask` subcommand) reads
  `tutor_output_schema` variants and renders the `.md` files.
- CI check: run `cargo xtask gen-strategies --check` in the pipeline;
  `git diff --exit-code` catches drift.

**Risks / don't-copy notes.**
- Don't port gstack's entire template placeholder zoo. We need two
  placeholders, not fifteen.

---

### 2.4 Preamble + universal AskUserQuestion format

**The pattern.** Every gstack skill starts with a shared `{{PREAMBLE}}` block
that standardises five things: update-check, session tracking, operational
self-improvement logging, a universal `AskUserQuestion` shape (context,
question, `RECOMMENDATION: Choose X because ___`, lettered options), and the
Search-Before-Building reminder (`/tmp/gstack/ARCHITECTURE.md` §"The
preamble"). Consistency is the feature.

**Why it maps to StudyOS.** The structured tutor output today has no
standard shape for *how it asks the student a question.* Sometimes it's an
open prompt, sometimes multiple-choice, sometimes a scaffolded fill-in.
Students lose predictability — they never know if "press a letter" will work
until they try. A universal shape for student questions solves this and
makes TUI handling trivial (one widget per shape).

**Where it lands.**

- Fix the `tutor_output_schema` ask-shape enum to exactly three variants:
  `open_response`, `letter_choice { options: [Labelled] }`, `structured_math
  { template }`. Drop any fourth shape silently being generated.
- TUI widget (`crates/studyos-cli/src/app.rs`) dispatches on the enum and
  renders the matching widget. Unknown variants are a parse-time error, not
  a runtime surprise.

**Risks / don't-copy notes.**
- Don't copy gstack's "ELI16 mode when 3+ sessions are running." Not
  relevant to a single-student app.

---

### 2.5 Read/Write/Meta command taxonomy for CLI subcommands

**The pattern.** gstack partitions browser commands into READ / WRITE / META
sets and uses the partition for dispatch *and* for the `help` output
(`/tmp/gstack/ARCHITECTURE.md` §"Command dispatch"). Knowing which bucket a
command is in is a precondition for reasoning about retries, idempotency,
and safety.

**Why it maps to StudyOS.** `crates/studyos-cli/src/main.rs` currently has
`init, doctor, deadlines, courses, materials, timetable, preferences,
session` subcommands. Some are read-only (`doctor`, `deadlines --list`),
some mutate the local DB (`deadlines --add`, `courses --add`), some spawn
the Codex runtime (the interactive TUI). They are not partitioned. A user
running `deadlines --list` should not need the Codex binary to be
installed; today that is unclear.

**Where it lands.**

- Tag each subcommand in `main.rs` with a `SubcommandKind` enum: `LocalRead`,
  `LocalWrite`, `RuntimeSession`.
- `LocalRead` + `LocalWrite` subcommands must not spawn Codex. Doctor check
  that the Codex binary exists is only required for `RuntimeSession`. This
  means `studyos deadlines --list` works on a plane.
- `--help` groups commands by kind so the user sees what is local vs. what
  needs the network/Codex.

**Risks / don't-copy notes.**
- Don't import the full dispatch machinery (`READ_COMMANDS.has(cmd)`). Rust's
  enum + match is tidier than a string set.

---

### 2.6 Actionable errors, written for the next agent

**The pattern.** "Errors are for AI agents, not humans" — every error
rewritten through `wrapError()` to strip stack traces and add guidance
(`/tmp/gstack/ARCHITECTURE.md` §"Error philosophy"):

- "Element not found" → "Element not found. Run `snapshot -i` to see
  available elements."
- "Selector matched multiple" → "Use @refs from `snapshot` instead."

**Why it maps to StudyOS.** Errors in StudyOS today bubble up as `anyhow`
chains ("failed to parse structured tutor payload: invalid type: null, ...").
That's fine for a human debugger, useless inside the TUI. A student sees an
opaque red line and quits. More importantly: when a next-turn auto-recovery
is possible ("the tutor returned malformed JSON; request a repair"), the
error message is the only place to document that.

**Where it lands.**

- Introduce `StudyError` in `crates/studyos-core/src/error.rs` with variants
  that each carry a `user_message` (friendly, actionable) and a
  `operator_message` (full debug chain, written to `studyos.log`).
- TUI renders only `user_message`. Log file gets both.
- Where a recovery is possible (malformed tutor JSON, stalled Codex turn),
  the `user_message` names the recovery the runtime will attempt: "Tutor
  output was malformed; retrying once." Then the runtime actually retries.

**Risks / don't-copy notes.**
- Don't over-invest in error taxonomy. Five or six variants cover the real
  cases. Anything beyond that is busywork.

---

### 2.7 State file with atomic write + health check, not PID tracking

**The pattern.** gstack's browse daemon writes
`.gstack/browse.json` with atomic tmp+rename, mode 0600, containing
`{ pid, port, token, startedAt, binaryVersion }`. On every CLI invocation,
it does a health check (GET /health) because PID-based liveness detection
is unreliable (`/tmp/gstack/ARCHITECTURE.md` §"State file").

**Why it maps to StudyOS.** The companion review flagged that
`AppServerClient` has lifecycle gaps — if the Codex process dies mid-session,
the CLI does not detect it cleanly. Also, resumable sessions need a
crash-survivable handle to the active thread id, port if app-server is
long-lived, and version of the Codex binary that originated the thread.

**Where it lands.**

- New file `~/.studyos/session.json`, atomic-written on `thread/started`:
  `{ thread_id, codex_version, started_at, pid, checksum_of_active_strategy }`.
- On startup, if the file exists, StudyOS attempts `thread/resume` against
  the recorded thread_id and Codex version. Mismatched version → warn, start
  new thread, archive old one.
- Replace the current "is the child alive?" check with a request/response
  ping before every turn. If the ping fails, respawn.

**Risks / don't-copy notes.**
- We don't need HTTP + bearer tokens (stdio is already confined to the
  spawned child). The atomic-write and health-check discipline is the
  portable lesson.

---

### 2.8 Three-tier test pyramid

**The pattern.** gstack's docs (`ARCHITECTURE.md` §"Template test tiers" and
§"Test tiers") explicitly separate: Tier 1 static validation (free, <5s,
runs every commit), Tier 2 real end-to-end (expensive, gated behind a flag),
Tier 3 LLM-as-judge (cheap but noisy, judgement calls only). "Catch 95% of
issues for free, use LLMs only for judgement calls."

**Why it maps to StudyOS.** Today StudyOS has only Tier-1-ish tests (core
types, DB layer). No Tier 2 (no real Codex-server integration), no Tier 3
(no judgement over whether tutor output is actually pedagogically sound).
The roadmap in the companion doc already requires Tier 2; this pattern
adds Tier 3 as a gated check.

**Where it lands.**

- `cargo test` stays Tier 1 — fast, green, runs every commit.
- `cargo test --features integration` spawns a real Codex app-server via a
  test fixture (see roadmap Phase 1). Tier 2, ~30s, gated.
- `cargo xtask eval-tutor --fixtures tests/fixtures/tutor_prompts/` sends a
  handful of canonical turns through the tutor and asks a second Codex
  instance to score each response against the pedagogical contract in §1.4.
  Tier 3, runs on-demand. Results go under `docs/evals/YYYY-MM-DD.md`.

**Risks / don't-copy notes.**
- Tier 3 results are *signal*, not ground truth. Track trend lines, not
  absolute pass/fail. Don't let it become a flaky CI blocker.

---

### 2.9 `/retro` — weekly retrospective reading the evidence log

**The pattern.** gstack's `/retro` skill reads the last N days of git log and
produces a senior-IC-level retrospective with per-person praise and growth
areas (`/tmp/gstack/retro/SKILL.md.tmpl`). It's the system's memory speaking
back to the user. Midnight-aligned windows, local timezone, compare mode.

**Why it maps to StudyOS.** StudyOS's whole premise is that the student
learns over weeks, but there is no moment in the current build where the
student sees *their own arc*. A weekly retro — "here is what you attempted,
what stuck, what regressed, what strategies worked for you" — is the thing
that turns a drill app into a study partner.

**Where it lands.**

- New subcommand `studyos retro [7d|14d|30d]`. Reads the attempts table,
  the misconception decisions table, and the strategy health view. Produces
  a Markdown report: topics attempted, mastery trajectory, misconception
  churn (items flagged-then-rejected ≠ zero is a signal to investigate
  grader calibration), strategies that worked, weeks with zero sessions.
- Saves to `~/.studyos/retros/YYYY-MM-DD.md`. Every retro is kept; the
  student builds a study autobiography.

**Risks / don't-copy notes.**
- Don't call a Codex turn to generate the retro in v1. Compute it from the
  local tables. The prose wrapper can come later; the numbers are the value.
- Don't compare to other students. Single-user tool; no leaderboards.

---

### 2.10 Things from gstack *not* to borrow

- **Conductor parallel-workspace model.** StudyOS is one process with one
  student, not ten parallel dev workspaces.
- **The Bun/TS/Chromium runtime architecture.** Irrelevant — StudyOS is a
  Rust TUI. The *discipline* (daemon-over-HTTP, token auth, version
  auto-restart) generalises; the implementation doesn't.
- **Browser ref system (`@e1`, `@c1`).** Not applicable.
- **23 specialist skills + 8 power tools.** The full gstack command zoo is
  calibrated for a professional dev doing a week of work per day. StudyOS
  needs a handful (tutor, retro, plan, doctor), not a circus.

---

## Part 3 — Synthesis: what the rebuilt StudyOS borrows, in order of value

Ranked by "how much does this move the project from 'aspirational Codex
output' to 'actually usable study partner'":

1. **Review-with-rationale for misconceptions** (§1.3). Without this, the
   student model is corrupted on day 1. Highest leverage.
2. **Schema-driven Codex request shapes, enforced in CI** (§2.2). Without
   this, runtime works by coincidence. Blocks Phase 1 of the roadmap.
3. **Tutor rules as a committed contract injected every turn** (§1.4).
   Without this, "boil the lake" has nothing to boil toward.
4. **Progressive-disclosure strategies with self-rewrite on failure**
   (§1.2 + §1.5). The first feature students will actually feel.
5. **Four-layer memory taxonomy applied to existing SQLite tables** (§1.1).
   Mostly a renaming + a new `student_profile` table; unlocks clean queries
   for the retro and strategy selector.
6. **Actionable error taxonomy with retry semantics** (§2.6). Catches the
   malformed-JSON class of runtime failures that today kill sessions.
7. **Weekly `studyos retro`** (§2.9). Low code cost, very high motivational
   return for the student.
8. **State file with atomic write + health-check ping** (§2.7). Enables
   resume-after-crash cleanly.
9. **Read/Write/Runtime subcommand taxonomy** (§2.5). Small, nice quality-of-
   life improvement; `studyos deadlines --list` without Codex.
10. **Three-tier test pyramid with Tier 3 LLM-judge eval** (§2.8). Needed
    eventually; gates on Phase 2 Tier 2 being real first.

Items 1–3 are the "boil the lake" targets for the next Codex pass. Items
4–7 are Phase 3–4 work. Items 8–10 are ambient infrastructure that should
be standing by the time the app is in real daily use.

---

## Part 4 — Directives the next Codex pass should lift verbatim

If the next Codex pass reads only one file, it should read this section.

1. **Never write a tutor prompt field that is not in the vendored Codex
   schema.** If you need a field that is not there, stop and write it up as
   a schema extension in `crates/studyos-core/schema/extensions.md` *before*
   touching runtime code. (§2.2)
2. **Never insert a row into `misconceptions` directly.** Insert into
   `misconception_candidates` with evidence. Promotion is a separate code
   path that requires either ≥3 corroborating attempts or explicit student
   consent, with a rationale. (§1.3)
3. **Never embed a pedagogical rule inside the main prompt.** Put it in
   `tutor_rules.md` and load the file. If you want to change a rule, edit
   the file, not the prompt assembly. (§1.4)
4. **Every user-visible error has both `user_message` and `operator_message`.
   The TUI renders the former; logs get both.** (§2.6)
5. **Every new schema-driven artefact (strategy, tutor rule, output schema
   variant) ships with a CI check that fails on drift.** `cargo xtask
   --check` + `git diff --exit-code`. (§2.3)
6. **No new pedagogical strategy lands without a rationale in the commit
   message and a fixture under `tests/fixtures/tutor_prompts/`.** One
   strategy, one fixture, one test assertion about the output shape. (§1.2)

---

## Part 5 — What success looks like after absorbing these patterns

Concretely, these are the six visible outcomes. If the next pass achieves
them, StudyOS has absorbed the patterns. If it cannot demonstrate them, the
borrowing did not happen.

1. `cargo build && cargo test && cargo xtask --check` is green on a clean
   clone. (gstack discipline)
2. A session can be killed mid-turn; next launch resumes cleanly with the
   correct thread, no duplicate turns. (§2.7)
3. A deliberately malformed tutor response produces one retry, a friendly
   user-visible message, and a full operator-log entry. No stack trace on
   the TUI. (§2.6)
4. A student can run `studyos retro 7d` with zero configuration and see
   their own arc — topics, hit-rate per strategy, misconception churn.
   (§2.9)
5. `misconception_candidates` has rows after one drill session; the
   `misconceptions` table does not, unless the student explicitly confirmed
   or three independent attempts corroborated. (§1.3)
6. Deleting `tutor_rules.md` fails CI. Editing one of the three required
   sections to be empty fails CI. (§1.4)

Anything short of these six is an incomplete implementation of the patterns,
and should be flagged back for another pass rather than shipped.

---

## Appendix — file pointers used in this document

- agentic-stack README: `/tmp/agentic-stack/README.md`
- agentic-stack architecture: `/tmp/agentic-stack/docs/architecture.md`
- agentic-stack writing-skills: `/tmp/agentic-stack/docs/writing-skills.md`
- agentic-stack Claude Code adapter: `/tmp/agentic-stack/adapters/claude-code/CLAUDE.md`
- gstack ethos: `/tmp/gstack/ETHOS.md`
- gstack architecture: `/tmp/gstack/ARCHITECTURE.md`
- gstack SKILL template: `/tmp/gstack/SKILL.md.tmpl`
- gstack review skill: `/tmp/gstack/review/SKILL.md.tmpl`
- gstack retro skill: `/tmp/gstack/retro/SKILL.md.tmpl`
- Companion StudyOS review: `docs/review/2026-04-19-comprehensive-build-review.md`
- Companion StudyOS roadmap: `docs/review/2026-04-19-reviewer-response-and-roadmap.md`
