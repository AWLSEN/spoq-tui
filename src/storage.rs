use crate::state::{Task, Thread};
use color_eyre::{eyre::WrapErr, Result};
use std::fs;
use std::path::PathBuf;

/// Get the base data directory for the application
pub fn get_data_dir() -> Result<PathBuf> {
    let data_dir = PathBuf::from("data");
    if !data_dir.exists() {
        fs::create_dir(&data_dir).wrap_err("Failed to create data directory")?;
    }
    Ok(data_dir)
}

/// Get the threads directory
pub fn get_threads_dir() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    let threads_dir = data_dir.join("threads");
    if !threads_dir.exists() {
        fs::create_dir(&threads_dir).wrap_err("Failed to create threads directory")?;
    }
    Ok(threads_dir)
}

/// Get the tasks directory
pub fn get_tasks_dir() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    let tasks_dir = data_dir.join("tasks");
    if !tasks_dir.exists() {
        fs::create_dir(&tasks_dir).wrap_err("Failed to create tasks directory")?;
    }
    Ok(tasks_dir)
}

/// Save threads to JSON file
pub fn save_threads(threads: &[Thread]) -> Result<()> {
    let threads_dir = get_threads_dir()?;
    let file_path = threads_dir.join("threads.json");
    let json = serde_json::to_string_pretty(threads)
        .wrap_err("Failed to serialize threads")?;
    fs::write(&file_path, json)
        .wrap_err(format!("Failed to write threads to {:?}", file_path))?;
    Ok(())
}

/// Load threads from JSON file
pub fn load_threads() -> Result<Vec<Thread>> {
    let threads_dir = get_threads_dir()?;
    let file_path = threads_dir.join("threads.json");

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let json = fs::read_to_string(&file_path)
        .wrap_err(format!("Failed to read threads from {:?}", file_path))?;
    let threads = serde_json::from_str(&json)
        .wrap_err("Failed to deserialize threads")?;
    Ok(threads)
}

/// Save tasks to JSON file
pub fn save_tasks(tasks: &[Task]) -> Result<()> {
    let tasks_dir = get_tasks_dir()?;
    let file_path = tasks_dir.join("tasks.json");
    let json = serde_json::to_string_pretty(tasks)
        .wrap_err("Failed to serialize tasks")?;
    fs::write(&file_path, json)
        .wrap_err(format!("Failed to write tasks to {:?}", file_path))?;
    Ok(())
}

/// Load tasks from JSON file
pub fn load_tasks() -> Result<Vec<Task>> {
    let tasks_dir = get_tasks_dir()?;
    let file_path = tasks_dir.join("tasks.json");

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let json = fs::read_to_string(&file_path)
        .wrap_err(format!("Failed to read tasks from {:?}", file_path))?;
    let tasks = serde_json::from_str(&json)
        .wrap_err("Failed to deserialize tasks")?;
    Ok(tasks)
}

/// Initialize all storage directories
pub fn init_storage() -> Result<()> {
    get_data_dir()?;
    get_threads_dir()?;
    get_tasks_dir()?;
    Ok(())
}
