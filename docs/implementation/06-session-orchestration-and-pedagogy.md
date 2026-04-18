# Session Orchestration And Pedagogy

## Goal

Run each session as a focused learning loop rather than a generic tutoring chat.

## Session Phases

### 1. Open

Load:

- current local time
- prior session context
- due reviews
- recent misconceptions
- upcoming deadlines
- configured available session length if known

Then produce a session plan.

### 2. Retrieval warm-up

The first meaningful interaction should usually be retrieval, not explanation.

Examples:

- quick definition recall
- one-step matrix operation
- short interpretation question

### 3. Core teaching loop

For each target concept:

1. ask a diagnostic or retrieval question
2. inspect the answer
3. choose one of:
   - advance
   - repair
   - hint
   - reveal partial step
4. ask a transfer or explanation question after success

### 4. Review pass

Revisit:

- failed concepts
- misconceptions surfaced today
- high-urgency due reviews

### 5. Recap and schedule

Summarize:

- what was demonstrated
- what remains weak
- what will be reviewed next

## Session Plan Object

V1 session plan should include:

- recommended duration
- session mode
- reason for focus choice
- prerequisite checks
- warm-up prompts
- core target concepts
- stretch target if time remains
- planned review items

## Pedagogical Rules

### Attempt first

Do not show a full solution before the student attempts, unless they explicitly force a reveal or repeated failure makes it necessary.

### Explanatory cap

The agent should not emit long passive explanation monologues repeatedly without prompting student action.

### Transfer after success

A correct answer should often be followed by:

- “why does that work?”
- “what changes if ...?”
- “give a counterexample”

### Misconception persistence

When a student repeats a known misconception, the session should acknowledge it as a recurring issue and adapt accordingly.

### Repair before novelty

Do not keep introducing new content while a prerequisite failure remains unresolved for the current objective.

## Strictness

V1 default should be moderately strict.

Behavior:

- reveal is available but not immediate
- blank-answer reveal is discouraged
- some prompts require an explanation or method
- timed drill mode reduces hints

Strictness should be configurable later, but V1 needs one solid default.

## Local Policy Enforcement

Some anti-crutch behavior should live locally in the client rather than depending entirely on the agent.

Examples:

- reveal disabled until attempt made
- warning shown for repeated reveal-only behavior
- transfer question scheduled after quick correct answer

## Misconception Types

V1 should classify at least:

- conceptual misunderstanding
- procedural slip
- notation error
- sign or arithmetic error
- incomplete justification
- correct answer with weak reasoning

## End Of Session Output

Every session should end with structured evidence:

- concepts practiced
- concepts demonstrated
- concepts failed
- misconception entries created or updated
- next review times
- unfinished objectives

## Deferred Pedagogical Features

- behavioral confidence inference
- more advanced passivity heuristics
- adaptive difficulty model beyond simple mastery and urgency
- automatic exam-block generation from larger materials corpus
