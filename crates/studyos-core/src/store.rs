use std::{
    hash::{Hash, Hasher},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, anyhow};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::SessionRecapSummary;

const LATEST_SCHEMA_VERSION: i64 = 3;
const META_SCHEMA_VERSION_KEY: &str = "schema_version";

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../migrations/0001_initial.sql")),
    (
        2,
        include_str!("../migrations/0002_resume_thread_and_recap.sql"),
    ),
    (
        3,
        include_str!("../migrations/0003_misconception_candidates.sql"),
    ),
];

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
pub struct AttemptAuditRecord {
    pub id: String,
    pub session_id: String,
    pub concept_id: String,
    pub question_type: String,
    pub correctness: String,
    pub reasoning_quality: String,
    pub latency_ms: i64,
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
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecapRecord {
    pub session_id: String,
    pub recap: SessionRecapSummary,
}

#[derive(Debug, Clone, Copy)]
struct ConceptStateSnapshot {
    mastery_estimate: f64,
    retrieval_strength: f64,
    stability_days: f64,
    ease_factor: f64,
}

#[derive(Debug, Clone, Copy)]
struct ConceptStateTransition {
    next_state: ConceptStateSnapshot,
    review_modifier: &'static str,
    success: bool,
}

#[derive(Debug)]
pub struct AppDatabase {
    connection: Connection,
}

impl AppDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
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

    pub fn save_session_recap(&self, record: &SessionRecapRecord) -> Result<()> {
        self.connection.execute(
            "
            UPDATE sessions
            SET outcome_summary = ?2,
                recap_payload = ?3
            WHERE id = ?1
            ",
            params![
                record.session_id,
                record.recap.outcome_summary,
                serde_json::to_string(&record.recap)?,
            ],
        )?;
        Ok(())
    }

    pub fn record_attempt(
        &self,
        attempt: &AttemptRecord,
        misconception: Option<&MisconceptionInput>,
    ) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
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

        ensure_concept_state_in(&transaction, &attempt.concept_id)?;
        update_concept_state_in(&transaction, attempt)?;

        if let Some(misconception) = misconception {
            stage_misconception_candidate_in(&transaction, attempt.id.as_str(), misconception)?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn list_attempts_for_session(&self, session_id: &str) -> Result<Vec<AttemptAuditRecord>> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, session_id, concept_id, question_type, correctness,
                   reasoning_quality, latency_ms, feedback_summary
            FROM attempts
            WHERE session_id = ?1
            ORDER BY rowid ASC
            ",
        )?;

