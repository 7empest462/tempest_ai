use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use similar::{TextDiff, ChangeTag};
use chrono::Utc;
use parking_lot::Mutex;

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
    pub snapshots: HashMap<PathBuf, Option<String>>,
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

            let content = fs::read_to_string(path).ok();
            pending.snapshots.insert(path.to_path_buf(), content);
        }
    }

    /// Commit the pending checkpoint to the stack.
    /// Returns the checkpoint ID if successful.
    pub fn commit_checkpoint(&mut self) -> Option<String> {
        if let Some(checkpoint) = self.pending.take() {
            if checkpoint.snapshots.is_empty() {
                return None; // Nothing was actually modified
            }

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
        let checkpoint = self.stack.pop()
            .ok_or_else(|| "No checkpoints available to undo.".to_string())?;

        let mut restored = Vec::new();
        let mut errors = Vec::new();

        for (path, original_content) in &checkpoint.snapshots {
            match original_content {
                Some(content) => {
                    // File existed before — restore its content
                    match fs::write(path, content) {
                        Ok(_) => restored.push(format!("  ↩ Restored: {}", path.display())),
                        Err(e) => errors.push(format!("  ✗ Failed to restore {}: {}", path.display(), e)),
                    }
                }
                None => {
                    // File didn't exist before — delete it
                    match fs::remove_file(path) {
                        Ok(_) => restored.push(format!("  🗑 Removed (was new): {}", path.display())),
                        Err(e) => {
                            // Not a hard error if the file is already gone
                            if path.exists() {
                                errors.push(format!("  ✗ Failed to remove {}: {}", path.display(), e));
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
}

/// Generate a unified diff preview for a batch of file modifications.
/// Takes a list of (path, new_content) pairs and produces a consolidated diff string.
pub fn generate_batch_diff(modifications: &[(PathBuf, String)]) -> String {
    if modifications.is_empty() {
        return "No modifications to preview.".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!("📋 Unified Diff Preview ({} file{})\n", 
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
                            ChangeTag::Equal  => (" ", change.new_index().map(|i| i + 1)),
                        };
                        let ln = line_num.map(|n| format!("{:>4}", n)).unwrap_or_else(|| "    ".to_string());
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
        
        let modifications = vec![
            (file_path, "fn main() {\n    println!(\"goodbye\");\n    println!(\"world\");\n}\n".to_string()),
        ];
        
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
                Some("content".to_string())
            );
            mgr.commit_checkpoint();
        }
        
        // Should only keep the last 3
        assert_eq!(mgr.checkpoint_count(), 3);
    }
}
