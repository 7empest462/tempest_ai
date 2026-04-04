use rusqlite::Connection;
use anyhow::Result;
use std::path::PathBuf;
use crate::crypto::{encrypt_history, decrypt_history};
use std::fs;

pub struct MemoryStore {
    conn: Connection,
    passphrase: String,
}

impl MemoryStore {
    pub fn new(passphrase: String) -> Result<Self> {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("tempest_ai");
        fs::create_dir_all(&path)?;
        path.push("brain.db");
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            if !path.exists() {
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .mode(0o600)
                    .open(&path);
            }
        }

        let conn = Connection::open(&path)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS memories (
                topic TEXT PRIMARY KEY,
                data BLOB NOT NULL,
                tags TEXT,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // 🛡️ MIGRATION: Add tags if it doesn't exist for legacy databases
        if let Err(_) = conn.execute("SELECT tags FROM memories LIMIT 1", []) {
            let _ = conn.execute("ALTER TABLE memories ADD COLUMN tags TEXT", []);
        }

        Ok(MemoryStore { conn, passphrase })
    }

    pub fn store(&self, topic: &str, content: &str, tags: Option<Vec<String>>) -> Result<()> {
        let encrypted = encrypt_history(content.as_bytes(), &self.passphrase)?;
        let tag_str = tags.map(|t| t.join(", "));
        
        self.conn.execute(
            "INSERT INTO memories (topic, data, tags, updated_at) VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(topic) DO UPDATE SET data=excluded.data, tags=excluded.tags, updated_at=CURRENT_TIMESTAMP",
            rusqlite::params![topic, encrypted, tag_str],
        )?;
        Ok(())
    }

    pub fn recall(&self, keyword: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare("SELECT topic, data FROM memories WHERE topic LIKE ?1 OR tags LIKE ?1 ORDER BY updated_at DESC")?;
        let search = format!("%{}%", keyword);
        let rows = stmt.query_map(rusqlite::params![search], |row| {
            let topic: String = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            Ok((topic, data))
        })?;

        let mut results = Vec::new();
        for r in rows {
            let (t, encrypted_data) = r?;
            if let Ok(decrypted) = decrypt_history(&encrypted_data, &self.passphrase) {
                if let Ok(content_str) = String::from_utf8(decrypted) {
                    results.push((t, content_str));
                }
            }
        }
        Ok(results)
    }

}
