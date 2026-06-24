use async_trait::async_trait;
use layer0::effect::Scope;
use layer0::error::StateError;
use layer0::state::{SearchResult, StateStore};
use std::fs;
use std::path::PathBuf;

/// A local filesystem-based implementation of the `layer0::StateStore` trait.
pub struct FileStateStore {
    base_dir: PathBuf,
}

impl FileStateStore {
    /// Create a new FileStateStore at the given directory.
    pub fn new(base_dir: PathBuf) -> Self {
        fs::create_dir_all(&base_dir).ok();
        Self { base_dir }
    }

    /// Resolve Scope + Key to a unique filepath inside `base_dir`.
    fn resolve_path(&self, scope: &Scope, key: &str) -> PathBuf {
        let scope_str = match scope {
            Scope::Session(id) => format!("session_{}", id.as_str()),
            Scope::Workflow(id) => format!("workflow_{}", id.as_str()),
            Scope::Operator { workflow, operator } => {
                format!("operator_{}_{}", workflow.as_str(), operator.as_str())
            }
            Scope::Global => "global".to_string(),
            Scope::Custom(s) => format!("custom_{}", s),
            _ => "other".to_string(),
        };
        // Sanitize to avoid directory traversal
        let safe_scope: String = scope_str
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let safe_key: String = key
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
            .collect();

        self.base_dir
            .join(format!("{}_{}.json", safe_scope, safe_key))
    }
}

#[async_trait]
impl StateStore for FileStateStore {
    async fn read(
        &self,
        scope: &Scope,
        key: &str,
    ) -> Result<Option<serde_json::Value>, StateError> {
        let path = self.resolve_path(scope, key);
        match tokio::fs::read_to_string(&path).await {
            Ok(data) => {
                let value = serde_json::from_str(&data)
                    .map_err(|e| StateError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StateError::Other(Box::new(e))),
        }
    }

    async fn write(
        &self,
        scope: &Scope,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), StateError> {
        let path = self.resolve_path(scope, key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| StateError::WriteFailed(e.to_string()))?;
        }
        let data = serde_json::to_string_pretty(&value)
            .map_err(|e| StateError::Serialization(e.to_string()))?;
        tokio::fs::write(&path, data)
            .await
            .map_err(|e| StateError::WriteFailed(e.to_string()))?;
        Ok(())
    }

    async fn delete(&self, scope: &Scope, key: &str) -> Result<(), StateError> {
        let path = self.resolve_path(scope, key);
        match tokio::fs::remove_file(&path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StateError::Other(Box::new(e))),
        }
    }

    async fn list(&self, scope: &Scope, prefix: &str) -> Result<Vec<String>, StateError> {
        let scope_str = match scope {
            Scope::Session(id) => format!("session_{}", id.as_str()),
            Scope::Workflow(id) => format!("workflow_{}", id.as_str()),
            Scope::Operator { workflow, operator } => {
                format!("operator_{}_{}", workflow.as_str(), operator.as_str())
            }
            Scope::Global => "global".to_string(),
            Scope::Custom(s) => format!("custom_{}", s),
            _ => "other".to_string(),
        };
        let safe_scope: String = scope_str
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        let file_prefix = format!("{}_{}", safe_scope, prefix);

        let mut keys = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(&self.base_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(name) = entry.file_name().to_str()
                    && name.starts_with(&file_prefix)
                    && name.ends_with(".json")
                {
                    let key = name[safe_scope.len() + 1..name.len() - 5].to_string();
                    keys.push(key);
                }
            }
        }
        Ok(keys)
    }

    async fn search(
        &self,
        _scope: &Scope,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResult>, StateError> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_state_store_basic() {
        let dir = tempdir().unwrap();
        let store = FileStateStore::new(dir.path().to_path_buf());
        let scope = Scope::Global;
        let key = "test_key";
        let val = serde_json::json!({"foo": "bar"});

        // Read non-existent key
        let res = store.read(&scope, key).await.unwrap();
        assert!(res.is_none());

        // Write value
        store.write(&scope, key, val.clone()).await.unwrap();

        // Read key
        let res = store.read(&scope, key).await.unwrap();
        assert_eq!(res, Some(val));

        // List keys
        let keys = store.list(&scope, "test").await.unwrap();
        assert_eq!(keys, vec!["test_key"]);

        // Delete key
        store.delete(&scope, key).await.unwrap();

        // Read key again
        let res = store.read(&scope, key).await.unwrap();
        assert!(res.is_none());
    }
}
