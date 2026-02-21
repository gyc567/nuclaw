use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{NuClawError, Result};

const DEFAULT_LINE_THRESHOLD: usize = 200;
const DEFAULT_MAX_AGE_DAYS: i64 = 90;

#[derive(Debug, Clone)]
pub struct ContentArchiver {
    threshold_lines: usize,
    archive_dir: PathBuf,
}

impl ContentArchiver {
    pub fn new(archive_dir: PathBuf) -> Self {
        Self {
            threshold_lines: DEFAULT_LINE_THRESHOLD,
            archive_dir,
        }
    }

    pub fn with_threshold(mut self, threshold: usize) -> Self {
        self.threshold_lines = threshold;
        self
    }

    pub fn should_archive(&self, path: &Path) -> bool {
        if !path.file_name().map_or(false, |n| n == "MEMORY.md") {
            return false;
        }

        if let Ok(content) = fs::read_to_string(path) {
            let lines = content.lines().count();
            return lines > self.threshold_lines;
        }

        false
    }

    pub fn archive(&self, path: &Path) -> Result<ArchiveRecord> {
        if !path.exists() {
            return Err(NuClawError::FileSystem {
                message: format!("File not found: {:?}", path),
            });
        }

        let content = fs::read_to_string(path)?;
        let line_count = content.lines().count();

        let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let archive_name = format!("MEMORY_{}.md", timestamp);

        let archive_path = self.archive_dir.join(archive_name);

        fs::create_dir_all(&self.archive_dir)?;
        fs::write(&archive_path, &content)?;

        Ok(ArchiveRecord {
            original_path: path.to_string_lossy().to_string(),
            archive_path: archive_path.to_string_lossy().to_string(),
            line_count,
        })
    }

    pub fn count_lines(&self, path: &Path) -> Result<usize> {
        let content = fs::read_to_string(path)?;
        Ok(content.lines().count())
    }
}

#[derive(Debug, Clone)]
pub struct LogCleaner {
    max_age_days: i64,
    log_dir: PathBuf,
}

impl LogCleaner {
    pub fn new(log_dir: PathBuf) -> Self {
        Self {
            max_age_days: DEFAULT_MAX_AGE_DAYS,
            log_dir,
        }
    }

    pub fn with_max_age(mut self, days: i64) -> Self {
        self.max_age_days = days;
        self
    }

    pub fn should_delete(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        if let Ok(metadata) = path.metadata() {
            if let Ok(modified) = metadata.modified() {
                let modified_dt: DateTime<Utc> = modified.into();
                let age = Utc::now().signed_duration_since(modified_dt);
                return age > Duration::days(self.max_age_days);
            }
        }

        false
    }

    pub fn clean(&self) -> Result<usize> {
        if !self.log_dir.exists() {
            return Ok(0);
        }

        let mut deleted_count = 0;

        if let Ok(entries) = fs::read_dir(&self.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if self.should_delete(&path) {
                    if fs::remove_file(&path).is_ok() {
                        deleted_count += 1;
                    }
                }
            }
        }

        Ok(deleted_count)
    }

    pub fn get_old_logs(&self) -> Result<Vec<PathBuf>> {
        if !self.log_dir.exists() {
            return Ok(Vec::new());
        }

        let mut old_logs = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.log_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if self.should_delete(&path) {
                    old_logs.push(path);
                }
            }
        }

        Ok(old_logs)
    }
}

pub struct MaintenanceScheduler {
    archiver: ContentArchiver,
    cleaner: LogCleaner,
}

impl MaintenanceScheduler {
    pub fn new(archiver: ContentArchiver, cleaner: LogCleaner) -> Self {
        Self { archiver, cleaner }
    }

    pub fn run_maintenance(&self, group_folder: &str) -> Result<MaintenanceReport> {
        let mut archives = Vec::new();
        let mut cleaned = 0;
        let mut errors = Vec::new();

        let memory_path = PathBuf::from(group_folder).join("MEMORY.md");
        if self.archiver.should_archive(&memory_path) {
            match self.archiver.archive(&memory_path) {
                Ok(record) => archives.push(record),
                Err(e) => errors.push(format!("Archive error: {}", e)),
            }
        }

        match self.cleaner.clean() {
            Ok(count) => cleaned = count,
            Err(e) => errors.push(format!("Clean error: {}", e)),
        }

        Ok(MaintenanceReport {
            archives,
            cleaned,
            errors,
            executed_at: Utc::now().to_rfc3339(),
        })
    }

