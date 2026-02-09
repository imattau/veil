use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: Uuid,
    pub namespace: u16,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoreSnapshot {
    pub queue: Vec<QueueItem>,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn load(&self) -> StoreSnapshot {
        let data = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(_) => return StoreSnapshot::default(),
        };
        serde_json::from_slice(&data).unwrap_or_default()
    }

    pub fn persist(&self, snapshot: &StoreSnapshot) {
        if let Ok(data) = serde_json::to_vec(snapshot) {
            let _ = fs::write(&self.path, data);
        }
    }
}