        let rows = statement.query_map(params![session_id], |row| {
            Ok(AttemptAuditRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                concept_id: row.get(2)?,
                question_type: row.get(3)?,
                correctness: row.get(4)?,
                reasoning_quality: row.get(5)?,
                latency_ms: row.get(6)?,
                feedback_summary: row.get(7)?,
            })
        })?;

        let mut attempts = Vec::new();
        for row in rows {
            attempts.push(row?);
        }
        Ok(attempts)
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
                status: "confirmed".to_string(),
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    pub fn list_recent_repair_signals(&self, limit: usize) -> Result<Vec<MisconceptionSummary>> {
        let mut statement = self.connection.prepare(
            "
            SELECT concept_name, error_type, description, last_seen_at, evidence_count, status
            FROM (
                SELECT concepts.name AS concept_name,
                       misconceptions.error_type AS error_type,
                       misconceptions.description AS description,
                       misconceptions.last_seen_at AS last_seen_at,
                       misconceptions.evidence_count AS evidence_count,
                       'confirmed' AS status
                FROM misconceptions
                INNER JOIN concepts ON concepts.id = misconceptions.concept_id
                WHERE misconceptions.resolved_at IS NULL
                UNION ALL
                SELECT concepts.name AS concept_name,
                       misconception_candidates.error_type AS error_type,
                       misconception_candidates.description AS description,
                       misconception_candidates.last_seen_at AS last_seen_at,
                       misconception_candidates.evidence_count AS evidence_count,
                       'candidate' AS status
                FROM misconception_candidates
                INNER JOIN concepts ON concepts.id = misconception_candidates.concept_id
                WHERE misconception_candidates.status = 'pending'
            )
            ORDER BY last_seen_at DESC
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
                status: row.get(5)?,
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    pub fn latest_session_recap(&self) -> Result<Option<SessionRecapSummary>> {
        let recap = self
            .connection
            .query_row(
                "
                SELECT recap_payload
                FROM sessions
                WHERE recap_payload IS NOT NULL AND recap_payload != ''
                ORDER BY started_at DESC
                LIMIT 1
                ",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        recap
            .map(|raw| serde_json::from_str::<SessionRecapSummary>(&raw).map_err(Into::into))
            .transpose()
    }

    fn initialize_schema(&self) -> Result<()> {
        self.ensure_meta_table()?;
        let current_version = self.detect_schema_version()?;
        if current_version > LATEST_SCHEMA_VERSION {
            return Err(anyhow!(
                "database schema version {current_version} is newer than supported version {LATEST_SCHEMA_VERSION}"
            ));
        }

        for (version, sql) in MIGRATIONS {
            if *version > current_version {
                self.connection.execute_batch(sql)?;
                self.set_schema_version(*version)?;
            }
        }

        self.seed_default_concepts()?;
        Ok(())
    }

    fn ensure_meta_table(&self) -> Result<()> {
        self.connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            ",
        )?;
        Ok(())
    }

    fn detect_schema_version(&self) -> Result<i64> {
        if let Some(version) = self
            .connection
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![META_SCHEMA_VERSION_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            return version
                .parse::<i64>()
                .map_err(|error| anyhow!("invalid schema version `{version}`: {error}"));
        }

        if !self.table_exists("sessions")? {
            return Ok(0);
        }

        if self.column_exists("sessions", "recap_payload")?
            && self.column_exists("resume_state", "runtime_thread_id")?
        {
            if self.table_exists("misconception_candidates")?
                && self.table_exists("misconception_decisions")?
            {
                self.set_schema_version(LATEST_SCHEMA_VERSION)?;
                return Ok(LATEST_SCHEMA_VERSION);
            }
            self.set_schema_version(2)?;
            return Ok(2);
        }

        Ok(1)
    }

    fn set_schema_version(&self, version: i64) -> Result<()> {
        self.connection.execute(
            "
            INSERT INTO meta (key, value)
            VALUES (?1, ?2)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
            ",
            params![META_SCHEMA_VERSION_KEY, version.to_string()],
        )?;
        Ok(())
    }

    fn table_exists(&self, name: &str) -> Result<bool> {
        let exists = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![name],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(exists == 1)
    }

    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        let pragma = format!("PRAGMA table_info({table})");
        let mut statement = self.connection.prepare(&pragma)?;
        let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
        for entry in columns {
            if entry? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn count_query(&self, sql: &str) -> Result<usize> {
        let count = self
            .connection
            .query_row(sql, [], |row| row.get::<_, i64>(0))?;
        Ok(count as usize)
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

#[derive(Debug, Clone)]
struct MisconceptionCandidateRecord {
    id: String,
    evidence: Vec<String>,
    evidence_count: usize,
    status: String,
}

fn ensure_concept_state_in(connection: &Connection, concept_id: &str) -> Result<()> {
    connection.execute(
        "
        INSERT INTO concept_state (concept_id)
        VALUES (?1)
        ON CONFLICT(concept_id) DO NOTHING
        ",
        params![concept_id],
    )?;
    Ok(())
}

fn concept_state_for_in(connection: &Connection, concept_id: &str) -> Result<ConceptStateSnapshot> {
    connection
        .query_row(
            "
        SELECT mastery_estimate, retrieval_strength, stability_days, ease_factor
        FROM concept_state
        WHERE concept_id = ?1
        ",
            params![concept_id],
            |row| {
                Ok(ConceptStateSnapshot {
                    mastery_estimate: row.get(0)?,
                    retrieval_strength: row.get(1)?,
                    stability_days: row.get(2)?,
                    ease_factor: row.get(3)?,
                })
            },
        )
        .map_err(Into::into)
}

fn update_concept_state_in(connection: &Connection, attempt: &AttemptRecord) -> Result<()> {
    let current = concept_state_for_in(connection, &attempt.concept_id)?;
    let transition = concept_state_after_attempt(
        current,
        attempt.correctness.as_str(),
        attempt.reasoning_quality.as_str(),
    );
    let success_timestamp = if transition.success {
        Some("datetime('now')")
    } else {
        None
    };
    let failure_timestamp = if transition.success {
        None
    } else {
        Some("datetime('now')")
    };

    connection.execute(
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
            transition.next_state.mastery_estimate,
            transition.next_state.retrieval_strength,
            transition.review_modifier,
            transition.next_state.stability_days,
            transition.next_state.ease_factor,
        ],
    )?;

    Ok(())
}

fn stage_misconception_candidate_in(
    transaction: &Transaction<'_>,
    attempt_id: &str,
    misconception: &MisconceptionInput,
) -> Result<()> {
    let existing = transaction
        .query_row(
            "
            SELECT id, evidence, evidence_count, status
            FROM misconception_candidates
            WHERE concept_id = ?1
              AND error_type = ?2
              AND description = ?3
            ORDER BY last_seen_at DESC
            LIMIT ?4
            ",
            params![
                misconception.concept_id,
                misconception.error_type,
                misconception.description,
                1_i64
            ],
            |row| {
                let raw_evidence = row.get::<_, String>(1)?;
                Ok(MisconceptionCandidateRecord {
                    id: row.get(0)?,
                    evidence: serde_json::from_str::<Vec<String>>(&raw_evidence)
                        .unwrap_or_default(),
                    evidence_count: row.get::<_, i64>(2)? as usize,
                    status: row.get(3)?,
                })
            },
        )
        .optional()?;

    match existing {
        Some(mut candidate) => {
            if !candidate.evidence.iter().any(|item| item == attempt_id) {
                candidate.evidence.push(attempt_id.to_string());
            }
            let next_count = candidate.evidence.len().max(candidate.evidence_count);
            let next_status = if candidate.status == "graduated" {
                "graduated"
            } else {
                "pending"
            };
            transaction.execute(
                "
                UPDATE misconception_candidates
                SET evidence = ?2,
                    evidence_count = ?3,
                    last_seen_at = datetime('now'),
                    status = ?4,
                    rationale = CASE WHEN ?4 = 'pending' THEN '' ELSE rationale END
                WHERE id = ?1
                ",
                params![
                    candidate.id,
                    serde_json::to_string(&candidate.evidence)?,
                    next_count as i64,
                    next_status,
                ],
            )?;

            if candidate.status == "graduated" {
                sync_graduated_misconception_in(transaction, misconception, next_count)?;
            } else if next_count >= 3 {
                graduate_misconception_candidate_in(
                    transaction,
                    &candidate.id,
                    misconception,
                    next_count,
                )?;
            }
        }
        None => {
            transaction.execute(
                "
                INSERT INTO misconception_candidates (
                    id, concept_id, error_type, description, evidence, first_seen_at, last_seen_at, status, rationale, evidence_count
                )
                VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'), 'pending', '', 1)
                ",
                params![
                    make_record_id("misconception-candidate", &misconception.description),
                    misconception.concept_id,
                    misconception.error_type,
                    misconception.description,
                    serde_json::to_string(&vec![attempt_id.to_string()])?,
                ],
            )?;
        }
    }

    Ok(())
}

fn graduate_misconception_candidate_in(
    transaction: &Transaction<'_>,
    candidate_id: &str,
    misconception: &MisconceptionInput,
    evidence_count: usize,
) -> Result<()> {
    let rationale = "auto-threshold after 3 corroborating attempts";
    transaction.execute(
        "
        UPDATE misconception_candidates
        SET status = 'graduated',
            rationale = ?2,
            last_seen_at = datetime('now')
        WHERE id = ?1
        ",
        params![candidate_id, rationale],
    )?;
    transaction.execute(
        "
        INSERT INTO misconception_decisions (id, candidate_id, decided_at, decided_by, rationale)
        VALUES (?1, ?2, datetime('now'), 'auto_threshold', ?3)
        ",
        params![
            make_record_id("misconception-decision", candidate_id),
            candidate_id,
            rationale,
        ],
    )?;
    sync_graduated_misconception_in(transaction, misconception, evidence_count)?;
    Ok(())
}

fn sync_graduated_misconception_in(
    connection: &Connection,
    misconception: &MisconceptionInput,
    evidence_count: usize,
) -> Result<()> {
    let existing = connection
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
        connection.execute(
            "
            UPDATE misconceptions
            SET last_seen_at = datetime('now'),
                evidence_count = ?2
            WHERE id = ?1
            ",
            params![id, evidence_count as i64],
        )?;
    } else {
        connection.execute(
            "
            INSERT INTO misconceptions (
                id, concept_id, error_type, description, first_seen_at, last_seen_at, resolved_at, evidence_count
            )
            VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'), NULL, ?5)
            ",
            params![
                make_record_id("misconception", &misconception.description),
                misconception.concept_id,
                misconception.error_type,
                misconception.description,
                evidence_count as i64,
            ],
        )?;
    }

    Ok(())
}

fn concept_state_after_attempt(
    current: ConceptStateSnapshot,
    correctness: &str,
    reasoning_quality: &str,
) -> ConceptStateTransition {
    let (mastery_delta, retrieval_delta, stability_delta, ease_delta, review_modifier, success) =
        match (correctness, reasoning_quality) {
            ("correct", "strong") => (0.18, 0.22, 2.0, 0.06, "+5 day", true),
            ("correct", "adequate") => (0.12, 0.16, 1.2, 0.03, "+3 day", true),
            ("correct", _) => (0.07, 0.08, 0.6, 0.0, "+1 day", true),
            ("partial", "adequate") => (0.03, -0.02, 0.2, -0.04, "+12 hour", false),
            ("partial", _) => (0.01, -0.05, 0.0, -0.06, "+8 hour", false),
            _ => (-0.08, -0.14, -0.4, -0.1, "+4 hour", false),
        };

    ConceptStateTransition {
        next_state: ConceptStateSnapshot {
            mastery_estimate: clamp(current.mastery_estimate + mastery_delta, 0.0, 1.0),
            retrieval_strength: clamp(current.retrieval_strength + retrieval_delta, 0.0, 1.0),
            stability_days: clamp(current.stability_days + stability_delta, 0.0, 60.0),
            ease_factor: clamp(current.ease_factor + ease_delta, 1.3, 3.0),
        },
        review_modifier,
        success,
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

    use proptest::prelude::*;
    use rusqlite::{Connection, params};

    use crate::SessionRecapSummary;

    use super::{
        AppDatabase, AttemptRecord, ConceptStateSnapshot, LATEST_SCHEMA_VERSION,
        META_SCHEMA_VERSION_KEY, MisconceptionInput, ResumeStateRecord, SessionRecapRecord,
        SessionRecord, concept_state_after_attempt,
    };

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

    fn attempt_case_strategy() -> impl Strategy<Value = (&'static str, &'static str)> {
        prop_oneof![
            Just(("correct", "strong")),
            Just(("correct", "adequate")),
            Just(("correct", "weak")),
            Just(("partial", "adequate")),
            Just(("partial", "missing")),
            Just(("incorrect", "missing")),
        ]
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
                draft_payload: "{\"draft\":true}".to_string(),
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
            let repair_signals = database
                .list_recent_repair_signals(5)
                .unwrap_or_else(|err| panic!("repair signal query failed: {err}"));
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
            assert_eq!(repair_signals.len(), 1);
            assert_eq!(repair_signals[0].status, "candidate".to_string());
            assert_eq!(misconceptions.len(), 0);
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn session_recap_round_trips() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        {
            let database = AppDatabase::open(&path)
                .unwrap_or_else(|err| panic!("database open failed: {err}"));

            database
                .start_session(&SessionRecord {
                    id: "session-recap".to_string(),
                    planned_minutes: 30,
                    mode: "Study".to_string(),
                })
                .unwrap_or_else(|err| panic!("session start failed: {err}"));

            let recap = SessionRecapSummary {
                outcome_summary: "Recovered the matrix product rule.".to_string(),
                demonstrated_concepts: vec!["Matrix multiplication dimensions".to_string()],
                weak_concepts: vec!["Explaining why rows dot beta".to_string()],
                next_review_items: vec!["Revisit matrix-vector products tomorrow".to_string()],
                unfinished_objectives: vec![
                    "Explain why each entry of X beta is a row dot product.".to_string(),
                ],
            };

            database
                .save_session_recap(&SessionRecapRecord {
                    session_id: "session-recap".to_string(),
                    recap: recap.clone(),
                })
                .unwrap_or_else(|err| panic!("save recap failed: {err}"));

            let loaded = database
                .latest_session_recap()
                .unwrap_or_else(|err| panic!("load recap failed: {err}"))
                .unwrap_or_else(|| panic!("missing recap"));

            assert_eq!(loaded, recap);
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn repeated_identical_misconception_stays_candidate_until_threshold() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        let database =
            AppDatabase::open(&path).unwrap_or_else(|err| panic!("database open failed: {err}"));
        database
            .start_session(&SessionRecord {
                id: "session-1".to_string(),
                planned_minutes: 30,
                mode: "Study".to_string(),
            })
            .unwrap_or_else(|err| panic!("session start failed: {err}"));

        for index in 0..2 {
            database
                .record_attempt(
                    &AttemptRecord {
                        id: format!("attempt-{index}"),
                        session_id: "session-1".to_string(),
                        concept_id: "matrix_multiplication_dims".to_string(),
                        question_type: "retrieval_response".to_string(),
                        prompt_hash: format!("hash-{index}"),
                        student_answer: "wrong".to_string(),
                        correctness: "incorrect".to_string(),
                        latency_ms: 500,
                        reasoning_quality: "missing".to_string(),
                        feedback_summary: "Still confused.".to_string(),
                    },
                    Some(&MisconceptionInput {
                        concept_id: "matrix_multiplication_dims".to_string(),
                        error_type: "conceptual_misunderstanding".to_string(),
                        description: "Confused inner and outer dimensions.".to_string(),
                    }),
                )
                .unwrap_or_else(|err| panic!("attempt record failed: {err}"));
        }

        let repair_signals = database
            .list_recent_repair_signals(5)
            .unwrap_or_else(|err| panic!("repair signal query failed: {err}"));
        let misconceptions = database
            .list_recent_misconceptions(5)
            .unwrap_or_else(|err| panic!("misconception query failed: {err}"));

        assert_eq!(repair_signals.len(), 1);
        assert_eq!(repair_signals[0].status, "candidate".to_string());
        assert_eq!(repair_signals[0].evidence_count, 2);
        assert_eq!(misconceptions.len(), 0);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn repeated_identical_misconception_graduates_after_threshold() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        let database =
            AppDatabase::open(&path).unwrap_or_else(|err| panic!("database open failed: {err}"));
        database
            .start_session(&SessionRecord {
                id: "session-1".to_string(),
                planned_minutes: 30,
                mode: "Study".to_string(),
            })
            .unwrap_or_else(|err| panic!("session start failed: {err}"));

        for index in 0..3 {
            database
                .record_attempt(
                    &AttemptRecord {
                        id: format!("attempt-{index}"),
                        session_id: "session-1".to_string(),
                        concept_id: "matrix_multiplication_dims".to_string(),
                        question_type: "retrieval_response".to_string(),
                        prompt_hash: format!("hash-{index}"),
                        student_answer: "wrong".to_string(),
                        correctness: "incorrect".to_string(),
                        latency_ms: 500,
                        reasoning_quality: "missing".to_string(),
                        feedback_summary: "Still confused.".to_string(),
                    },
                    Some(&MisconceptionInput {
                        concept_id: "matrix_multiplication_dims".to_string(),
                        error_type: "conceptual_misunderstanding".to_string(),
                        description: "Confused inner and outer dimensions.".to_string(),
                    }),
                )
                .unwrap_or_else(|err| panic!("attempt record failed: {err}"));
        }

        let repair_signals = database
            .list_recent_repair_signals(5)
            .unwrap_or_else(|err| panic!("repair signal query failed: {err}"));
        let misconceptions = database
            .list_recent_misconceptions(5)
            .unwrap_or_else(|err| panic!("misconception query failed: {err}"));
        let decision_count = database
            .connection
            .query_row("SELECT COUNT(*) FROM misconception_decisions", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or_else(|err| panic!("decision count failed: {err}"));

        assert_eq!(repair_signals.len(), 1);
        assert_eq!(repair_signals[0].status, "confirmed".to_string());
        assert_eq!(repair_signals[0].evidence_count, 3);
        assert_eq!(misconceptions.len(), 1);
        assert_eq!(misconceptions[0].evidence_count, 3);
        assert_eq!(decision_count, 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn schema_version_refuses_newer_db() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        let connection =
            Connection::open(&path).unwrap_or_else(|err| panic!("sqlite open failed: {err}"));
        connection
            .execute_batch(
                "
                CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                INSERT INTO meta (key, value) VALUES ('schema_version', '999');
                ",
            )
            .unwrap_or_else(|err| panic!("meta seed failed: {err}"));

        let error = AppDatabase::open(&path).expect_err("newer schema should be rejected");
        assert!(error.to_string().contains("newer than supported"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_schema_upgrades_forward() {
        let dir = temp_db_dir();
        let path = dir.join("studyos.db");
        let connection =
            Connection::open(&path).unwrap_or_else(|err| panic!("sqlite open failed: {err}"));
        connection
            .execute_batch(include_str!("../migrations/0001_initial.sql"))
            .unwrap_or_else(|err| panic!("initial migration seed failed: {err}"));

        let database =
            AppDatabase::open(&path).unwrap_or_else(|err| panic!("database open failed: {err}"));
        let loaded = database
            .load_resume_state()
            .unwrap_or_else(|err| panic!("resume load failed: {err}"));
        assert!(loaded.is_none());
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT value FROM meta WHERE key = ?1",
                    params![META_SCHEMA_VERSION_KEY],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_else(|err| panic!("schema version read failed: {err}")),
            LATEST_SCHEMA_VERSION.to_string()
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn correct_attempts_raise_mastery_and_incorrect_attempts_lower_it() {
        let mut state = ConceptStateSnapshot {
            mastery_estimate: 0.5,
            retrieval_strength: 0.5,
            stability_days: 3.0,
            ease_factor: 2.5,
        };
        for _ in 0..8 {
            state = concept_state_after_attempt(state, "correct", "strong").next_state;
        }
        assert!(state.mastery_estimate > 0.9);

        for _ in 0..8 {
            state = concept_state_after_attempt(state, "incorrect", "missing").next_state;
        }
        assert!(state.mastery_estimate < 0.5);
    }

    proptest! {
        #[test]
        fn mastery_retrieval_and_ease_stay_in_range(sequence in prop::collection::vec(attempt_case_strategy(), 1..64)) {
            let mut state = ConceptStateSnapshot {
                mastery_estimate: 0.0,
                retrieval_strength: 0.0,
                stability_days: 0.0,
                ease_factor: 2.5,
            };

            for (correctness, reasoning_quality) in sequence {
                state = concept_state_after_attempt(state, correctness, reasoning_quality).next_state;
                prop_assert!((0.0..=1.0).contains(&state.mastery_estimate));
                prop_assert!((0.0..=1.0).contains(&state.retrieval_strength));
                prop_assert!((0.0..=60.0).contains(&state.stability_days));
                prop_assert!((1.3..=3.0).contains(&state.ease_factor));
            }
        }
    }
}
