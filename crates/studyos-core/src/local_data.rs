use std::{fs, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use time::{Duration, OffsetDateTime, Weekday, format_description::well_known::Rfc3339};

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
        let timetable = load_timetable(&paths.timetable_path)?;
        let materials_manifest = paths.materials_dir.join("manifest.json");
        let materials = load_materials(&materials_manifest)?;
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

    pub fn search_materials(
        &self,
        course_filter: Option<&str>,
        terms: &[String],
        limit: usize,
    ) -> Vec<MaterialEntry> {
        let normalized_terms = terms
            .iter()
            .map(|term| term.trim().to_lowercase())
            .filter(|term| !term.is_empty())
            .collect::<Vec<_>>();

        let mut scored = self
            .materials
            .iter()
            .filter_map(|entry| {
                let course_matches = course_filter
                    .map(|course| entry.course.eq_ignore_ascii_case(course))
                    .unwrap_or(true);
                if !course_matches {
                    return None;
                }

                let haystack = format!(
                    "{} {} {} {}",
                    entry.title,
                    entry.snippet,
                    entry.material_type,
                    entry.topic_tags.join(" ")
                )
                .to_lowercase();

                let mut score = if course_filter.is_some() { 2 } else { 0 };
                if normalized_terms.is_empty() {
                    score += 1;
                } else {
                    for term in &normalized_terms {
                        if haystack.contains(term) {
                            score += 2;
                        }
                    }
                }

                (score > 0).then(|| (score, entry.clone()))
            })
            .collect::<Vec<_>>();

        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.title.cmp(&right.1.title))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry)
            .collect()
    }

    pub fn next_timetable_slots(&self, limit: usize) -> Vec<TimetableSlot> {
        let Some(timetable) = &self.timetable else {
            return Vec::new();
        };

        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let today_index = weekday_index(now.weekday());

        let mut slots = timetable
            .slots
            .iter()
            .filter_map(|slot| {
                let slot_index = weekday_index(parse_weekday(&slot.day)?);
                let day_distance = (slot_index + 7 - today_index) % 7;
                Some((day_distance, slot.start.clone(), slot.clone()))
            })
            .collect::<Vec<_>>();

        slots.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        slots
            .into_iter()
            .take(limit)
            .map(|(_, _, slot)| slot)
            .collect()
    }

    pub fn today_timetable_slots(&self) -> Vec<TimetableSlot> {
        let Some(timetable) = &self.timetable else {
            return Vec::new();
        };

        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        timetable.slots_for_weekday(now.weekday())
    }
}

