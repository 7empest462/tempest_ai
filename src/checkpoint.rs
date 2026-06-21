use chrono::Utc;
use parking_lot::Mutex;
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A single checkpoint capturing the state of files before a batch of modifications.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    /// Unique identifier for this checkpoint
    pub id: String,
    /// Human-readable description of what triggered this checkpoint
    pub description: String,
    /// Timestamp when the checkpoint was created
    pub timestamp: String,
    /// Map of file path → original content (before modification)
    /// If the file didn't exist, the value is None (so undo = delete)
    pub snapshots: HashMap<PathBuf, Option<Vec<u8>>>,
    /// Map of file path → committed content (after modification, captured at commit time)
    /// If the file didn't exist and wasn't created/written, the value is None
    pub committed: HashMap<PathBuf, Option<Vec<u8>>>,
}

/// Manages the checkpoint stack for undo/redo operations.
pub struct CheckpointManager {
    /// Stack of checkpoints (most recent last)
    stack: Vec<Checkpoint>,
    /// Maximum number of checkpoints to retain
    max_checkpoints: usize,
    /// Currently accumulating checkpoint (files snapshotted during this tool batch)
    pending: Option<Checkpoint>,
}

impl CheckpointManager {
    pub fn new(max_checkpoints: usize) -> Self {
        Self {
            stack: Vec::new(),
            max_checkpoints,
            pending: None,
        }
    }

