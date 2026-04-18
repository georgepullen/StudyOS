use std::{
    hash::{Hash, Hasher},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppStats {
    pub due_reviews: usize,
    pub upcoming_deadlines: usize,
    pub total_attempts: usize,
    pub total_sessions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeStateRecord {
    pub session_id: String,
    pub runtime_thread_id: Option<String>,
    pub active_mode: String,
    pub active_question_id: Option<String>,
    pub focused_panel: String,
    pub draft_payload: String,
    pub scratchpad_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub id: String,
    pub planned_minutes: u16,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptRecord {
    pub id: String,
    pub session_id: String,
    pub concept_id: String,
    pub question_type: String,
    pub prompt_hash: String,
    pub student_answer: String,
    pub correctness: String,
    pub latency_ms: i64,
    pub reasoning_quality: String,
    pub feedback_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MisconceptionInput {
    pub concept_id: String,
    pub error_type: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueReviewSummary {
    pub concept_id: String,
    pub concept_name: String,
    pub next_review_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MisconceptionSummary {
    pub concept_name: String,
    pub error_type: String,
    pub description: String,
    pub last_seen_at: String,
    pub evidence_count: usize,
}

pub struct AppDatabase {
    connection: Connection,
}

impl AppDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        let database = Self { connection };
        database.initialize_schema()?;
        Ok(database)
    }

    pub fn stats(&self) -> Result<AppStats> {
        let due_reviews = self.count_query(
            "SELECT COUNT(*) FROM concept_state WHERE next_review_at IS NOT NULL AND next_review_at <= datetime('now')",
        )?;
        let upcoming_deadlines = self.count_query(
            "SELECT COUNT(*) FROM deadlines WHERE due_at IS NOT NULL AND due_at <= datetime('now', '+14 day')",
        )?;
        let total_attempts = self.count_query("SELECT COUNT(*) FROM attempts")?;
        let total_sessions = self.count_query("SELECT COUNT(*) FROM sessions")?;

        Ok(AppStats {
            due_reviews,
            upcoming_deadlines,
            total_attempts,
            total_sessions,
        })
    }

    pub fn load_resume_state(&self) -> Result<Option<ResumeStateRecord>> {
        let record = self
            .connection
            .query_row(
                "
                SELECT session_id, runtime_thread_id, active_mode, active_question_id, focused_panel, draft_payload, scratchpad_text
                FROM resume_state
                ORDER BY saved_at DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok(ResumeStateRecord {
                        session_id: row.get(0)?,
                        runtime_thread_id: row.get(1)?,
                        active_mode: row.get(2)?,
                        active_question_id: row.get(3)?,
                        focused_panel: row.get(4)?,
                        draft_payload: row.get(5)?,
                        scratchpad_text: row.get(6)?,
                    })
                },
            )
            .optional()?;

        Ok(record)
    }

    pub fn save_resume_state(&self, record: &ResumeStateRecord) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO resume_state (
                session_id, runtime_thread_id, saved_at, active_mode, active_question_id, focused_panel, draft_payload, scratchpad_text
            )
            VALUES (?1, ?2, datetime('now'), ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(session_id) DO UPDATE SET
                runtime_thread_id = excluded.runtime_thread_id,
                saved_at = excluded.saved_at,
                active_mode = excluded.active_mode,
                active_question_id = excluded.active_question_id,
                focused_panel = excluded.focused_panel,
                draft_payload = excluded.draft_payload,
                scratchpad_text = excluded.scratchpad_text
            ",
            params![
                record.session_id,
                record.runtime_thread_id,
                record.active_mode,
                record.active_question_id,
                record.focused_panel,
                record.draft_payload,
                record.scratchpad_text,
            ],
        )?;

        Ok(())
    }

    pub fn start_session(&self, record: &SessionRecord) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO sessions (id, started_at, planned_minutes, mode)
            VALUES (?1, datetime('now'), ?2, ?3)
            ON CONFLICT(id) DO NOTHING
            ",
            params![record.id, record.planned_minutes, record.mode],
        )?;
        Ok(())
    }

    pub fn complete_session(
        &self,
        session_id: &str,
        actual_minutes: i64,
        outcome_summary: &str,
        aborted_reason: Option<&str>,
    ) -> Result<()> {
        self.connection.execute(
            "
            UPDATE sessions
            SET ended_at = datetime('now'),
                actual_minutes = ?2,
                outcome_summary = ?3,
                aborted_reason = ?4
            WHERE id = ?1
            ",
            params![session_id, actual_minutes, outcome_summary, aborted_reason],
        )?;
        Ok(())
    }

    pub fn record_attempt(
        &self,
        attempt: &AttemptRecord,
        misconception: Option<&MisconceptionInput>,
    ) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO attempts (
                id, session_id, concept_id, question_type, prompt_hash, student_answer,
                correctness, latency_ms, reasoning_quality, feedback_summary
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                attempt.id,
                attempt.session_id,
                attempt.concept_id,
                attempt.question_type,
                attempt.prompt_hash,
                attempt.student_answer,
                attempt.correctness,
                attempt.latency_ms,
                attempt.reasoning_quality,
                attempt.feedback_summary,
            ],
        )?;

        self.ensure_concept_state(&attempt.concept_id)?;
        self.update_concept_state(attempt)?;

        if let Some(misconception) = misconception {
            self.upsert_misconception(misconception)?;
        }

        Ok(())
    }

    pub fn resolve_concept_id(&self, candidates: &[String]) -> Result<Option<String>> {
        for candidate in candidates {
            let resolved = self
                .connection
                .query_row(
                    "
                    SELECT id
                    FROM concepts
                    WHERE lower(id) = lower(?1)
                       OR lower(name) = lower(?1)
                       OR lower(tags) LIKE '%' || lower(?1) || '%'
                    LIMIT 1
                    ",
                    params![candidate],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;

            if resolved.is_some() {
                return Ok(resolved);
            }
        }

        Ok(None)
    }

    pub fn list_due_reviews(&self, limit: usize) -> Result<Vec<DueReviewSummary>> {
        let mut statement = self.connection.prepare(
            "
            SELECT concepts.id, concepts.name, concept_state.next_review_at
            FROM concept_state
            INNER JOIN concepts ON concepts.id = concept_state.concept_id
            WHERE concept_state.next_review_at IS NOT NULL
            ORDER BY
                CASE
                    WHEN concept_state.next_review_at <= datetime('now') THEN 0
                    ELSE 1
                END,
                concept_state.next_review_at ASC
            LIMIT ?1
            ",
        )?;

        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(DueReviewSummary {
                concept_id: row.get(0)?,
                concept_name: row.get(1)?,
                next_review_at: row.get(2)?,
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    pub fn list_recent_misconceptions(&self, limit: usize) -> Result<Vec<MisconceptionSummary>> {
        let mut statement = self.connection.prepare(
            "
            SELECT concepts.name, misconceptions.error_type, misconceptions.description,
                   misconceptions.last_seen_at, misconceptions.evidence_count
            FROM misconceptions
            INNER JOIN concepts ON concepts.id = misconceptions.concept_id
            WHERE misconceptions.resolved_at IS NULL
            ORDER BY misconceptions.last_seen_at DESC
            LIMIT ?1
            ",
        )?;

        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(MisconceptionSummary {
                concept_name: row.get(0)?,
                error_type: row.get(1)?,
                description: row.get(2)?,
                last_seen_at: row.get(3)?,
                evidence_count: row.get::<_, i64>(4)? as usize,
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    fn count_query(&self, sql: &str) -> Result<usize> {
        let count = self
            .connection
            .query_row(sql, [], |row| row.get::<_, i64>(0))?;
        Ok(count as usize)
    }

    fn ensure_concept_state(&self, concept_id: &str) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO concept_state (concept_id)
            VALUES (?1)
            ON CONFLICT(concept_id) DO NOTHING
            ",
            params![concept_id],
        )?;
        Ok(())
    }

    fn update_concept_state(&self, attempt: &AttemptRecord) -> Result<()> {
        let (current_mastery, current_retrieval, current_stability, current_ease) =
            self.connection.query_row(
                "
                SELECT mastery_estimate, retrieval_strength, stability_days, ease_factor
                FROM concept_state
                WHERE concept_id = ?1
                ",
                params![attempt.concept_id],
                |row| {
                    Ok((
                        row.get::<_, f64>(0)?,
                        row.get::<_, f64>(1)?,
                        row.get::<_, f64>(2)?,
                        row.get::<_, f64>(3)?,
                    ))
                },
            )?;

        let (mastery_delta, retrieval_delta, stability_delta, ease_delta, review_modifier, success) =
            match (
                attempt.correctness.as_str(),
                attempt.reasoning_quality.as_str(),
            ) {
                ("correct", "strong") => (0.18, 0.22, 2.0, 0.06, "+5 day", true),
                ("correct", "adequate") => (0.12, 0.16, 1.2, 0.03, "+3 day", true),
                ("correct", _) => (0.07, 0.08, 0.6, 0.0, "+1 day", true),
                ("partial", "adequate") => (0.03, -0.02, 0.2, -0.04, "+12 hour", false),
                ("partial", _) => (0.01, -0.05, 0.0, -0.06, "+8 hour", false),
                (_, _) => (-0.08, -0.14, -0.4, -0.1, "+4 hour", false),
            };

        let mastery_estimate = clamp(current_mastery + mastery_delta, 0.0, 1.0);
        let retrieval_strength = clamp(current_retrieval + retrieval_delta, 0.0, 1.0);
        let stability_days = clamp(current_stability + stability_delta, 0.0, 60.0);
        let ease_factor = clamp(current_ease + ease_delta, 1.3, 3.0);
        let success_timestamp = if success {
            Some("datetime('now')")
        } else {
            None
        };
        let failure_timestamp = if success {
            None
        } else {
            Some("datetime('now')")
        };

        self.connection.execute(
            &format!(
                "
                UPDATE concept_state
                SET mastery_estimate = ?2,
                    retrieval_strength = ?3,
                    last_seen_at = datetime('now'),
                    last_success_at = {},
                    last_failure_at = {},
                    next_review_at = datetime('now', ?4),
                    stability_days = ?5,
                    ease_factor = ?6
                WHERE concept_id = ?1
                ",
                success_timestamp.unwrap_or("last_success_at"),
                failure_timestamp.unwrap_or("last_failure_at"),
            ),
            params![
                attempt.concept_id,
                mastery_estimate,
                retrieval_strength,
                review_modifier,
                stability_days,
                ease_factor,
            ],
        )?;

        Ok(())
    }

    fn upsert_misconception(&self, misconception: &MisconceptionInput) -> Result<()> {
        let existing = self
            .connection
            .query_row(
                "
                SELECT id
                FROM misconceptions
                WHERE concept_id = ?1
                  AND error_type = ?2
                  AND description = ?3
                  AND resolved_at IS NULL
                ORDER BY last_seen_at DESC
                LIMIT 1
                ",
                params![
                    misconception.concept_id,
                    misconception.error_type,
                    misconception.description
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(id) = existing {
            self.connection.execute(
                "
                UPDATE misconceptions
                SET last_seen_at = datetime('now'),
                    evidence_count = evidence_count + 1
                WHERE id = ?1
                ",
                params![id],
            )?;
        } else {
            self.connection.execute(
                "
                INSERT INTO misconceptions (
                    id, concept_id, error_type, description, first_seen_at, last_seen_at, resolved_at, evidence_count
                )
                VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'), NULL, 1)
                ",
                params![
                    make_record_id("misconception", &misconception.description),
                    misconception.concept_id,
                    misconception.error_type,
                    misconception.description,
                ],
            )?;
        }

        Ok(())
    }

    fn initialize_schema(&self) -> Result<()> {
        self.connection.execute_batch(
            "
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
                runtime_thread_id TEXT,
                saved_at TEXT NOT NULL,
                active_mode TEXT NOT NULL,
                active_question_id TEXT,
                focused_panel TEXT NOT NULL,
                draft_payload TEXT NOT NULL DEFAULT '',
                scratchpad_text TEXT NOT NULL DEFAULT ''
            );
            ",
        )?;

        self.migrate_resume_state()?;
        self.seed_default_concepts()?;
        Ok(())
    }

    fn migrate_resume_state(&self) -> Result<()> {
        let mut statement = self.connection.prepare("PRAGMA table_info(resume_state)")?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        let mut has_runtime_thread_id = false;

        for column in columns {
            if column? == "runtime_thread_id" {
                has_runtime_thread_id = true;
            }
        }

        if !has_runtime_thread_id {
            self.connection.execute(
                "ALTER TABLE resume_state ADD COLUMN runtime_thread_id TEXT",
                [],
            )?;
        }

        Ok(())
    }

    fn seed_default_concepts(&self) -> Result<()> {
        let concepts = [
            (
                "matrix_multiplication_dims",
                "Matrix Algebra & Linear Models",
                "Matrix multiplication dimensions",
                "[\"matrix_multiplication\"]",
            ),
            (
                "determinant_singularity",
                "Matrix Algebra & Linear Models",
                "Determinant zero implies singularity",
                "[\"determinant\", \"invertibility\"]",
            ),
            (
                "variance_definition",
                "Probability & Statistics for Scientists",
                "Variance as expected squared deviation",
                "[\"variance\", \"expectation\"]",
            ),
        ];

        for (id, course, name, tags) in concepts {
            self.connection.execute(
                "
                INSERT INTO concepts (id, course, name, tags)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(id) DO NOTHING
                ",
                params![id, course, name, tags],
            )?;
        }

        Ok(())
    }
}

fn make_record_id(prefix: &str, seed: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    seed.hash(&mut hasher);
    let seed_hash = hasher.finish();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{nanos:x}-{seed_hash:x}")
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::{AppDatabase, AttemptRecord, MisconceptionInput, ResumeStateRecord, SessionRecord};

    fn temp_db_dir() -> std::path::PathBuf {
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };

        let dir = env::temp_dir().join(format!("studyos-test-{}-{nanos}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap_or_else(|err| panic!("failed to create temp dir: {err}"));
        dir
    }

    #[test]
    fn database_bootstrap_seeds_initial_stats() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        {
            let database = AppDatabase::open(&path)
                .unwrap_or_else(|err| panic!("database open failed: {err}"));
            let stats = database
                .stats()
                .unwrap_or_else(|err| panic!("stats query failed: {err}"));

            assert_eq!(stats.due_reviews, 0);
            assert_eq!(stats.upcoming_deadlines, 0);
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resume_state_round_trips() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        {
            let database = AppDatabase::open(&path)
                .unwrap_or_else(|err| panic!("database open failed: {err}"));

            let record = ResumeStateRecord {
                session_id: "test-session".to_string(),
                runtime_thread_id: Some("runtime-thread".to_string()),
                active_mode: "Study".to_string(),
                active_question_id: Some("4".to_string()),
                focused_panel: "Scratchpad".to_string(),
                draft_payload: "draft = true".to_string(),
                scratchpad_text: "rough working".to_string(),
            };

            database
                .save_resume_state(&record)
                .unwrap_or_else(|err| panic!("resume save failed: {err}"));

            let loaded = database
                .load_resume_state()
                .unwrap_or_else(|err| panic!("resume load failed: {err}"))
                .unwrap_or_else(|| panic!("missing resume state"));

            assert_eq!(loaded.session_id, record.session_id);
            assert_eq!(loaded.runtime_thread_id, record.runtime_thread_id);
            assert_eq!(loaded.focused_panel, record.focused_panel);
            assert_eq!(loaded.scratchpad_text, record.scratchpad_text);
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn attempt_logging_updates_reviews_and_misconceptions() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        {
            let database = AppDatabase::open(&path)
                .unwrap_or_else(|err| panic!("database open failed: {err}"));

            database
                .start_session(&SessionRecord {
                    id: "session-1".to_string(),
                    planned_minutes: 45,
                    mode: "Study".to_string(),
                })
                .unwrap_or_else(|err| panic!("session start failed: {err}"));

            database
                .record_attempt(
                    &AttemptRecord {
                        id: "attempt-1".to_string(),
                        session_id: "session-1".to_string(),
                        concept_id: "matrix_multiplication_dims".to_string(),
                        question_type: "retrieval_response".to_string(),
                        prompt_hash: "abc123".to_string(),
                        student_answer: "rows and columns mismatched".to_string(),
                        correctness: "incorrect".to_string(),
                        latency_ms: 1200,
                        reasoning_quality: "missing".to_string(),
                        feedback_summary: "You mixed up inner and outer dimensions.".to_string(),
                    },
                    Some(&MisconceptionInput {
                        concept_id: "matrix_multiplication_dims".to_string(),
                        error_type: "conceptual_misunderstanding".to_string(),
                        description: "Confused inner and outer dimensions.".to_string(),
                    }),
                )
                .unwrap_or_else(|err| panic!("attempt record failed: {err}"));

            let reviews = database
                .list_due_reviews(5)
                .unwrap_or_else(|err| panic!("due review query failed: {err}"));
            let misconceptions = database
                .list_recent_misconceptions(5)
                .unwrap_or_else(|err| panic!("misconception query failed: {err}"));
            let stats = database
                .stats()
                .unwrap_or_else(|err| panic!("stats query failed: {err}"));

            assert_eq!(stats.total_attempts, 1);
            assert_eq!(stats.total_sessions, 1);
            assert!(!reviews.is_empty());
            assert_eq!(reviews[0].concept_id, "matrix_multiplication_dims");
            assert_eq!(misconceptions.len(), 1);
            assert_eq!(
                misconceptions[0].error_type,
                "conceptual_misunderstanding".to_string()
            );
        }

        let _ = fs::remove_dir_all(dir);
    }
}
