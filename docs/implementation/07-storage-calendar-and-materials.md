# Storage, Calendar, And Materials

## Goal

Persist enough local state that StudyOS remembers the student and can plan sensible sessions without relying on live integrations.

## Storage Stack

### Primary store

- SQLite

### Secondary local files

- user config file
- local deadline and timetable import files
- materials manifest files

## Database Tables

V1 should implement these tables or close equivalents.

### `concepts`

- `id`
- `course`
- `name`
- `prerequisite_ids`
- `tags`

### `concept_state`

- `concept_id`
- `mastery_estimate`
- `retrieval_strength`
- `last_seen_at`
- `last_success_at`
- `last_failure_at`
- `next_review_at`
- `stability_days`
- `ease_factor`

### `misconceptions`

- `id`
- `concept_id`
- `error_type`
- `description`
- `first_seen_at`
- `last_seen_at`
- `resolved_at`
- `evidence_count`

### `attempts`

- `id`
- `session_id`
- `concept_id`
- `question_type`
- `prompt_hash`
- `student_answer`
- `correctness`
- `latency_ms`
- `reasoning_quality`
- `feedback_summary`

### `sessions`

- `id`
- `started_at`
- `ended_at`
- `planned_minutes`
- `actual_minutes`
- `mode`
- `outcome_summary`
- `aborted_reason`

### `deadlines`

- `id`
- `source`
- `title`
- `due_at`
- `course`
- `weight`
- `notes`

### `resume_state`

- `session_id`
- `saved_at`
- `active_mode`
- `active_question_id`
- `focused_panel`
- `draft_payload`
- `scratchpad_text`

## Update Rules

### After successful retrieval

- increase retrieval strength
- reduce urgency if answer was strong and timely
- schedule review further out

### After failed retrieval

- increase urgency
- shorten next review interval
- log or increment misconception

### After method-poor success

- count correctness but reduce mastery gain
- prefer transfer or explanation follow-up later

## Local Calendar Storage

V1 uses local calendar-style data, not live integrations.

### Supported sources

- `deadlines.json`
- `timetable.json`
- manual in-app deadline entry later if needed

### V1 behavior

- read local deadlines at startup
- compute urgency score by due date and weight
- infer upcoming work pressure from local timetable and deadlines
- use that to bias session plan toward repair or exam drill mode

## Materials Storage

V1 materials support should stay lightweight.

### Supported V1 data

- local file paths
- material title
- course
- topic tags
- material type
- short extracted text snippets when available

### V1 capabilities

- search by keyword and topic
- find files linked to a concept
- surface examples or past-paper items by metadata

### Deferred

- OCR
- deep PDF layout parsing
- automatic semantic chunk extraction at scale

## File Layout Recommendation

Suggested local paths:

```text
.studyos/
    studyos.db
    config.toml
    deadlines.json
    timetable.json
    materials/
        manifest.json
```

## Privacy Rules

- study database stays local by default
- materials paths stay local
- raw notes or answers should not be sent remotely unless needed for the active turn
- logs should avoid leaking sensitive full-content history when summaries suffice
