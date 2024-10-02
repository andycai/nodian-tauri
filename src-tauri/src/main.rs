// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command

use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use log::info;
use arboard::Clipboard;
use std::sync::Mutex;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
// use chrono::NaiveDate;

#[tauri::command]
fn get_root_folder() -> String {
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let root_dir = home_dir.join("nodian");
    if !root_dir.exists() {
        fs::create_dir_all(&root_dir).unwrap();
    }
    root_dir.to_str().unwrap().to_string()
}

#[tauri::command]
fn get_file_tree(path: &str) -> Result<FileNode, String> {
    info!("Getting file tree for path: {}", path);
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }
    let tree = build_file_tree(&path, true);
    info!("File tree: {:?}", tree);
    Ok(tree)
}

fn build_file_tree(path: &PathBuf, is_root: bool) -> FileNode {
    let name = path.file_name().unwrap_or_default().to_str().unwrap_or_default().to_string();
    let is_dir = path.is_dir() || is_root;
    let children = if is_dir {
        fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| build_file_tree(&entry.path(), false))
                    .collect()
            })
            .unwrap_or_else(|e| {
                info!("Error reading directory {}: {}", path.display(), e);
                Vec::new()
            })
    } else {
        Vec::new()
    };

    FileNode {
        name,
        path: path.to_str().unwrap_or_default().to_string(),
        is_dir,
        children,
    }
}

#[tauri::command]
fn read_file(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_file(path: &str, content: &str) -> Result<(), String> {
    fs::write(path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_folder(path: &str) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_file(path: &str) -> Result<(), String> {
    fs::File::create(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn rename_item(old_path: &str, new_path: &str) -> Result<(), String> {
    fs::rename(old_path, new_path).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_item(path: &str) -> Result<(), String> {
    let path = PathBuf::from(path);
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

struct ClipboardState(Mutex<Clipboard>);

#[tauri::command]
fn get_clipboard_content(state: tauri::State<ClipboardState>) -> Result<String, String> {
    let mut clipboard = state.0.lock().unwrap();
    clipboard.get_text().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_clipboard_content(content: String, state: tauri::State<ClipboardState>) -> Result<(), String> {
    let mut clipboard = state.0.lock().unwrap();
    clipboard.set_text(content).map_err(|e| e.to_string())
}

// 日程

#[derive(Debug, Serialize, Deserialize)]
struct Event {
    id: i64,
    title: String,
    description: String,
    date: String,
}

#[tauri::command]
fn get_events(date: String) -> Result<Vec<Event>, String> {
    let conn = Connection::open("events.db").map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            date TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT id, title, description, date FROM events WHERE date = ?")
        .map_err(|e| e.to_string())?;
    let events = stmt
        .query_map(params![date], |row| {
            Ok(Event {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                date: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(events)
}

#[tauri::command]
fn add_event(title: String, description: String, date: String) -> Result<(), String> {
    let conn = Connection::open("events.db").map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO events (title, description, date) VALUES (?, ?, ?)",
        params![title, description, date],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_event(id: i64) -> Result<(), String> {
    let conn = Connection::open("events.db").map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM events WHERE id = ?", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize, Debug)]
struct FileNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Vec<FileNode>,
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .manage(ClipboardState(Mutex::new(Clipboard::new().unwrap())))
        .invoke_handler(tauri::generate_handler![
            get_root_folder,
            get_file_tree,
            read_file,
            write_file,
            create_folder,
            create_file,
            rename_item,
            delete_item,
            get_clipboard_content,
            set_clipboard_content,
            get_events,
            add_event,
            delete_event
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
