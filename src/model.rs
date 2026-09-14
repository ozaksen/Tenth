use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const BILLING_INCREMENT_SECONDS: i64 = 6 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl Project {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: Uuid,
    pub project_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub elapsed_seconds: i64,
    pub billed_tenths: i64,
    #[serde(default)]
    pub note: String,
}

impl TimeEntry {
    pub fn from_session(
        project_id: Uuid,
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
    ) -> Self {
        let elapsed_seconds = (ended_at - started_at).num_seconds().max(1);
        Self {
            id: Uuid::new_v4(),
            project_id,
            started_at,
            ended_at,
            elapsed_seconds,
            billed_tenths: billed_tenths(elapsed_seconds),
            note: String::new(),
        }
    }

    pub fn manual(project_id: Uuid, date: NaiveDate, billed_tenths: i64) -> Self {
        let local_start = date.and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        let started_at = Local
            .from_local_datetime(&local_start)
            .single()
            .or_else(|| Local.from_local_datetime(&local_start).earliest())
            .unwrap()
            .with_timezone(&Utc);
        let elapsed_seconds = billed_tenths.max(1) * BILLING_INCREMENT_SECONDS;
        Self {
            id: Uuid::new_v4(),
            project_id,
            started_at,
            ended_at: started_at + chrono::Duration::seconds(elapsed_seconds),
            elapsed_seconds,
            billed_tenths: billed_tenths.max(1),
            note: String::new(),
        }
    }

    pub fn update_manual_values(&mut self, date: NaiveDate, billed_tenths: i64) {
        // Editing a note or project must not replace precise timer timestamps
        // with a synthetic noon start and a rounded duration.
        if self.started_at.with_timezone(&Local).date_naive() == date
            && self.billed_tenths == billed_tenths.max(1)
        {
            return;
        }
        let updated = Self::manual(self.project_id, date, billed_tenths);
        self.started_at = updated.started_at;
        self.ended_at = updated.ended_at;
        self.elapsed_seconds = updated.elapsed_seconds;
        self.billed_tenths = updated.billed_tenths;
    }

    pub fn local_date(&self) -> String {
        self.started_at
            .with_timezone(&Local)
            .format("%b %-d, %Y")
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveTimer {
    pub project_id: Uuid,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackerData {
    pub projects: Vec<Project>,
    pub entries: Vec<TimeEntry>,
    pub active_timer: Option<ActiveTimer>,
    pub dark_mode: bool,
}

pub fn billed_tenths(elapsed_seconds: i64) -> i64 {
    if elapsed_seconds <= 0 {
        0
    } else {
        (elapsed_seconds + BILLING_INCREMENT_SECONDS - 1) / BILLING_INCREMENT_SECONDS
    }
}

pub fn format_elapsed(total_seconds: i64) -> String {
    let total_seconds = total_seconds.max(0);
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billing_rounds_up_to_six_minute_increments() {
        assert_eq!(billed_tenths(0), 0);
        assert_eq!(billed_tenths(1), 1);
        assert_eq!(billed_tenths(360), 1);
        assert_eq!(billed_tenths(361), 2);
        assert_eq!(billed_tenths(3600), 10);
    }

    #[test]
    fn elapsed_time_is_formatted_for_long_sessions() {
        assert_eq!(format_elapsed(0), "00:00:00");
        assert_eq!(format_elapsed(3661), "01:01:01");
        assert_eq!(format_elapsed(90_061), "25:01:01");
    }

    #[test]
    fn manual_entries_preserve_date_and_tenths() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 26).unwrap();
        let entry = TimeEntry::manual(Uuid::new_v4(), date, 17);
        assert_eq!(entry.started_at.with_timezone(&Local).date_naive(), date);
        assert_eq!(entry.billed_tenths, 17);
        assert_eq!(entry.elapsed_seconds, 17 * BILLING_INCREMENT_SECONDS);
    }

    #[test]
    fn manual_values_can_be_updated_without_changing_entry_identity() {
        let original_date = NaiveDate::from_ymd_opt(2026, 8, 26).unwrap();
        let updated_date = NaiveDate::from_ymd_opt(2026, 8, 28).unwrap();
        let mut entry = TimeEntry::manual(Uuid::new_v4(), original_date, 4);
        let id = entry.id;
        let project_id = entry.project_id;
        entry.note = "Prepared client update".into();

        entry.update_manual_values(updated_date, 15);

        assert_eq!(entry.id, id);
        assert_eq!(entry.project_id, project_id);
        assert_eq!(
            entry.started_at.with_timezone(&Local).date_naive(),
            updated_date
        );
        assert_eq!(entry.billed_tenths, 15);
        assert_eq!(entry.elapsed_seconds, 15 * BILLING_INCREMENT_SECONDS);
        assert_eq!(entry.note, "Prepared client update");
    }

    #[test]
    fn unchanged_billing_preserves_precise_session_times() {
        let start = Utc::now();
        let end = start + chrono::Duration::seconds(61);
        let mut entry = TimeEntry::from_session(Uuid::new_v4(), start, end);
        entry.update_manual_values(start.with_timezone(&Local).date_naive(), 1);
        assert_eq!(entry.started_at, start);
        assert_eq!(entry.ended_at, end);
        assert_eq!(entry.elapsed_seconds, 61);
    }

    #[test]
    fn old_entries_without_notes_still_load() {
        let project_id = Uuid::new_v4();
        let id = Uuid::new_v4();
        let json = format!(
            r#"{{"id":"{id}","project_id":"{project_id}","started_at":"2026-08-26T12:00:00Z","ended_at":"2026-08-26T12:06:00Z","elapsed_seconds":360,"billed_tenths":1}}"#
        );

        let entry: TimeEntry = serde_json::from_str(&json).unwrap();

        assert!(entry.note.is_empty());
    }

    #[test]
    fn old_tracker_data_defaults_to_light_mode() {
        let data: TrackerData =
            serde_json::from_str(r#"{"projects":[],"entries":[],"active_timer":null}"#).unwrap();

        assert!(!data.dark_mode);
    }
}
