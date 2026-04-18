use std::path::Path;

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
    pub active_mode: String,
    pub active_question_id: Option<String>,
    pub focused_panel: String,
    pub draft_payload: String,
    pub scratchpad_text: String,
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
                SELECT session_id, active_mode, active_question_id, focused_panel, draft_payload, scratchpad_text
                FROM resume_state
                ORDER BY saved_at DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok(ResumeStateRecord {
                        session_id: row.get(0)?,
                        active_mode: row.get(1)?,
                        active_question_id: row.get(2)?,
                        focused_panel: row.get(3)?,
                        draft_payload: row.get(4)?,
                        scratchpad_text: row.get(5)?,
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
                session_id, saved_at, active_mode, active_question_id, focused_panel, draft_payload, scratchpad_text
            )
            VALUES (?1, datetime('now'), ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(session_id) DO UPDATE SET
                saved_at = excluded.saved_at,
                active_mode = excluded.active_mode,
                active_question_id = excluded.active_question_id,
                focused_panel = excluded.focused_panel,
                draft_payload = excluded.draft_payload,
                scratchpad_text = excluded.scratchpad_text
            ",
            params![
                record.session_id,
                record.active_mode,
                record.active_question_id,
                record.focused_panel,
                record.draft_payload,
                record.scratchpad_text,
            ],
        )?;

        Ok(())
    }

    fn count_query(&self, sql: &str) -> Result<usize> {
        let count = self
            .connection
            .query_row(sql, [], |row| row.get::<_, i64>(0))?;
        Ok(count as usize)
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
                saved_at TEXT NOT NULL,
                active_mode TEXT NOT NULL,
                active_question_id TEXT,
                focused_panel TEXT NOT NULL,
                draft_payload TEXT NOT NULL DEFAULT '',
                scratchpad_text TEXT NOT NULL DEFAULT ''
            );
            ",
        )?;

        self.seed_default_concepts()?;
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

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::{AppDatabase, ResumeStateRecord};

    fn temp_db_path() -> std::path::PathBuf {
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };

        env::temp_dir().join(format!("studyos-test-{}-{nanos}.db", std::process::id()))
    }

    #[test]
    fn database_bootstrap_seeds_initial_stats() {
        let path = temp_db_path();
        let database =
            AppDatabase::open(&path).unwrap_or_else(|err| panic!("database open failed: {err}"));
        let stats = database
            .stats()
            .unwrap_or_else(|err| panic!("stats query failed: {err}"));

        assert_eq!(stats.due_reviews, 0);
        assert_eq!(stats.upcoming_deadlines, 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn resume_state_round_trips() {
        let path = temp_db_path();
        let database =
            AppDatabase::open(&path).unwrap_or_else(|err| panic!("database open failed: {err}"));

        let record = ResumeStateRecord {
            session_id: "test-session".to_string(),
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
        assert_eq!(loaded.focused_panel, record.focused_panel);
        assert_eq!(loaded.scratchpad_text, record.scratchpad_text);

        let _ = fs::remove_file(path);
    }
}
