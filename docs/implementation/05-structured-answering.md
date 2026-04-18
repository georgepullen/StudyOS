# Structured Answering

## Goal

Make answering maths questions inside the terminal fast enough and precise enough that students do not feel pushed back into plain text when real mathematical structure matters.

## Principle

The primary answer path in V1 is structured widget input.

Free text remains available where appropriate, but it should not be the default for core mathematical exercises.

## V1 Widget Set

### Matrix entry grid

Use for:

- matrix multiplication outputs
- inverses
- row-reduction steps
- vectors
- determinants where intermediate structured entries help

Requirements:

- arrow-key cell navigation
- `tab` and `shift-tab` cell movement
- row and column metadata
- support integer, decimal, fraction, and simple symbolic entries
- clear validation state per cell
- quick clear row or grid action
- submit without leaving the grid

### Working plus final answer form

Use for:

- derivations
- multipart exam-style questions
- interpretation plus numeric result
- “show your method” prompts

Fields:

- `working`
- `final_answer`

Optional prompt metadata:

- expected answer style
- mark weighting
- unit or notation hints

### Step list

Use for:

- derivation steps
- proof skeletons
- row-operation sequences
- reasoning chains

Requirements:

- multiple ordered steps
- add, delete, reorder
- per-step validation markers
- optional justification field in later iterations

### Short retrieval response

Use for:

- definition recall
- single-result calculations
- concept checks
- rapid warm-up prompts

Requirements:

- minimal friction
- one-line or compact multiline mode
- fast submit loop

## Submission Model

Every structured submission should normalize to an internal payload.

Example shape:

```text
submission_kind
question_id
prompt_id
concept_ids
widget_payload
display_snapshot
submitted_at
```

The `display_snapshot` exists so later review can show what the student actually entered, even if widget internals evolve later.

## Validation

Validation happens at three levels.

### Client-side structural validation

Examples:

- required field missing
- wrong matrix dimensions
- malformed fraction token

### Agent-side pedagogical grading

Examples:

- answer mathematically incorrect
- working omitted crucial justification
- final answer correct but explanation weak

### Persistence validation

Ensure stored submission shape matches expected schema.

## Question Types Mapped To Widgets

- matrix operation drill -> matrix grid
- concept definition -> short retrieval response
- derivation completion -> step list or working plus final answer
- interpretation question -> working plus final answer
- timed multipart question -> working plus final answer or mixed widget sequence

## Hint And Reveal Behavior

Hints and reveals should attach to the active widget context.

Requirements:

- hint should not destroy current input
- reveal should require explicit action
- revealed steps should be visually distinct from student-authored steps

## Anti-Passivity Behavior In Widgets

The widget layer should support anti-crutch design directly.

Examples:

- require non-empty attempt before reveal in strict mode
- warn when student tries to submit blank working with only a final answer for a method-mark question
- ask for one additional explanation sentence after a correct low-effort response when transfer is needed

## Non-Goals For V1 Widgets

- computer algebra syntax editor
- handwriting capture
- symbolic pretty-printer with full CAS semantics
- fully spreadsheet-like matrix editing
