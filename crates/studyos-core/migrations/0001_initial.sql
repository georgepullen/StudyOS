CREATE TABLE IF NOT EXISTS concepts (
    id TEXT PRIMARY KEY,
    course TEXT NOT NULL,
    name TEXT NOT NULL,
    prerequisite_ids TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS concept_state (
    concept_id TEXT PRIMARY KEY,
    mastery_estimate REAL NOT NULL DEFAULT 0.0,
    retrieval_strength REAL NOT NULL DEFAULT 0.0,
    last_seen_at TEXT,
    last_success_at TEXT,
    last_failure_at TEXT,
    next_review_at TEXT,
    stability_days REAL NOT NULL DEFAULT 0.0,
    ease_factor REAL NOT NULL DEFAULT 2.5
);

CREATE TABLE IF NOT EXISTS misconceptions (
    id TEXT PRIMARY KEY,
    concept_id TEXT NOT NULL,
    error_type TEXT NOT NULL,
    description TEXT NOT NULL,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    resolved_at TEXT,
    evidence_count INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS attempts (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    concept_id TEXT NOT NULL,
    question_type TEXT NOT NULL,
    prompt_hash TEXT NOT NULL,
    student_answer TEXT NOT NULL,
    correctness TEXT NOT NULL,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    reasoning_quality TEXT NOT NULL DEFAULT 'unknown',
    feedback_summary TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    planned_minutes INTEGER NOT NULL,
    actual_minutes INTEGER,
    mode TEXT NOT NULL,
    outcome_summary TEXT NOT NULL DEFAULT '',
    aborted_reason TEXT
);

CREATE TABLE IF NOT EXISTS deadlines (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    title TEXT NOT NULL,
    due_at TEXT NOT NULL,
    course TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,
    notes TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS resume_state (
    session_id TEXT PRIMARY KEY,
    saved_at TEXT NOT NULL,
    active_mode TEXT NOT NULL,
    active_question_id TEXT,
    focused_panel TEXT NOT NULL,
    draft_payload TEXT NOT NULL DEFAULT '',
    scratchpad_text TEXT NOT NULL DEFAULT ''
);
