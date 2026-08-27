use std::{
    fs, io,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;

use crate::model::TrackerData;

pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new() -> Self {
        let path = ProjectDirs::from("com", "HourTracker", "Hour Tracker")
            .map(|dirs| dirs.data_local_dir().join("tracker.json"))
            .unwrap_or_else(|| PathBuf::from("hour-tracker-data.json"));
        Self { path }
    }

    #[cfg(test)]
    fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<TrackerData, String> {
        if !self.path.exists() {
            return Ok(TrackerData::default());
        }
        let json = fs::read_to_string(&self.path)
            .map_err(|error| format!("Could not read data: {error}"))?;
        serde_json::from_str(&json).map_err(|error| format!("Could not parse data: {error}"))
    }

    pub fn save(&self, data: &TrackerData) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create data directory: {error}"))?;
        }

        let json = serde_json::to_vec_pretty(data)
            .map_err(|error| format!("Could not serialize data: {error}"))?;
        let temporary_path = self.path.with_extension("json.tmp");
        fs::write(&temporary_path, json)
            .map_err(|error| format!("Could not write data: {error}"))?;
        replace_file(&temporary_path, &self.path)
            .map_err(|error| format!("Could not finish saving data: {error}"))
    }

    pub fn export_report(&self, file_name: &str, contents: &str) -> Result<PathBuf, String> {
        let directory = self
            .path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("reports");
        fs::create_dir_all(&directory)
            .map_err(|error| format!("Could not create reports directory: {error}"))?;
        let path = directory.join(file_name);
        fs::write(&path, contents)
            .map_err(|error| format!("Could not export weekly report: {error}"))?;
        Ok(path)
    }
}

fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    if to.exists() {
        fs::remove_file(to)?;
    }
    fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    #[test]
    fn saved_data_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let storage = Storage::at(directory.path().join("nested/tracker.json"));
        let mut data = TrackerData::default();
        data.projects.push(Project::new("Client redesign".into()));

        storage.save(&data).unwrap();
        let loaded = storage.load().unwrap();

        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].name, "Client redesign");
    }
}
