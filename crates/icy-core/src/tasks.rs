//! Persistent workspace To-dos stored in `~/.config/icy/tasks.json`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::platform;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
    pub id: u64,
    pub title: String,
    pub done: bool,
    pub tag: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStore {
    next_id: u64,
    items: Vec<TaskItem>,
}

impl TaskStore {
    pub fn load() -> Self {
        Self::load_from(&tasks_path())
    }

    pub fn load_from(path: &Path) -> Self {
        if let Ok(raw) = fs::read_to_string(path)
            && let Ok(parsed) = serde_json::from_str::<TaskStore>(&raw)
        {
            return parsed;
        }
        let store = Self {
            next_id: 4,
            items: vec![
                TaskItem {
                    id: 1,
                    title: "Connect SSH key to GitHub in Settings".into(),
                    done: false,
                    tag: "setup".into(),
                },
                TaskItem {
                    id: 2,
                    title: "Try Niri dual-column tiling (Alt+H / Alt+L)".into(),
                    done: false,
                    tag: "workflow".into(),
                },
                TaskItem {
                    id: 3,
                    title: "Customize theme JSON in ~/.config/icy/themes/".into(),
                    done: false,
                    tag: "theme".into(),
                },
            ],
        };
        let _ = store.save_to(path);
        store
    }

    pub fn items(&self) -> &[TaskItem] {
        &self.items
    }

    pub fn add(&mut self, title: impl Into<String>, tag: impl Into<String>) -> Option<&TaskItem> {
        let trimmed = title.into().trim().to_string();
        if trimmed.is_empty() {
            return None;
        }
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        self.items.push(TaskItem {
            id,
            title: trimmed,
            done: false,
            tag: tag.into(),
        });
        let _ = self.save();
        self.items.last()
    }

    pub fn toggle(&mut self, id: u64) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.done = !item.done;
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        if self.items.len() != before {
            let _ = self.save();
            true
        } else {
            false
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&tasks_path())
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        fs::write(path, json)
    }
}

pub fn tasks_path() -> PathBuf {
    platform::config_dir().join("tasks.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_toggles_and_persists_tasks() {
        let path = std::env::temp_dir().join(format!("icy-tasks-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut store = TaskStore::load_from(&path);
        let initial = store.items().len();
        store.add("Ship tiling panes", "core");
        assert_eq!(store.items().len(), initial + 1);
        let id = store.items().last().unwrap().id;
        assert!(store.toggle(id));
        assert!(store.items().last().unwrap().done);
        let _ = fs::remove_file(&path);
    }
}