    /// Begin a new checkpoint batch. Call this before executing a batch of modifying tools.
    pub fn begin_checkpoint(&mut self, description: &str) {
        self.pending = Some(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            description: description.to_string(),
            timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            snapshots: HashMap::new(),
            committed: HashMap::new(),
        });
    }

    /// Snapshot a file's current state BEFORE it gets modified.
    /// This is idempotent — if the same file is snapshotted twice in one checkpoint,
    /// only the first (original) state is kept.
    pub fn snapshot_file(&mut self, path: &Path) {
        if let Some(ref mut pending) = self.pending {
            // Only snapshot once per file per checkpoint (keep the original state)
            if pending.snapshots.contains_key(path) {
                return;
            }

            let content = fs::read(path).ok();
            pending.snapshots.insert(path.to_path_buf(), content);
        }
    }

    /// Commit the pending checkpoint to the stack.
    /// Returns the checkpoint ID if successful.
    pub fn commit_checkpoint(&mut self) -> Option<String> {
        if let Some(mut checkpoint) = self.pending.take() {
            if checkpoint.snapshots.is_empty() {
                return None; // Nothing was actually modified
            }

            // Capture the committed state of each file
            let mut committed = HashMap::new();
            for path in checkpoint.snapshots.keys() {
                let content = fs::read(path).ok();
                committed.insert(path.clone(), content);
            }
            checkpoint.committed = committed;

            let id = checkpoint.id.clone();
            self.stack.push(checkpoint);

            // Trim old checkpoints if we exceed the limit
            while self.stack.len() > self.max_checkpoints {
                self.stack.remove(0);
            }

            Some(id)
        } else {
            None
        }
    }

    /// Undo the most recent checkpoint by restoring all snapshotted files.
    /// Returns a summary of what was restored.
    pub fn undo(&mut self) -> Result<String, String> {
        let checkpoint = self
            .stack
            .pop()
            .ok_or_else(|| "No checkpoints available to undo.".to_string())?;

        let mut restored = Vec::new();
        let mut errors = Vec::new();

        for (path, original_content) in &checkpoint.snapshots {
            let committed_content = checkpoint.committed.get(path).cloned().flatten();
            let current_content = fs::read(path).ok();

            let is_modified_externally = current_content.is_some()
                && committed_content.is_some()
                && current_content != committed_content;

            let suffix = if is_modified_externally {
                " (Warning: Modified externally since checkpoint)"
            } else {
                ""
            };

            match original_content {
                Some(content) => {
                    // Ensure parent directories exist
                    if let Some(parent) = path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    // File existed before — restore its content
                    match fs::write(path, content) {
                        Ok(_) => {
                            restored.push(format!("  ↩ Restored{}: {}", suffix, path.display()))
                        }
                        Err(e) => {
                            errors.push(format!("  ✗ Failed to restore {}: {}", path.display(), e))
                        }
                    }
                }
                None => {
                    // File didn't exist before — delete it
                    if path.exists() {
                        match fs::remove_file(path) {
                            Ok(_) => restored.push(format!(
                                "  🗑 Removed (was new){}: {}",
                                suffix,
                                path.display()
                            )),
                            Err(e) => {
                                errors.push(format!(
                                    "  ✗ Failed to remove {}: {}",
                                    path.display(),
                                    e
                                ));
                            }
                        }
                    }
                }
            }
        }

        let mut summary = format!(
            "⏪ Undo Checkpoint: {} ({})\n{}\n",
            checkpoint.description,
            checkpoint.timestamp,
            "─".repeat(50)
        );

        if !restored.is_empty() {
            summary.push_str(&restored.join("\n"));
            summary.push('\n');
        }

        if !errors.is_empty() {
            summary.push_str("\n⚠️ Errors:\n");
            summary.push_str(&errors.join("\n"));
            summary.push('\n');
        }

        Ok(summary)
    }

    /// Rollback the pending checkpoint, restoring all snapshotted files to original states.
    /// Discards the pending checkpoint.
    pub fn rollback_pending(&mut self) -> Result<String, String> {
        let checkpoint = self
            .pending
            .take()
            .ok_or_else(|| "No pending checkpoint to rollback.".to_string())?;

        let mut restored = Vec::new();
        let mut errors = Vec::new();

        for (path, original_content) in &checkpoint.snapshots {
            match original_content {
                Some(content) => {
                    // Ensure parent directories exist
                    if let Some(parent) = path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    match fs::write(path, content) {
                        Ok(_) => restored.push(format!("  ↩ Restored: {}", path.display())),
                        Err(e) => {
                            errors.push(format!("  ✗ Failed to restore {}: {}", path.display(), e))
                        }
                    }
                }
                None => {
                    if path.exists() {
                        match fs::remove_file(path) {
                            Ok(_) => {
                                restored.push(format!("  🗑 Removed (was new): {}", path.display()))
                            }
                            Err(e) => {
                                errors.push(format!(
                                    "  ✗ Failed to remove {}: {}",
                                    path.display(),
                                    e
                                ));
                            }
                        }
                    }
                }
            }
        }

        let mut summary = format!(
            "⏪ Rolled Back Pending Checkpoint: {} ({})\n",
            checkpoint.description, checkpoint.timestamp
        );

        if !restored.is_empty() {
            summary.push_str(&restored.join("\n"));
            summary.push('\n');
        }

        if !errors.is_empty() {
            summary.push_str("\n⚠️ Errors during rollback:\n");
            summary.push_str(&errors.join("\n"));
            summary.push('\n');
        }

        Ok(summary)
    }

    /// Get the number of available checkpoints.
    pub fn checkpoint_count(&self) -> usize {
        self.stack.len()
    }

    /// Get a summary of all available checkpoints (most recent first).
    pub fn list_checkpoints(&self) -> String {
        if self.stack.is_empty() {
            return "No checkpoints available.".to_string();
        }

        let mut out = String::from("📋 Checkpoint History:\n");
        for (i, cp) in self.stack.iter().rev().enumerate() {
            let file_count = cp.snapshots.len();
            out.push_str(&format!(
                "  {}. [{}] {} ({} file{})\n",
                i + 1,
                cp.timestamp,
                cp.description,
                file_count,
                if file_count != 1 { "s" } else { "" }
            ));
        }
        out
    }

    /// Check if there's a pending (uncommitted) checkpoint.
    #[allow(dead_code)]
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Discard the pending checkpoint without committing.
    pub fn discard_pending(&mut self) {
        self.pending = None;
    }

    /// Get the original states and current states of all files modified in the last `n` checkpoints.
    pub fn get_turn_modifications(
        &self,
        n: usize,
    ) -> Vec<(PathBuf, Option<String>, Option<String>)> {
        let mut original_states = HashMap::new();
        let len = self.stack.len();
        if n == 0 || len == 0 {
            return Vec::new();
        }

        let start_idx = len.saturating_sub(n);
        // Iterate from oldest to newest to capture the earliest original state
        for cp in &self.stack[start_idx..] {
            for (path, original_content) in &cp.snapshots {
                if !original_states.contains_key(path) {
                    original_states.insert(path.clone(), original_content.clone());
                }
            }
        }

        let mut results = Vec::new();
        for (path, original) in original_states {
            let current = fs::read(&path).ok();
            let original_str = original.map(|v| String::from_utf8_lossy(&v).into_owned());
            let current_str = current.map(|v| String::from_utf8_lossy(&v).into_owned());
            results.push((path, original_str, current_str));
        }
        results
    }
}

