//! Aggregate limits for one presentation-config/theme loading operation.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

pub const MAX_PATHS: usize = 64;
pub const MAX_PATH_BYTES: usize = 32 * 1024;
pub const MAX_READ_BYTES: usize = 2 * 1024 * 1024;

#[derive(Default)]
pub struct ReadBudget {
    paths: usize,
    path_bytes: usize,
    read_bytes: usize,
}

impl ReadBudget {
    pub fn check_path(&mut self, path: &Path) -> Result<(), String> {
        let paths = self.paths.saturating_add(1);
        let bytes = self.path_bytes.saturating_add(path.as_os_str().len());
        if paths > MAX_PATHS || bytes > MAX_PATH_BYTES {
            return Err("TUI config/theme aggregate path budget exceeded".into());
        }
        self.paths = paths;
        self.path_bytes = bytes;
        Ok(())
    }

    pub fn read(
        &mut self,
        path: &Path,
        per_file: usize,
        label: &str,
    ) -> Result<Option<String>, String> {
        self.check_path(path)?;
        let remaining = MAX_READ_BYTES.saturating_sub(self.read_bytes);
        if remaining == 0 {
            return Err("TUI config/theme aggregate read budget exceeded".into());
        }
        let limit = per_file.min(remaining.saturating_sub(1));
        let text = match crate::native_ui_preferences::read_bounded(path, limit, label) {
            Ok(text) => text,
            Err(error) => {
                // A failed bounded read may have consumed its sentinel byte.
                // Charge the worst case so retries cannot reset the allowance.
                self.read_bytes = self
                    .read_bytes
                    .saturating_add(limit.saturating_add(1).min(remaining));
                return Err(error);
            }
        };
        if let Some(text) = &text {
            self.read_bytes = self.read_bytes.saturating_add(text.len());
        }
        Ok(text)
    }
}

pub fn check_paths(paths: &[PathBuf]) -> Result<(), String> {
    let mut budget = ReadBudget::default();
    for path in paths {
        budget.check_path(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_paths_still_consume_the_probe_budget() {
        let mut budget = ReadBudget::default();
        for _ in 0..MAX_PATHS {
            budget.check_path(Path::new("/missing/tui.json")).unwrap();
        }
        assert!(budget.check_path(Path::new("/one-more")).is_err());
    }

    #[test]
    fn long_paths_have_a_cumulative_limit() {
        let mut budget = ReadBudget::default();
        let path = PathBuf::from("x".repeat(MAX_PATH_BYTES / 2));
        budget.check_path(&path).unwrap();
        budget.check_path(&path).unwrap();
        assert!(budget.check_path(Path::new("x")).is_err());
    }

    #[test]
    fn cumulative_bytes_reduce_the_next_read_limit() {
        let mut budget = ReadBudget {
            read_bytes: MAX_READ_BYTES,
            ..ReadBudget::default()
        };
        // Existing nonempty source is rejected with a zero remaining budget;
        // no further filesystem probes occur after byte exhaustion.
        assert!(budget
            .read(Path::new(file!()), 1024 * 1024, "budget fixture")
            .is_err());
        assert!(budget
            .read(
                Path::new("/no-such-opencode-config-budget-fixture"),
                1024,
                "fixture"
            )
            .is_err());
    }

    #[test]
    fn individually_valid_files_cannot_exceed_aggregate_input_limit() {
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "oc-ui-budget-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(directory.clone());
        let mut budget = ReadBudget::default();
        for index in 0..3 {
            let path = directory.join(format!("{index}.json"));
            std::fs::write(&path, " ".repeat(900 * 1024)).unwrap();
            let result = budget.read(&path, 1024 * 1024, "aggregate fixture");
            if index < 2 {
                assert_eq!(result.unwrap().unwrap().len(), 900 * 1024);
            } else {
                assert!(
                    result.is_err(),
                    "third individually valid file exceeds aggregate budget"
                );
            }
        }
        assert!(
            budget
                .read(&directory.join("0.json"), 1024 * 1024, "retry")
                .is_err(),
            "retries cannot restore an exhausted byte allowance"
        );
    }
}
