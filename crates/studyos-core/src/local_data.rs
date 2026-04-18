use std::{fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{AppPaths, CourseCatalog};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeadlineEntry {
    pub id: String,
    pub source: String,
    pub title: String,
    pub due_at: String,
    pub course: String,
    pub weight: f32,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimetableSlot {
    pub day: String,
    pub start: String,
    pub end: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimetableData {
    pub timezone: String,
    pub slots: Vec<TimetableSlot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialEntry {
    pub id: String,
    pub title: String,
    pub course: String,
    pub topic_tags: Vec<String>,
    pub material_type: String,
    pub path: String,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocalContext {
    pub deadlines: Vec<DeadlineEntry>,
    pub timetable: Option<TimetableData>,
    pub materials: Vec<MaterialEntry>,
    pub courses: CourseCatalog,
}

impl LocalContext {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        let deadlines = load_deadlines(&paths.deadlines_path)?;
        let timetable = load_json_file::<TimetableData>(&paths.timetable_path)?;
        let materials_manifest = paths.materials_dir.join("manifest.json");
        let materials =
            load_json_file::<Vec<MaterialEntry>>(&materials_manifest)?.unwrap_or_default();
        let courses = CourseCatalog::load(&paths.courses_dir)?;

        Ok(Self {
            deadlines,
            timetable,
            materials,
            courses,
        })
    }

    pub fn upcoming_deadline_count(&self) -> usize {
        let horizon = OffsetDateTime::now_utc() + Duration::days(14);
        self.deadlines
            .iter()
            .filter(|deadline| {
                OffsetDateTime::parse(&deadline.due_at, &Rfc3339)
                    .map(|due_at| due_at <= horizon)
                    .unwrap_or(true)
            })
            .count()
    }
}

pub fn load_deadlines(path: &Path) -> Result<Vec<DeadlineEntry>> {
    let mut deadlines = load_json_file::<Vec<DeadlineEntry>>(path)?.unwrap_or_default();
    deadlines.sort_by(|left, right| left.due_at.cmp(&right.due_at));
    Ok(deadlines)
}

pub fn save_deadlines(path: &Path, deadlines: &[DeadlineEntry]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut entries = deadlines.to_vec();
    entries.sort_by(|left, right| left.due_at.cmp(&right.due_at));
    fs::write(path, serde_json::to_string_pretty(&entries)?)?;
    Ok(())
}

pub fn upsert_deadline(path: &Path, entry: DeadlineEntry) -> Result<Vec<DeadlineEntry>> {
    let mut deadlines = load_deadlines(path)?;
    if let Some(existing) = deadlines
        .iter_mut()
        .find(|deadline| deadline.id == entry.id)
    {
        *existing = entry;
    } else {
        deadlines.push(entry);
    }
    save_deadlines(path, &deadlines)?;
    Ok(deadlines)
}

fn load_json_file<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_json_path() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "studyos-local-data-{}-{}.json",
            std::process::id(),
            OffsetDateTime::now_utc().unix_timestamp_nanos()
        ));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn deadline_round_trip_sorts_by_due_time() {
        let path = temp_json_path();
        let deadlines = vec![
            DeadlineEntry {
                id: "later".to_string(),
                source: "manual".to_string(),
                title: "Later".to_string(),
                due_at: "2026-05-10T12:00:00Z".to_string(),
                course: "Matrix Algebra & Linear Models".to_string(),
                weight: 0.2,
                notes: String::new(),
            },
            DeadlineEntry {
                id: "earlier".to_string(),
                source: "manual".to_string(),
                title: "Earlier".to_string(),
                due_at: "2026-05-01T12:00:00Z".to_string(),
                course: "Probability & Statistics for Scientists".to_string(),
                weight: 0.4,
                notes: String::new(),
            },
        ];

        save_deadlines(&path, &deadlines)
            .unwrap_or_else(|err| panic!("save deadlines failed: {err}"));
        let loaded =
            load_deadlines(&path).unwrap_or_else(|err| panic!("load deadlines failed: {err}"));

        assert_eq!(loaded[0].id, "earlier");
        assert_eq!(loaded[1].id, "later");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn upcoming_deadline_count_ignores_far_future_entries() {
        let now = OffsetDateTime::now_utc();
        let context = LocalContext {
            deadlines: vec![
                DeadlineEntry {
                    id: "soon".to_string(),
                    source: "manual".to_string(),
                    title: "Soon".to_string(),
                    due_at: (now + Duration::days(7))
                        .format(&Rfc3339)
                        .unwrap_or_else(|err| panic!("format failed: {err}")),
                    course: "Matrix Algebra & Linear Models".to_string(),
                    weight: 0.2,
                    notes: String::new(),
                },
                DeadlineEntry {
                    id: "later".to_string(),
                    source: "manual".to_string(),
                    title: "Later".to_string(),
                    due_at: (now + Duration::days(30))
                        .format(&Rfc3339)
                        .unwrap_or_else(|err| panic!("format failed: {err}")),
                    course: "Probability & Statistics for Scientists".to_string(),
                    weight: 0.4,
                    notes: String::new(),
                },
            ],
            timetable: None,
            materials: Vec::new(),
            courses: CourseCatalog::default(),
        };

        assert_eq!(context.upcoming_deadline_count(), 1);
    }
}