/// Generate a unified diff preview for a batch of file modifications.
/// Takes a list of (path, new_content) pairs and produces a consolidated diff string.
pub fn generate_batch_diff(modifications: &[(PathBuf, String)]) -> String {
    if modifications.is_empty() {
        return "No modifications to preview.".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!(
        "📋 Unified Diff Preview ({} file{})\n",
        modifications.len(),
        if modifications.len() != 1 { "s" } else { "" }
    ));
    output.push_str(&"═".repeat(60));
    output.push('\n');

    for (path, new_content) in modifications {
        let old_content = fs::read_to_string(path).unwrap_or_default();
        let file_existed = path.exists();

        let display_path = path.display().to_string();

        if !file_existed {
            // New file
            output.push_str(&format!("\n┌─ 📄 NEW FILE: {}\n", display_path));
            output.push_str(&"─".repeat(60));
            output.push('\n');

            let line_count = new_content.lines().count();
            output.push_str(&format!("  + {} lines\n", line_count));

            // Show first 20 lines as preview
            for (i, line) in new_content.lines().take(20).enumerate() {
                output.push_str(&format!("  + {:>4} │ {}\n", i + 1, line));
            }
            if line_count > 20 {
                output.push_str(&format!("  ... ({} more lines)\n", line_count - 20));
            }
        } else {
            // Modified file — show unified diff
            let diff = TextDiff::from_lines(&old_content, new_content);
            let ops = diff.grouped_ops(3);

            if ops.is_empty() {
                output.push_str(&format!("\n┌─ 📄 {} (no changes)\n", display_path));
                continue;
            }

            // Count additions and deletions
            let mut additions = 0;
            let mut deletions = 0;
            for group in &ops {
                for op in group {
                    for change in diff.iter_changes(op) {
                        match change.tag() {
                            ChangeTag::Insert => additions += 1,
                            ChangeTag::Delete => deletions += 1,
                            ChangeTag::Equal => {}
                        }
                    }
                }
            }

            output.push_str(&format!("\n┌─ 📝 {}", display_path));
            output.push_str(&format!("  (+{} -{} lines)\n", additions, deletions));
            output.push_str(&"─".repeat(60));
            output.push('\n');

            for (i, group) in ops.iter().enumerate() {
                if i > 0 {
                    output.push_str("  ┈┈┈\n");
                }
                for op in group {
                    for change in diff.iter_changes(op) {
                        let (sign, line_num) = match change.tag() {
                            ChangeTag::Delete => ("-", change.old_index().map(|i| i + 1)),
                            ChangeTag::Insert => ("+", change.new_index().map(|i| i + 1)),
                            ChangeTag::Equal => (" ", change.new_index().map(|i| i + 1)),
                        };
                        let ln = line_num
                            .map(|n| format!("{:>4}", n))
                            .unwrap_or_else(|| "    ".to_string());
                        output.push_str(&format!("  {} {} │ {}", sign, ln, change.value()));
                        // Ensure newline
                        if !change.value().ends_with('\n') {
                            output.push('\n');
                        }
                    }
                }
            }
        }
        output.push_str(&"─".repeat(60));
        output.push('\n');
    }

    output.push_str(&"═".repeat(60));
    output.push('\n');
    output
}

/// Thread-safe wrapper for the CheckpointManager
pub type SharedCheckpointManager = std::sync::Arc<Mutex<CheckpointManager>>;

