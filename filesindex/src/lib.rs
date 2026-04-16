#![warn(clippy::missing_errors_doc, clippy::result_large_err)]

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub trait Storage {
    /// # Errors
    ///
    /// Повертає `StorageError` у випадку проблем із базою даних або файловою системою
    fn add_file(&mut self, path: String, tags: Vec<String>) -> Result<(), StorageError>;

    /// # Errors
    ///
    /// Повертає `StorageError` у випадку проблем із базою даних або файловою системою
    fn get_files(&self, tags: Vec<String>) -> Result<Vec<String>, StorageError>;
}

#[derive(Serialize, Deserialize, Default)]
struct JsonData {
    files: HashMap<String, HashSet<String>>,
}

pub struct JsonStorage {
    path: PathBuf,
    data: JsonData,
}

impl JsonStorage {
    /// # Errors
    ///
    /// Повертає `StorageError` у випадку помилки читання або парсингу JSON
    pub fn new(path: PathBuf) -> Result<Self, StorageError> {
        let data = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            JsonData::default()
        };
        Ok(Self { path, data })
    }

    /// # Errors
    ///
    /// Повертає `StorageError` у випадку помилки запису файлу
    fn save(&self) -> Result<(), StorageError> {
        let content = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}

impl Storage for JsonStorage {
    /// # Errors
    ///
    /// Повертає `StorageError` при помилці збереження файлу
    fn add_file(&mut self, path: String, tags: Vec<String>) -> Result<(), StorageError> {
        let entry = self.data.files.entry(path).or_default();
        for tag in tags {
            entry.insert(tag);
        }
        self.save()?;
        Ok(())
    }

    /// # Errors
    ///
    /// Повертає `StorageError` при неможливості зчитати дані (в даній реалізації завжди Ok)
    fn get_files(&self, tags: Vec<String>) -> Result<Vec<String>, StorageError> {
        let search_tags: HashSet<_> = tags.into_iter().collect();
        let mut result = Vec::new();
        for (file, file_tags) in &self.data.files {
            if search_tags.is_subset(file_tags) {
                result.push(file.clone());
            }
        }
        Ok(result)
    }
}

pub struct SqliteStorage {
    conn: Connection,
}

impl SqliteStorage {
    /// # Errors
    ///
    /// Повертає `StorageError` при неможливості відкрити БД або створити таблиці
    pub fn new(path: PathBuf) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                id INTEGER PRIMARY KEY,
                path TEXT UNIQUE NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS file_tags (
                file_id INTEGER,
                tag_id INTEGER,
                PRIMARY KEY (file_id, tag_id),
                FOREIGN KEY (file_id) REFERENCES files(id),
                FOREIGN KEY (tag_id) REFERENCES tags(id)
            )",
            [],
        )?;
        Ok(Self { conn })
    }
}

impl Storage for SqliteStorage {
    /// # Errors
    ///
    /// Повертає `StorageError` при помилках транзакцій або запитів SQL
    fn add_file(&mut self, path: String, tags: Vec<String>) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;
        
        tx.execute(
            "INSERT OR IGNORE INTO files (path) VALUES (?1)",
            [&path],
        )?;
        
        let file_id: i64 = tx.query_row(
            "SELECT id FROM files WHERE path = ?1",
            [&path],
            |row| row.get(0),
        )?;

        for tag in tags {
            tx.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                [&tag],
            )?;
            let tag_id: i64 = tx.query_row(
                "SELECT id FROM tags WHERE name = ?1",
                [&tag],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO file_tags (file_id, tag_id) VALUES (?1, ?2)",
                [file_id, tag_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// # Errors
    ///
    /// Повертає `StorageError` при помилках виконання SQL запиту
    fn get_files(&self, tags: Vec<String>) -> Result<Vec<String>, StorageError> {
        if tags.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT f.path 
             FROM files f
             JOIN file_tags ft ON f.id = ft.file_id
             JOIN tags t ON ft.tag_id = t.id
             WHERE t.name IN ({})
             GROUP BY f.id
             HAVING COUNT(DISTINCT t.id) = ?",
            placeholders
        );

        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();
        for tag in &tags {
            params.push(tag);
        }
        let tag_count = tags.len() as i64;
        params.push(&tag_count);

        let mut stmt = self.conn.prepare(&query)?;
        let file_iter = stmt.query_map(params.as_slice(), |row| row.get(0))?;

        let mut result = Vec::new();
        for file in file_iter {
            result.push(file?);
        }

        Ok(result)
    }
}