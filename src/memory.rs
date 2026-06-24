use crate::crypto::{decrypt_history_with_key, derive_key, encrypt_history_with_key};
use miette::{IntoDiagnostic, Result};
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MemoryRecord {
    pub topic: String,
    pub content: String,
    pub tags: Option<String>,
    pub updated_at: String,
}

pub struct MemoryStore {
    conn: Connection,
    passphrase: String,
    derived_key: zeroize::Zeroizing<[u8; 32]>,
}

impl MemoryStore {
    pub fn new(passphrase: String) -> Result<Self> {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("tempest_ai");
        fs::create_dir_all(&path).into_diagnostic()?;
        path.push("brain.db");
        Self::new_with_path(path, passphrase)
    }

    pub fn new_with_path(path: PathBuf, passphrase: String) -> Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            if !path.exists() {
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .mode(0o600)
                    .open(&path);
            }
        }

        let conn = Connection::open(&path).into_diagnostic()?;

        // ⚡ Enable WAL mode for better concurrency
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| miette::miette!("Failed to enable WAL mode: {}", e))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                topic TEXT PRIMARY KEY,
                data BLOB NOT NULL,
                tags TEXT,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .into_diagnostic()?;

        // 🛡️ MIGRATION: Robustly check for 'tags' column using PRAGMA table_info
        let mut has_tags = false;
        if let Ok(mut stmt) = conn.prepare("PRAGMA table_info(memories)")
            && let Ok(mut rows) = stmt.query([])
        {
            while let Ok(Some(row)) = rows.next() {
                if let Ok(name) = row.get::<_, String>(1)
                    && name == "tags"
                {
                    has_tags = true;
                    break;
                }
            }
        }

        if !has_tags {
            let _ = conn.execute("ALTER TABLE memories ADD COLUMN tags TEXT", []);
        }

        let derived_key = derive_key(&passphrase);
        Ok(MemoryStore {
            conn,
            passphrase,
            derived_key,
        })
    }

    pub fn store(&self, topic: &str, content: &str, tags: Option<Vec<String>>) -> Result<()> {
        let encrypted = encrypt_history_with_key(content.as_bytes(), &self.derived_key)?;
        let tag_str = tags.map(|t| t.join(", "));

        self.conn.execute(
            "INSERT INTO memories (topic, data, tags, updated_at) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(topic) DO UPDATE SET data=excluded.data, tags=excluded.tags, updated_at=CURRENT_TIMESTAMP",
            rusqlite::params![topic, encrypted, tag_str],
        ).into_diagnostic()?;
        Ok(())
    }

    pub fn recall(&self, keyword: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT topic, data FROM memories WHERE topic LIKE ?1 OR tags LIKE ?1 ORDER BY updated_at DESC").into_diagnostic()?;
        let search = format!("%{}%", keyword);
        let rows = stmt
            .query_map(rusqlite::params![search], |row| {
                let topic: String = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                Ok((topic, data))
            })
            .into_diagnostic()?;

        let mut results = Vec::new();
        for r in rows {
            let (t, encrypted_data) = r.into_diagnostic()?;
            if let Ok(decrypted) = decrypt_history_with_key(&encrypted_data, &self.derived_key)
                && let Ok(content_str) = String::from_utf8(decrypted)
            {
                results.push((t, content_str));
            }
        }
        Ok(results)
    }

    pub fn recall_latest(&self) -> Result<Option<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT topic, data FROM memories ORDER BY updated_at DESC LIMIT 1")
            .into_diagnostic()?;
        let mut rows = stmt.query([]).into_diagnostic()?;
        if let Some(row) = rows.next().into_diagnostic()? {
            let topic: String = row.get(0).into_diagnostic()?;
            let encrypted_data: Vec<u8> = row.get(1).into_diagnostic()?;
            if let Ok(decrypted) = decrypt_history_with_key(&encrypted_data, &self.derived_key)
                && let Ok(content_str) = String::from_utf8(decrypted)
            {
                return Ok(Some((topic, content_str)));
            }
        }
        Ok(None)
    }

    pub fn list_all(&self) -> Result<Vec<MemoryRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT topic, data, tags, updated_at FROM memories ORDER BY updated_at DESC")
            .into_diagnostic()?;
        let rows = stmt
            .query_map([], |row| {
                let topic: String = row.get(0)?;
                let data: Vec<u8> = row.get(1)?;
                let tags: Option<String> = row.get(2)?;
                let updated_at: String = row.get(3)?;
                Ok((topic, data, tags, updated_at))
            })
            .into_diagnostic()?;

        let mut results = Vec::new();
        for r in rows {
            let (topic, encrypted_data, tags, updated_at) = r.into_diagnostic()?;
            if let Ok(decrypted) = decrypt_history_with_key(&encrypted_data, &self.derived_key)
                && let Ok(content_str) = String::from_utf8(decrypted)
            {
                results.push(MemoryRecord {
                    topic,
                    content: content_str,
                    tags,
                    updated_at,
                });
            }
        }
        Ok(results)
    }

    pub fn passphrase(&self) -> &str {
        &self.passphrase
    }

    pub fn clear_and_repopulate(
        &self,
        records: Vec<(String, String, Option<Vec<String>>)>,
    ) -> Result<()> {
        self.conn
            .execute("DELETE FROM memories", [])
            .map_err(|e| miette::miette!("Failed to clear memories: {}", e))?;
        for (topic, content, tags) in records {
            self.store(&topic, &content, tags)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_store_encryption_and_clear_repopulate() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("brain.db");
        let store = MemoryStore::new_with_path(db_path, "test-passphrase".to_string()).unwrap();

        // 1. Store a memory fact
        store
            .store("fact1", "fact1 content", Some(vec!["tag1".to_string()]))
            .unwrap();
        store
            .store("fact2", "fact2 content", Some(vec!["tag2".to_string()]))
            .unwrap();

        // Verify stored
        let records = store.list_all().unwrap();
        assert_eq!(records.len(), 2);

        // 2. Clear and repopulate
        let new_records = vec![(
            "consolidated_fact".to_string(),
            "consolidated fact content".to_string(),
            Some(vec!["tag1".to_string(), "tag2".to_string()]),
        )];
        store.clear_and_repopulate(new_records).unwrap();

        // Verify clear and repopulate succeeded
        let final_records = store.list_all().unwrap();
        assert_eq!(final_records.len(), 1);
        assert_eq!(final_records[0].topic, "consolidated_fact");
        assert_eq!(final_records[0].content, "consolidated fact content");
        assert_eq!(final_records[0].tags.as_deref(), Some("tag1, tag2"));
    }
}
