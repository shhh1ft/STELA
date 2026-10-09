use crate::model::Project;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppData {
    pub projects: Vec<Project>,
    pub active_project_id: Option<String>,
}

fn data_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("stela");
    let _ = fs::create_dir_all(&dir);
    dir.join("atelier.json")
}

pub fn load() -> AppData {
    let path = data_path();
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => AppData::default(),
    }
}

pub fn save(data: &AppData) {
    let path = data_path();
    if let Ok(text) = serde_json::to_string_pretty(data) {
        let _ = fs::write(path, text);
    }
    for project in &data.projects {
        if let Some(fp) = project.file_path.as_ref() {
            let _ = export_project(project, std::path::Path::new(fp));
        }
    }
}

pub fn export_project(project: &Project, path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(project).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

pub fn import_project(path: &std::path::Path) -> Result<Project, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut project: Project = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    project.id = crate::model::new_id();
    project.updated_at = chrono::Utc::now();
    project.file_path = Some(path.to_string_lossy().into_owned());
    Ok(project)
}
