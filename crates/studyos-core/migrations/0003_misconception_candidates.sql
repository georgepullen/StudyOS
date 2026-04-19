CREATE TABLE IF NOT EXISTS misconception_candidates (
    id TEXT PRIMARY KEY,
    concept_id TEXT NOT NULL,
    error_type TEXT NOT NULL,
    description TEXT NOT NULL,
    evidence TEXT NOT NULL DEFAULT '[]',
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    rationale TEXT NOT NULL DEFAULT '',
    evidence_count INTEGER NOT NULL DEFAULT 1,
    CHECK (status IN ('pending', 'graduated', 'rejected'))
);

CREATE TABLE IF NOT EXISTS misconception_decisions (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    decided_at TEXT NOT NULL,
    decided_by TEXT NOT NULL,
    rationale TEXT NOT NULL,
    FOREIGN KEY(candidate_id) REFERENCES misconception_candidates(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_misconception_candidates_lookup
    ON misconception_candidates (concept_id, error_type, description);

CREATE INDEX IF NOT EXISTS idx_misconception_candidates_status
    ON misconception_candidates (status, last_seen_at DESC);

CREATE INDEX IF NOT EXISTS idx_misconception_decisions_candidate
    ON misconception_decisions (candidate_id, decided_at DESC);