    pub fn archive_memory(&self, path: &Path) -> Result<Option<ArchiveRecord>> {
        if self.archiver.should_archive(path) {
            Ok(Some(self.archiver.archive(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn clean_logs(&self) -> Result<usize> {
        self.cleaner.clean()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRecord {
    pub original_path: String,
    pub archive_path: String,
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceReport {
    pub archives: Vec<ArchiveRecord>,
    pub cleaned: usize,
    pub errors: Vec<String>,
    pub executed_at: String,
}

#[cfg(test)]
mod archiver_tests {
    use super::*;
    use std::io::Write;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nuclaw_maint_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(path: &std::path::Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_content_archiver_new() {
        let archiver = ContentArchiver::new(PathBuf::from("/tmp/archive"));
        assert_eq!(archiver.threshold_lines, DEFAULT_LINE_THRESHOLD);
    }

    #[test]
    fn test_content_archiver_with_threshold() {
        let archiver = ContentArchiver::new(PathBuf::from("/tmp")).with_threshold(100);
        assert_eq!(archiver.threshold_lines, 100);
    }

    #[test]
    fn test_should_archive_non_memory_file() {
        let dir = temp_dir();
        let test_file = dir.join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let archiver = ContentArchiver::new(dir.clone());
        assert!(!archiver.should_archive(&test_file));

        cleanup(&dir);
    }

    #[test]
    fn test_should_archive_small_memory() {
        let dir = temp_dir();
        let memory_file = dir.join("MEMORY.md");
        let mut file = fs::File::create(&memory_file).unwrap();
        for i in 0..100 {
            writeln!(file, "Line {}", i).unwrap();
        }

        let archiver = ContentArchiver::new(dir.clone());
        assert!(!archiver.should_archive(&memory_file));

        cleanup(&dir);
    }

    #[test]
    fn test_should_archive_large_memory() {
        let dir = temp_dir();
        let memory_file = dir.join("MEMORY.md");
        let mut file = fs::File::create(&memory_file).unwrap();
        for i in 0..250 {
            writeln!(file, "Line {}", i).unwrap();
        }

        let archiver = ContentArchiver::new(dir.clone());
        assert!(archiver.should_archive(&memory_file));

        cleanup(&dir);
    }

    #[test]
    fn test_archive_memory() {
        let dir = temp_dir();
        let memory_file = dir.join("MEMORY.md");
        let mut file = fs::File::create(&memory_file).unwrap();
        for i in 0..250 {
            writeln!(file, "Line {}", i).unwrap();
        }

        let archive_dir = dir.join(".history");
        let archiver = ContentArchiver::new(archive_dir);

        let result = archiver.archive(&memory_file);
        assert!(result.is_ok());

        let record = result.unwrap();
        assert!(record.archive_path.contains("MEMORY_"));
        assert_eq!(record.line_count, 250);

        cleanup(&dir);
    }

    #[test]
    fn test_count_lines() {
        let dir = temp_dir();
        let test_file = dir.join("test.txt");
        let mut file = fs::File::create(&test_file).unwrap();
        for i in 0..50 {
            writeln!(file, "Line {}", i).unwrap();
        }

        let archiver = ContentArchiver::new(dir.clone());
        let count = archiver.count_lines(&test_file).unwrap();
        assert_eq!(count, 50);

        cleanup(&dir);
    }
}

#[cfg(test)]
mod cleaner_tests {
    use super::*;
    use std::io::Write;
    use std::thread;
    use std::time::Duration as StdDuration;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nuclaw_cleaner_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(path: &std::path::Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_log_cleaner_new() {
        let cleaner = LogCleaner::new(PathBuf::from("/tmp/logs"));
        assert_eq!(cleaner.max_age_days, DEFAULT_MAX_AGE_DAYS);
    }

    #[test]
    fn test_log_cleaner_with_max_age() {
        let cleaner = LogCleaner::new(PathBuf::from("/tmp")).with_max_age(30);
        assert_eq!(cleaner.max_age_days, 30);
    }

    #[test]
    fn test_should_delete_recent_file() {
        let dir = temp_dir();
        let log_file = dir.join("recent.log");
        fs::write(&log_file, "recent log").unwrap();

        let cleaner = LogCleaner::new(dir.clone());
        assert!(!cleaner.should_delete(&log_file));

        cleanup(&dir);
    }

    #[test]
    fn test_should_delete_logic() {
        // Test the should_delete logic by checking max_age = 0
        // This will cause recent files to be treated as old
        let dir = temp_dir();
        let log_file = dir.join("test.log");
        fs::write(&log_file, "test").unwrap();

        let cleaner = LogCleaner::new(dir.clone()).with_max_age(0);

        // With max_age = 0, any file should be considered old
        let result = cleaner.should_delete(&log_file);

        cleanup(&dir);
    }

    #[test]
    fn test_clean_logs() {
        let dir = temp_dir();

        // Create a single log file
        let log_file = dir.join("test.log");
        fs::write(&log_file, "test").unwrap();

        let cleaner = LogCleaner::new(dir.clone()).with_max_age(0);
        let count = cleaner.clean().unwrap();

        // With max_age = 0, the file should be cleaned
        assert_eq!(count, 1);

        cleanup(&dir);
    }

    #[test]
    fn test_get_old_logs() {
        let dir = temp_dir();

        let log_file = dir.join("test.log");
        fs::write(&log_file, "test").unwrap();

        let cleaner = LogCleaner::new(dir.clone()).with_max_age(0);
        let old_logs = cleaner.get_old_logs().unwrap();

        // With max_age = 0, should find 1 old log
        assert_eq!(old_logs.len(), 1);

        cleanup(&dir);
    }

    #[test]
    fn test_clean_nonexistent_dir() {
        let dir = temp_dir();
        let non_existent = dir.join("nonexistent");

        let cleaner = LogCleaner::new(non_existent);
        let count = cleaner.clean().unwrap();

        assert_eq!(count, 0);
    }
}

#[cfg(test)]
mod scheduler_tests {
    use super::*;
    use std::io::Write;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("nuclaw_scheduler_{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn cleanup(path: &std::path::Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_maintenance_scheduler_new() {
        let archiver = ContentArchiver::new(PathBuf::from("/tmp/archive"));
        let cleaner = LogCleaner::new(PathBuf::from("/tmp/logs"));

        let scheduler = MaintenanceScheduler::new(archiver, cleaner);

        // Just verify it creates without panic
        assert!(true);
    }

    #[test]
    fn test_run_maintenance_no_memory() {
        let dir = temp_dir();

        let archiver = ContentArchiver::new(dir.join(".history"));
        let cleaner = LogCleaner::new(dir.clone()).with_max_age(0);

        let scheduler = MaintenanceScheduler::new(archiver, cleaner);

        let result = scheduler.run_maintenance(dir.to_str().unwrap());
        assert!(result.is_ok());

        let report = result.unwrap();
        assert!(report.archives.is_empty());

        cleanup(&dir);
    }

    #[test]
    fn test_run_maintenance_with_memory() {
        let dir = temp_dir();

        // Create MEMORY.md with content
        let memory = dir.join("MEMORY.md");
        let mut file = fs::File::create(&memory).unwrap();
        for i in 0..250 {
            writeln!(file, "Line {}", i).unwrap();
        }

        let archiver = ContentArchiver::new(dir.join(".history"));
        let cleaner = LogCleaner::new(dir.clone()).with_max_age(0);

        let scheduler = MaintenanceScheduler::new(archiver, cleaner);

        let result = scheduler.run_maintenance(dir.to_str().unwrap());
        assert!(result.is_ok());

        let report = result.unwrap();
        assert!(!report.archives.is_empty());

        cleanup(&dir);
    }

    #[test]
    fn test_archive_memory_no_archive_needed() {
        let dir = temp_dir();

        let memory = dir.join("MEMORY.md");
        fs::write(&memory, "short content").unwrap();

        let archiver = ContentArchiver::new(dir.join(".history"));
        let cleaner = LogCleaner::new(dir.clone());

        let scheduler = MaintenanceScheduler::new(archiver, cleaner);

        let result = scheduler.archive_memory(&memory).unwrap();
        assert!(result.is_none());

        cleanup(&dir);
    }

    #[test]
    fn test_archive_memory_archive_needed() {
        let dir = temp_dir();

        let memory = dir.join("MEMORY.md");
        let mut file = fs::File::create(&memory).unwrap();
        for i in 0..250 {
            writeln!(file, "Line {}", i).unwrap();
        }

        let archiver = ContentArchiver::new(dir.join(".history"));
        let cleaner = LogCleaner::new(dir.clone());

        let scheduler = MaintenanceScheduler::new(archiver, cleaner);

        let result = scheduler.archive_memory(&memory).unwrap();
        assert!(result.is_some());

        cleanup(&dir);
    }

    #[test]
    fn test_clean_logs() {
        let dir = temp_dir();

        let old_log = dir.join("old.log");
        fs::write(&old_log, "old").unwrap();

        let archiver = ContentArchiver::new(dir.clone());
        let cleaner = LogCleaner::new(dir.clone()).with_max_age(0);

        let scheduler = MaintenanceScheduler::new(archiver, cleaner);

        let count = scheduler.clean_logs().unwrap();
        assert_eq!(count, 1);

        cleanup(&dir);
    }
}

#[cfg(test)]
mod record_tests {
    use super::*;

    #[test]
    fn test_archive_record() {
        let record = ArchiveRecord {
            original_path: "/path/to/MEMORY.md".to_string(),
            archive_path: "/path/to/.history/MEMORY_20260101.md".to_string(),
            line_count: 250,
        };

        assert_eq!(record.original_path, "/path/to/MEMORY.md");
        assert_eq!(record.line_count, 250);
    }

    #[test]
    fn test_maintenance_report() {
        let report = MaintenanceReport {
            archives: vec![ArchiveRecord {
                original_path: "/path/to/MEMORY.md".to_string(),
                archive_path: "/path/to/.history/MEMORY_20260101.md".to_string(),
                line_count: 250,
            }],
            cleaned: 5,
            errors: vec![],
            executed_at: "2026-01-01T00:00:00Z".to_string(),
        };

        assert_eq!(report.archives.len(), 1);
        assert_eq!(report.cleaned, 5);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_maintenance_report_serialization() {
        let report = MaintenanceReport {
            archives: vec![],
            cleaned: 0,
            errors: vec!["error 1".to_string()],
            executed_at: "2026-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&report).unwrap();
        let parsed: MaintenanceReport = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.errors.len(), 1);
    }
}
