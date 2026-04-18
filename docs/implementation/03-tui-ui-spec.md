# TUI UI Spec

## UI Goal

The interface should feel like a focused interactive maths workstation, not a terminal chat log.

## Primary Views

### Study view

Default mode for most sessions.

Contains:

- header bar
- transcript pane
- contextual side panel or bottom drawer
- structured answer area

### Review view

Prioritizes due reviews and recent misconceptions.

Contains:

- compact concept queue
- one-question-at-a-time review flow
- quick navigation between due items

### Drill view

Timed question sequence with reduced explanatory chatter.

Contains:

- timer
- single active prompt
- structured answer widget
- compact feedback strip

### Recap view

End-of-session summary and next-review commitments.

Contains:

- session outcomes
- concept wins and misses
- next scheduled reviews
- unfinished objectives

## Layout

### Header

Must always show:

- current mode
- course or topic
- session timer or elapsed time
- due review count
- deadline urgency indicator
- connection health indicator

### Main transcript pane

Shows:

- streamed tutor output
- question blocks
- feedback blocks
- rich math blocks
- session markers

Requirements:

- smooth scroll
- keyboard navigation
- jump to latest
- jump to previous question
- visible marker for active prompt

### Secondary panel

Panel tabs for:

- session plan
- due reviews
- deadlines
- misconceptions
- scratchpad
- activity

Requirements:

- open on right in wide terminals
- collapse to bottom drawer on smaller sizes
- preserve tab state across mode changes

### Answer area

The answer area should not be a generic single-line chat composer.

Requirements:

- host structured widget for active question
- show prompt-specific controls
- show hint and reveal actions when allowed
- show submit status and validation messages

## Keyboard Model

### Global keys

- `q`: begin safe quit flow
- `?`: open key help
- `tab` and `shift-tab`: cycle focus regions
- `esc`: close modal or unfocus detail pane
- `ctrl-l`: force redraw

### Navigation keys

- arrows or `hjkl`: move within lists and panes
- `g`: jump to top of transcript
- `G`: jump to latest transcript item
- `]`: next pending item or question
- `[`: previous question or important marker

### Panel keys

- `1` to `5`: switch panel tab
- `s`: open scratchpad
- `p`: open session plan
- `d`: open deadlines

### Answer-flow keys

- `enter`: submit where safe
- `ctrl-enter`: force submit from multiline areas
- `h`: request hint when allowed
- `r`: request reveal when allowed

## Safe Quit Flow

The user must always be able to leave, but leaving should preserve continuity.

On quit:

- ask for confirmation if there is an active unsaved answer
- save resume state
- ask optional abort reason if session objectives remain unfinished
- return cleanly to terminal

## Scratchpad

V1 should include a persistent plain-text scratchpad pane.

Purpose:

- rough working
- note taking
- copying definitions or reminders
- storing temporary thoughts that should not be submitted as answers

Requirements:

- autosave locally
- separate from scored answer widgets
- no markdown parsing requirement

## Error States

The UI must expose failure modes clearly.

Examples:

- app-server disconnected
- failed to load database
- malformed structured response from agent
- unsupported rendering capability

Requirements:

- clear banner or modal
- recovery action when possible
- no silent failure

## Accessibility

V1 should support:

- full keyboard navigation
- monochrome-safe theme
- reduced-motion mode
- linear transcript fallback
- high-contrast focus indicators
