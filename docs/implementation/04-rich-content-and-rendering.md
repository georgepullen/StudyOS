# Rich Content And Rendering

## Goal

Render mathematical and pedagogical content cleanly enough that students do not feel they are studying through degraded plaintext.

## V1 Rendering Strategy

V1 supports two input paths:

### Structured content payloads

Preferred when strict shape matters.

Examples:

- session plan
- question cards
- recap objects
- grading feedback

### Rich-text directive parsing

Used for freer agent responses.

Examples:

- `:::question`
- `:::hint`
- `:::solution_reveal`
- `:::recap`
- fenced `math`
- fenced `matrix`

## Internal Content Blocks

Every rendered message should be parsed into a sequence of internal blocks.

V1 block types:

- `heading`
- `paragraph`
- `bullet_list`
- `math_inline`
- `math_block`
- `matrix_block`
- `question_card`
- `hint_card`
- `solution_reveal`
- `warning_box`
- `recap_box`
- `misconception_box`
- `progress_meter`
- `divider`

## Rendering Rules

### Text

- wrap to pane width
- preserve short list structure
- avoid reflowing code-like maths text incorrectly

### Inline math

- use Unicode-friendly inline rendering first
- avoid huge inline art that breaks sentence flow
- fall back to plaintext token form if parsing fails

### Block math

Primary target:

- graphics-capable local terminal

Preferred pipeline:

1. LaTeX or KaTeX-like parse
2. render to SVG or terminal-friendly cell image
3. place as a block inside transcript flow

Fallback pipeline:

1. aligned Unicode text representation
2. plaintext representation with safe wrapping

### Matrix blocks

Matrix rendering needs dedicated treatment.

Requirements:

- preserve row and column boundaries
- render brackets clearly
- support vectors and augmented matrices
- keep dimensions visible in metadata when useful

### Cards and boxes

Question, hint, recap, and warning blocks should feel visually distinct.

Requirements:

- title bar or label
- border or shading treatment
- compact metadata row where relevant
- consistent spacing

## Directive Parsing

V1 parser requirements:

- tolerate unknown directives without crashing
- preserve original text if a directive cannot be parsed
- attach source span metadata for debugging
- support nested math inside supported card bodies where practical

## Content Validation

Structured payloads should be validated before display.

If invalid:

- show a readable fallback block
- log the validation failure
- preserve raw content for debugging

## Rendering Capability Detection

At startup the client should determine:

- terminal size
- color capability
- graphics capability
- reduced-motion preference if configured

Renderer mode options:

- `rich_graphics`
- `unicode_rich`
- `plaintext_safe`

## Transcript Performance

The renderer should be optimized for long sessions.

Requirements:

- incremental render updates
- cached layout for old transcript blocks
- avoid rerendering heavy math blocks unnecessarily
- keep scroll stable while streaming

## Deferred Rendering Features

- slide frames
- diagram placeholders with richer visuals
- external PDF page embedding
- advanced animation or staged reveal effects