impl TimetableData {
    pub fn slots_for_weekday(&self, weekday: Weekday) -> Vec<TimetableSlot> {
        let target = weekday_index(weekday);
        let mut slots = self
            .slots
            .iter()
            .filter(|slot| {
                parse_weekday(&slot.day)
                    .map(|day| weekday_index(day) == target)
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>();
        slots.sort_by(|left, right| left.start.cmp(&right.start));
        slots
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

pub fn load_materials(path: &Path) -> Result<Vec<MaterialEntry>> {
    let mut materials = load_json_file::<Vec<MaterialEntry>>(path)?.unwrap_or_default();
    materials.sort_by(|left, right| left.title.cmp(&right.title));
    Ok(materials)
}

pub fn load_timetable(path: &Path) -> Result<Option<TimetableData>> {
    let mut timetable = load_json_file::<TimetableData>(path)?;
    if let Some(data) = &mut timetable {
        sort_timetable_slots(&mut data.slots);
    }
    Ok(timetable)
}

pub fn save_timetable(path: &Path, timetable: &TimetableData) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut data = timetable.clone();
    sort_timetable_slots(&mut data.slots);
    fs::write(path, serde_json::to_string_pretty(&data)?)?;
    Ok(())
}

pub fn append_timetable_slot(
    path: &Path,
    timezone: String,
    slot: TimetableSlot,
) -> Result<TimetableData> {
    let mut timetable = load_timetable(path)?.unwrap_or(TimetableData {
        timezone: timezone.clone(),
        slots: Vec::new(),
    });
    if timetable.timezone.trim().is_empty() {
        timetable.timezone = timezone;
    }
    timetable.slots.push(slot);
    save_timetable(path, &timetable)?;
    Ok(timetable)
}

fn parse_weekday(day: &str) -> Option<Weekday> {
    match day.trim().to_ascii_lowercase().as_str() {
        "monday" => Some(Weekday::Monday),
        "tuesday" => Some(Weekday::Tuesday),
        "wednesday" => Some(Weekday::Wednesday),
        "thursday" => Some(Weekday::Thursday),
        "friday" => Some(Weekday::Friday),
        "saturday" => Some(Weekday::Saturday),
        "sunday" => Some(Weekday::Sunday),
        _ => None,
    }
}

fn weekday_index(day: Weekday) -> u8 {
    match day {
        Weekday::Monday => 0,
        Weekday::Tuesday => 1,
        Weekday::Wednesday => 2,
        Weekday::Thursday => 3,
        Weekday::Friday => 4,
        Weekday::Saturday => 5,
        Weekday::Sunday => 6,
    }
}

fn sort_timetable_slots(slots: &mut [TimetableSlot]) {
    slots.sort_by(|left, right| {
        let left_day = parse_weekday(&left.day)
            .map(weekday_index)
            .unwrap_or(u8::MAX);
        let right_day = parse_weekday(&right.day)
            .map(weekday_index)
            .unwrap_or(u8::MAX);
        left_day
            .cmp(&right_day)
            .then_with(|| left.start.cmp(&right.start))
            .then_with(|| left.title.cmp(&right.title))
    });
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
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_json_path() -> std::path::PathBuf {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "studyos-local-data-{}-{}-{}.json",
            std::process::id(),
            counter,
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

    #[test]
    fn search_materials_prefers_course_and_term_matches() {
        let context = LocalContext {
            deadlines: Vec::new(),
            timetable: None,
            materials: vec![
                MaterialEntry {
                    id: "worksheet".to_string(),
                    title: "Matrix Multiplication Worksheet".to_string(),
                    course: "Matrix Algebra & Linear Models".to_string(),
                    topic_tags: vec!["matrix_multiplication".to_string()],
                    material_type: "worksheet".to_string(),
                    path: "materials/linear/matrix.pdf".to_string(),
                    snippet: "Compute products and explain undefined cases.".to_string(),
                },
                MaterialEntry {
                    id: "variance-notes".to_string(),
                    title: "Variance Notes".to_string(),
                    course: "Probability & Statistics for Scientists".to_string(),
                    topic_tags: vec!["variance".to_string(), "expectation".to_string()],
                    material_type: "notes".to_string(),
                    path: "materials/probability/variance.md".to_string(),
                    snippet: "Variance as expected squared deviation.".to_string(),
                },
            ],
            courses: CourseCatalog::default(),
        };

        let matches = context.search_materials(
            Some("Probability & Statistics for Scientists"),
            &[String::from("variance")],
            3,
        );

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "variance-notes");
    }

    #[test]
    fn next_timetable_slots_returns_sorted_upcoming_slots() {
        let context = LocalContext {
            deadlines: Vec::new(),
            timetable: Some(TimetableData {
                timezone: "Europe/London".to_string(),
                slots: vec![
                    TimetableSlot {
                        day: "wednesday".to_string(),
                        start: "14:00".to_string(),
                        end: "15:00".to_string(),
                        title: "Probability Workshop".to_string(),
                    },
                    TimetableSlot {
                        day: "monday".to_string(),
                        start: "09:00".to_string(),
                        end: "10:00".to_string(),
                        title: "Linear Algebra Lecture".to_string(),
                    },
                ],
            }),
            materials: Vec::new(),
            courses: CourseCatalog::default(),
        };

        let slots = context.next_timetable_slots(2);
        assert_eq!(slots.len(), 2);
        assert!(
            slots
                .iter()
                .any(|slot| slot.title == "Linear Algebra Lecture")
        );
        assert!(
            slots
                .iter()
                .any(|slot| slot.title == "Probability Workshop")
        );
    }

    #[test]
    fn timetable_round_trip_sorts_slots() {
        let path = temp_json_path();
        let timetable = TimetableData {
            timezone: "Europe/London".to_string(),
            slots: vec![
                TimetableSlot {
                    day: "wednesday".to_string(),
                    start: "14:00".to_string(),
                    end: "15:00".to_string(),
                    title: "Probability Workshop".to_string(),
                },
                TimetableSlot {
                    day: "monday".to_string(),
                    start: "09:00".to_string(),
                    end: "10:00".to_string(),
                    title: "Linear Algebra Lecture".to_string(),
                },
            ],
        };

        save_timetable(&path, &timetable)
            .unwrap_or_else(|err| panic!("save timetable failed: {err}"));
        let loaded = load_timetable(&path)
            .unwrap_or_else(|err| panic!("load timetable failed: {err}"))
            .unwrap_or_else(|| panic!("timetable should be present"));

        assert_eq!(loaded.slots[0].day, "monday");
        assert_eq!(loaded.slots[1].day, "wednesday");

        let _ = fs::remove_file(path);
    }
}
