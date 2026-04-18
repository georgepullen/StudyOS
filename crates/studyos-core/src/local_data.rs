use std::{fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

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
        let deadlines =
            load_json_file::<Vec<DeadlineEntry>>(&paths.deadlines_path)?.unwrap_or_default();
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
        self.deadlines.len()
    }
}

fn load_json_file<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}
