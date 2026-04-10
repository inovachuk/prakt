use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;

trait Storage {
    fn add_file(&mut self, path: String, tags: Vec<String>) -> Result<()>;
    fn get_files(&self, tags: Vec<String>) -> Result<Vec<String>>;
}

#[derive(Serialize, Deserialize, Default)]
struct JsonData {
    files: HashMap<String, HashSet<String>>,
}

struct JsonStorage {
    path: PathBuf,
    data: JsonData,
}

impl JsonStorage {
    fn new(path: PathBuf) -> Result<Self> {
        let data = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            JsonData::default()
        };
        Ok(Self { path, data })
    }

    fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}

impl Storage for JsonStorage {
    fn add_file(&mut self, path: String, tags: Vec<String>) -> Result<()> {
        let entry = self.data.files.entry(path).or_default();
        for tag in tags {
            entry.insert(tag);
        }
        self.save()?;
        Ok(())
    }

    fn get_files(&self, tags: Vec<String>) -> Result<Vec<String>> {
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

struct SqliteStorage {
    conn: Connection,
}

impl SqliteStorage {
    fn new(path: PathBuf) -> Result<Self> {
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
    fn add_file(&mut self, path: String, tags: Vec<String>) -> Result<()> {
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

    fn get_files(&self, tags: Vec<String>) -> Result<Vec<String>> {
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

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Add {
        #[arg(long)]
        path: String,
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    Get {
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
}

fn init_storage(env_var: &str) -> Result<Box<dyn Storage>> {
    let parts: Vec<&str> = env_var.splitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid FILES_INDEX_PATH format");
    }

    let storage_type = parts[0];
    let path = PathBuf::from(parts[1]);

    match storage_type {
        "json" => Ok(Box::new(JsonStorage::new(path)?)),
        "sqlite" => Ok(Box::new(SqliteStorage::new(path)?)),
        _ => anyhow::bail!("Unknown storage type"),
    }
}

fn main() -> Result<()> {
    let env_var = env::var("FILES_INDEX_PATH")
        .context("FILES_INDEX_PATH environment variable not set")?;

    let mut storage = init_storage(&env_var)?;
    let cli = Cli::parse();

    match cli.command {
        Commands::Add { path, tags } => {
            storage.add_file(path, tags)?;
        }
        Commands::Get { tags } => {
            let files = storage.get_files(tags)?;
            for file in files {
                println!("{}", file);
            }
        }
    }

    Ok(())
}