/// Create a new shared checkpoint manager
pub fn new_shared(max_checkpoints: usize) -> SharedCheckpointManager {
    std::sync::Arc::new(Mutex::new(CheckpointManager::new(max_checkpoints)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_checkpoint_and_undo() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        // Create initial file
        let mut f = fs::File::create(&file_path).unwrap();
        write!(f, "original content").unwrap();
        drop(f);

        let mut mgr = CheckpointManager::new(10);

        // Begin checkpoint and snapshot
        mgr.begin_checkpoint("test edit");
        mgr.snapshot_file(&file_path);

        // Simulate modification
        fs::write(&file_path, "modified content").unwrap();

        // Commit checkpoint
        let id = mgr.commit_checkpoint();
        assert!(id.is_some());
        assert_eq!(mgr.checkpoint_count(), 1);

        // Verify file is modified
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "modified content");

        // Undo
        let result = mgr.undo();
        assert!(result.is_ok());

        // Verify file is restored
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "original content");
        assert_eq!(mgr.checkpoint_count(), 0);
    }

    #[test]
    fn test_new_file_undo() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let mut mgr = CheckpointManager::new(10);

        // Begin checkpoint — file doesn't exist yet
        mgr.begin_checkpoint("create new file");
        mgr.snapshot_file(&file_path);

        // Create the file
        fs::write(&file_path, "new content").unwrap();

        mgr.commit_checkpoint();
        assert!(file_path.exists());

        // Undo should delete the file
        mgr.undo().unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn test_batch_diff_preview() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("code.rs");
        fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        let modifications = vec![(
            file_path,
            "fn main() {\n    println!(\"goodbye\");\n    println!(\"world\");\n}\n".to_string(),
        )];

        let preview = generate_batch_diff(&modifications);
        assert!(preview.contains("code.rs"));
        assert!(preview.contains("goodbye"));
    }

    #[test]
    fn test_max_checkpoints() {
        let mut mgr = CheckpointManager::new(3);

        for i in 0..5 {
            mgr.begin_checkpoint(&format!("checkpoint {}", i));
            mgr.pending.as_mut().unwrap().snapshots.insert(
                PathBuf::from(format!("/tmp/fake_{}", i)),
                Some("content".as_bytes().to_vec()),
            );
            mgr.commit_checkpoint();
        }

        // Should only keep the last 3
        assert_eq!(mgr.checkpoint_count(), 3);
    }

    #[test]
    fn test_checkpoint_binary_safety() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.bin");

        let binary_data = vec![0, 15, 255, 128, 64, 0, 32];
        fs::write(&file_path, &binary_data).unwrap();

        let mut mgr = CheckpointManager::new(10);
        mgr.begin_checkpoint("binary edit");
        mgr.snapshot_file(&file_path);

        fs::write(&file_path, vec![255, 0, 255]).unwrap();
        mgr.commit_checkpoint();

        // Undo
        mgr.undo().unwrap();
        assert_eq!(fs::read(&file_path).unwrap(), binary_data);
    }

    #[test]
    fn test_rollback_pending() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("rollback.txt");
        fs::write(&file_path, "original content").unwrap();

        let mut mgr = CheckpointManager::new(10);
        mgr.begin_checkpoint("rollback test");
        mgr.snapshot_file(&file_path);

        fs::write(&file_path, "dirty content").unwrap();

        // Rollback
        let res = mgr.rollback_pending();
        assert!(res.is_ok());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "original content");
        assert!(!mgr.has_pending());
        assert_eq!(mgr.checkpoint_count(), 0);
    }

    #[test]
    fn test_directory_recreation_on_undo() {
        let dir = tempfile::tempdir().unwrap();
        let sub_dir = dir.path().join("nested").join("folder");
        fs::create_dir_all(&sub_dir).unwrap();
        let file_path = sub_dir.join("nested_file.txt");
        fs::write(&file_path, "nested file").unwrap();

        let mut mgr = CheckpointManager::new(10);
        mgr.begin_checkpoint("dir deletion test");
        mgr.snapshot_file(&file_path);

        // Delete nested file and the directories
        fs::remove_file(&file_path).unwrap();
        fs::remove_dir(&sub_dir).unwrap();
        fs::remove_dir(sub_dir.parent().unwrap()).unwrap();

        mgr.commit_checkpoint();

        // Undo should recreate nested directories and restore the file
        mgr.undo().unwrap();
        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "nested file");
    }

    #[test]
    fn test_external_change_detection() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("external.txt");
        fs::write(&file_path, "base").unwrap();

        let mut mgr = CheckpointManager::new(10);
        mgr.begin_checkpoint("external mod test");
        mgr.snapshot_file(&file_path);

        fs::write(&file_path, "agent mod").unwrap();
        mgr.commit_checkpoint();

        // Now simulate external modification (by user or another process)
        fs::write(&file_path, "external user mod").unwrap();

        let summary = mgr.undo().unwrap();
        assert!(summary.contains("Warning: Modified externally since checkpoint"));
        // Check that original content was still successfully restored
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "base");
    }
}
