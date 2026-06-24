use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VectorEntry {
    pub text: String,
    pub embedding: Vec<f32>,
    pub source: String,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct VectorBrain {
    pub entries: Vec<VectorEntry>,
    #[serde(skip)]
    pub passphrase: Option<String>,
}

impl VectorBrain {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            passphrase: None,
        }
    }

    pub fn normalize_vector(v: &mut [f32]) {
        let norm: f32 = v.iter().map(|&x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }

    pub fn add_entry(
        &mut self,
        text: String,
        mut embedding: Vec<f32>,
        source: String,
        metadata: HashMap<String, String>,
    ) {
        Self::normalize_vector(&mut embedding);
        let entry = VectorEntry {
            text,
            embedding,
            source,
            metadata,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        self.entries.push(entry);
    }

    pub fn remove_entries_by_source_prefix(&mut self, prefix: &str) {
        self.entries.retain(|e| !e.source.starts_with(prefix));
    }

    pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
        if v1.len() != v2.len() || v1.is_empty() {
            return 0.0;
        }
        let dot_product: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        let norm_v1: f32 = v1.iter().map(|a| a * a).sum::<f32>().sqrt();
        let norm_v2: f32 = v2.iter().map(|a| a * a).sum::<f32>().sqrt();

        if norm_v1 == 0.0 || norm_v2 == 0.0 {
            0.0
        } else {
            dot_product / (norm_v1 * norm_v2)
        }
    }

    pub fn search(&self, query_vector: &[f32], top_k: usize) -> Vec<(VectorEntry, f32)> {
        if query_vector.is_empty() || self.entries.is_empty() {
            return Vec::new();
        }

        // Calculate the norm of the query vector once
        let query_norm: f32 = query_vector.iter().map(|&x| x * x).sum::<f32>().sqrt();
        if query_norm == 0.0 {
            return Vec::new();
        }

        let mut results: Vec<(VectorEntry, f32)> = self
            .entries
            .iter()
            .map(|entry| {
                if entry.embedding.len() != query_vector.len() {
                    return (entry.clone(), 0.0);
                }
                let dot_product: f32 = entry
                    .embedding
                    .iter()
                    .zip(query_vector.iter())
                    .map(|(a, b)| a * b)
                    .sum();
                // Since entry.embedding is pre-normalized to unit length, its norm is 1.0.
                let sim = dot_product / query_norm;
                (entry.clone(), sim)
            })
            .collect();

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    pub fn save_to_disk<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).into_diagnostic()?;
        }
        let json = serde_json::to_string_pretty(self).into_diagnostic()?;
        let data = if let Some(passphrase) = &self.passphrase {
            crate::crypto::encrypt_history(json.as_bytes(), passphrase)?
        } else {
            json.into_bytes()
        };
        fs::write(path, data).into_diagnostic()?;
        Ok(())
    }

    pub fn load_from_disk<P: AsRef<Path>>(path: P, passphrase: Option<&str>) -> Result<Self> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            let mut brain = Self::new();
            brain.passphrase = passphrase.map(|s| s.to_string());
            return Ok(brain);
        }
        let bytes = fs::read(path_ref).into_diagnostic()?;
        let json_str = if let Some(passphrase) = passphrase {
            match crate::crypto::decrypt_history(&bytes, passphrase) {
                Ok(decrypted) => String::from_utf8(decrypted).into_diagnostic()?,
                Err(_) => {
                    // Fallback to reading as plain text if it was unencrypted
                    String::from_utf8(bytes).into_diagnostic()?
                }
            }
        } else {
            String::from_utf8(bytes).into_diagnostic()?
        };
        let mut brain: VectorBrain = serde_json::from_str(&json_str).into_diagnostic()?;
        // Pre-normalize all loaded legacy/stored vectors
        for entry in &mut brain.entries {
            Self::normalize_vector(&mut entry.embedding);
        }
        brain.passphrase = passphrase.map(|s| s.to_string());
        Ok(brain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_brain_encryption_and_migration() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("brain.json");
        let passphrase = "test-secret-passphrase";

        // 1. Create a VectorBrain, add some entries, and save it UNENCRYPTED
        let mut brain = VectorBrain::new();
        brain.add_entry(
            "unencrypted fact".to_string(),
            vec![0.1, 0.2, 0.3],
            "test_source".to_string(),
            HashMap::new(),
        );
        brain.save_to_disk(&file_path).unwrap();

        // Verify it was saved as plain-text JSON
        let raw_content = fs::read_to_string(&file_path).unwrap();
        assert!(raw_content.contains("unencrypted fact"));

        // 2. Load it back with a passphrase. It should decrypt/migrate transparently!
        let loaded_brain = VectorBrain::load_from_disk(&file_path, Some(passphrase)).unwrap();
        assert_eq!(loaded_brain.entries.len(), 1);
        assert_eq!(loaded_brain.entries[0].text, "unencrypted fact");
        assert_eq!(loaded_brain.passphrase.as_deref(), Some(passphrase));

        // 3. Save it again. Since it now has a passphrase, it must be encrypted!
        loaded_brain.save_to_disk(&file_path).unwrap();

        // Verify it is no longer readable as plain text
        let raw_bytes = fs::read(&file_path).unwrap();
        let raw_str = String::from_utf8(raw_bytes.clone());
        if let Ok(s) = raw_str {
            assert!(!s.contains("unencrypted fact"));
        }

        // 4. Load the encrypted brain back with the passphrase
        let final_brain = VectorBrain::load_from_disk(&file_path, Some(passphrase)).unwrap();
        assert_eq!(final_brain.entries.len(), 1);
        assert_eq!(final_brain.entries[0].text, "unencrypted fact");

        // 5. Loading with the WRONG passphrase should fail to decrypt
        let bad_load = VectorBrain::load_from_disk(&file_path, Some("wrong-passphrase"));
        assert!(bad_load.is_err());
    }
}
