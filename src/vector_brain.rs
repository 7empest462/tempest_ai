use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use miette::{Result, IntoDiagnostic};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VectorEntry {
    pub text: String,
    pub embedding: Vec<f32>,
    pub source: String,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct VectorBrain {
    pub entries: Vec<VectorEntry>,
}

impl VectorBrain {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add_entry(&mut self, text: String, embedding: Vec<f32>, source: String, metadata: HashMap<String, String>) {
        let entry = VectorEntry {
            text,
            embedding,
            source,
            metadata,
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
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
        let mut results: Vec<(VectorEntry, f32)> = self.entries.iter()
            .map(|entry| {
                let sim = Self::cosine_similarity(&entry.embedding, query_vector);
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
        fs::write(path, json).into_diagnostic()?;
        Ok(())
    }

    pub fn load_from_disk<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            return Ok(Self::new());
        }
        let json = fs::read_to_string(path_ref).into_diagnostic()?;
        let brain: VectorBrain = serde_json::from_str(&json).into_diagnostic()?;
        Ok(brain)
    }
}
