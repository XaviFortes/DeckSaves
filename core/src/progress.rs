use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Progress information for sync operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub operation: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub current_file: Option<String>,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub percentage: f32,
    pub message: String,
}

impl SyncProgress {
    pub fn new(operation: String, total_steps: usize) -> Self {
        Self {
            operation,
            current_step: 0,
            total_steps,
            current_file: None,
            bytes_transferred: 0,
            total_bytes: 0,
            percentage: 0.0,
            message: "Starting...".to_string(),
        }
    }

    pub fn update_step(&mut self, step: usize, message: String) {
        self.current_step = step;
        self.message = message;
        self.percentage = if self.total_steps > 0 {
            (self.current_step as f32 / self.total_steps as f32) * 100.0
        } else {
            0.0
        };
    }

    pub fn set_current_file(&mut self, file_name: String) {
        self.current_file = Some(file_name);
    }

    pub fn update_bytes(&mut self, transferred: u64, total: u64) {
        self.bytes_transferred = transferred;
        self.total_bytes = total;
    }

    pub fn complete(&mut self) {
        self.current_step = self.total_steps;
        self.percentage = 100.0;
        self.message = "Completed successfully".to_string();
        self.current_file = None;
    }
}

/// Progress callback trait for sync operations
pub trait ProgressCallback: Send + Sync {
    fn on_progress(&self, progress: SyncProgress);
}

/// A simple progress callback that stores the latest progress
#[derive(Debug, Clone)]
pub struct SimpleProgressCallback {
    pub latest_progress: std::sync::Arc<std::sync::Mutex<Option<SyncProgress>>>,
}

impl SimpleProgressCallback {
    pub fn new() -> Self {
        Self {
            latest_progress: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn get_latest(&self) -> Option<SyncProgress> {
        self.latest_progress.lock().unwrap().clone()
    }
}

impl ProgressCallback for SimpleProgressCallback {
    fn on_progress(&self, progress: SyncProgress) {
        *self.latest_progress.lock().unwrap() = Some(progress);
    }
}

/// Enhanced sync summary with progress tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedSyncSummary {
    pub remote_versions_merged: bool,
    pub manifest_uploaded: bool,
    pub files_downloaded: u32,
    pub files_uploaded: u32,
    pub conflicts_resolved: u32,
    pub total_bytes_transferred: u64,
    pub file_details: HashMap<String, FileOperationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperationResult {
    pub operation: String, // "upload", "download", "merge"
    pub success: bool,
    pub bytes: u64,
    pub duration_ms: u64,
    pub error: Option<String>,
}

impl EnhancedSyncSummary {
    pub fn new() -> Self {
        Self {
            remote_versions_merged: false,
            manifest_uploaded: false,
            files_downloaded: 0,
            files_uploaded: 0,
            conflicts_resolved: 0,
            total_bytes_transferred: 0,
            file_details: HashMap::new(),
        }
    }

    pub fn add_file_operation(&mut self, file_path: String, result: FileOperationResult) {
        self.total_bytes_transferred += result.bytes;
        match result.operation.as_str() {
            "download" if result.success => self.files_downloaded += 1,
            "upload" if result.success => self.files_uploaded += 1,
            _ => {}
        }
        self.file_details.insert(file_path, result);
    }
}
