ALTER TABLE resume_state ADD COLUMN active_course TEXT;

CREATE TABLE IF NOT EXISTS course_runtime_threads (
    course TEXT PRIMARY KEY,
    runtime_thread_id TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
