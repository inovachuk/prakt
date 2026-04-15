#![warn(clippy::missing_errors_doc, clippy::result_large_err)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use filesindex::{JsonStorage, SqliteStorage, Storage};
use std::env;
use std::path::PathBuf;

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

/// # Errors
///
/// Повертає помилку `anyhow::Error`, якщо формат `env_var` недійсний
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

/// # Errors
///
/// Повертає помилку `anyhow::Error`, якщо змінна середовища не знайдена або сталася помилка виконання
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