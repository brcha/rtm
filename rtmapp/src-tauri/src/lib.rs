use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use todotxt::{TodoContext, TodoItem, TodoLibrary, TodoPriority, TodoProject};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppConfig {
    file_name: Option<String>,
    show_completed_items: bool,
    show_future_items: bool,
    hide_no_date: bool,
    reverse_sort: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            file_name: None,
            show_completed_items: false,
            show_future_items: false,
            hide_no_date: false,
            reverse_sort: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TodoItemDto {
    pub index: usize,
    pub done: bool,
    pub priority: Option<i32>,
    pub completion_date: Option<String>,
    pub creation_date: Option<String>,
    pub description: String,
    pub projects: Vec<String>,
    pub contexts: Vec<String>,
    pub due: Option<String>,
    pub recurrence: Option<String>,
    pub threshold: Option<String>,
}

impl From<(usize, &TodoItem)> for TodoItemDto {
    fn from((idx, item): (usize, &TodoItem)) -> Self {
        TodoItemDto {
            index: idx,
            done: item.done,
            priority: item.priority.priority.map(|p| p as i32),
            completion_date: item
                .completion_date
                .map(|d| d.format("%Y-%m-%d").to_string()),
            creation_date: item.creation_date.map(|d| d.format("%Y-%m-%d").to_string()),
            description: item.description.clone(),
            projects: item.projects.iter().map(|p| p.name.clone()).collect(),
            contexts: item.contexts.iter().map(|c| c.name.clone()).collect(),
            due: item.due.map(|d| d.format("%Y-%m-%d").to_string()),
            recurrence: item.recurrence.as_ref().map(|r| r.to_string()),
            threshold: item.threshold.map(|t| t.format("%Y-%m-%d").to_string()),
        }
    }
}

pub struct AppState {
    lib: Mutex<Option<TodoLibrary>>,
    config: Mutex<AppConfig>,
    config_path: PathBuf,
}

impl AppState {
    fn new() -> Self {
        let config_path = Self::get_config_path();
        let config: AppConfig = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();

        let mut lib: Option<TodoLibrary> = None;
        if let Some(ref file_name) = config.file_name {
            let path = PathBuf::from(file_name);
            if path.exists() {
                let mut library = TodoLibrary::new(file_name.clone());
                if library.load().is_ok() {
                    lib = Some(library);
                }
            }
        }

        AppState {
            lib: Mutex::new(lib),
            config: Mutex::new(config),
            config_path,
        }
    }

    fn get_config_path() -> PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| std::env::temp_dir());
        path.push("rtm");
        std::fs::create_dir_all(&path).unwrap();
        path.push("config.toml");
        path
    }

    fn save_config(&self) {
        let config = self.config.lock().unwrap();
        if let Ok(toml_str) = toml::to_string(&*config) {
            let _ = std::fs::write(&self.config_path, toml_str);
        }
    }
}

#[tauri::command]
fn load_file(path: String, state: tauri::State<AppState>) -> Result<bool, String> {
    let canonical_path = std::path::Path::new(&path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let canonical_str = canonical_path.to_string_lossy().to_string();

    let mut library = TodoLibrary::new(canonical_str.clone());
    library.load().map_err(|e| e.to_string())?;

    let mut lib_guard = state.lib.lock().unwrap();
    *lib_guard = Some(library);

    let mut config = state.config.lock().unwrap();
    config.file_name = Some(canonical_str);
    drop(config);
    state.save_config();

    Ok(true)
}

#[tauri::command]
fn get_file_name(state: tauri::State<AppState>) -> Option<String> {
    let config = state.config.lock().unwrap();
    config.file_name.clone()
}

#[tauri::command]
fn has_file_loaded(state: tauri::State<AppState>) -> bool {
    let lib_guard = state.lib.lock().unwrap();
    lib_guard.is_some()
}

#[tauri::command]
fn save_file(state: tauri::State<AppState>) -> Result<bool, String> {
    let mut lib_guard = state.lib.lock().unwrap();
    if let Some(ref mut lib) = *lib_guard {
        lib.save().map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("No file loaded".to_string())
    }
}

#[tauri::command]
fn get_items(state: tauri::State<AppState>) -> Vec<TodoItemDto> {
    let lib_guard = state.lib.lock().unwrap();
    if let Some(ref lib) = *lib_guard {
        let today = Local::now().date_naive();
        let config = state.config.lock().unwrap();

        let mut items: Vec<(usize, &TodoItem)> = lib.list_items().iter().enumerate().collect();

        items.sort_by_key(|(_, item)| {
            let priority_key = item.priority.priority.map(|p| p as i32).unwrap_or(26);
            match item.due {
                None => (0i32, 0i64, priority_key),
                Some(d) => {
                    let date_key = if config.reverse_sort {
                        d.num_days_from_ce() as i64
                    } else {
                        -d.num_days_from_ce() as i64
                    };
                    (1i32, date_key, priority_key)
                }
            }
        });

        items
            .into_iter()
            .filter(|(_, item)| {
                !(item.due.map_or(false, |d| d > today) && !config.show_future_items)
                    && !(item.done && !config.show_completed_items)
                    && !(config.hide_no_date && item.due.is_none())
            })
            .map(|item| TodoItemDto::from(item))
            .collect()
    } else {
        vec![]
    }
}

#[tauri::command]
fn get_item_count(state: tauri::State<AppState>) -> usize {
    let lib_guard = state.lib.lock().unwrap();
    if let Some(ref lib) = *lib_guard {
        lib.item_count()
    } else {
        0
    }
}

#[tauri::command]
fn add_item(text: String, state: tauri::State<AppState>) -> Result<bool, String> {
    let item: TodoItem = text.parse().map_err(|_| "Failed to parse item")?;

    let mut lib_guard = state.lib.lock().unwrap();
    if let Some(ref mut lib) = *lib_guard {
        lib.add_item(item);
        lib.save().map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("No file loaded".to_string())
    }
}

#[tauri::command]
fn complete_item(index: usize, state: tauri::State<AppState>) -> Result<bool, String> {
    let mut lib_guard = state.lib.lock().unwrap();
    if let Some(ref mut lib) = *lib_guard {
        if let Some(has_recurrence) = lib.complete_item(index) {
            lib.save().map_err(|e| e.to_string())?;
            lib.load().map_err(|e| e.to_string())?;
            Ok(has_recurrence)
        } else {
            Err("Failed to complete item".to_string())
        }
    } else {
        Err("No file loaded".to_string())
    }
}

#[tauri::command]
fn uncomplete_item(index: usize, state: tauri::State<AppState>) -> Result<bool, String> {
    let mut lib_guard = state.lib.lock().unwrap();
    if let Some(ref mut lib) = *lib_guard {
        if index < lib.items.len() {
            let item = &mut lib.items[index];
            let new_item = item.set_done(false);
            *item = new_item;
            lib.save().map_err(|e| e.to_string())?;
            Ok(true)
        } else {
            Err("Invalid index".to_string())
        }
    } else {
        Err("No file loaded".to_string())
    }
}

#[derive(Serialize, Deserialize)]
pub struct UpdateItemRequest {
    pub index: usize,
    pub description: String,
    pub priority: Option<i32>,
    pub due: Option<String>,
    pub recurrence: Option<String>,
    pub threshold: Option<String>,
    pub projects: Vec<String>,
    pub contexts: Vec<String>,
}

#[tauri::command]
fn update_item(request: UpdateItemRequest, state: tauri::State<AppState>) -> Result<bool, String> {
    let mut lib_guard = state.lib.lock().unwrap();
    if let Some(ref mut lib) = *lib_guard {
        if request.index >= lib.items.len() {
            return Err("Invalid index".to_string());
        }

        let item = &mut lib.items[request.index];

        item.description = request.description;

        if let Some(p) = request.priority {
            item.priority = TodoPriority {
                priority: Some(p as u8),
            };
        } else {
            item.priority = TodoPriority { priority: None };
        }

        item.due = request
            .due
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
        item.threshold = request
            .threshold
            .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

        if let Some(ref rec_str) = request.recurrence {
            if rec_str.is_empty() {
                item.recurrence = None;
            } else {
                item.recurrence = rec_str.parse().ok();
            }
        } else {
            item.recurrence = None;
        }

        item.projects = request
            .projects
            .into_iter()
            .map(|n| TodoProject { name: n })
            .collect();

        item.contexts = request
            .contexts
            .into_iter()
            .map(|n| TodoContext { name: n })
            .collect();

        lib.save().map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Err("No file loaded".to_string())
    }
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> AppConfig {
    let config = state.config.lock().unwrap();
    config.clone()
}

#[tauri::command]
fn save_config(
    show_completed_items: Option<bool>,
    show_future_items: Option<bool>,
    hide_no_date: Option<bool>,
    reverse_sort: Option<bool>,
    state: tauri::State<AppState>,
) -> Result<bool, String> {
    let mut config = state.config.lock().unwrap();
    if let Some(v) = show_completed_items {
        config.show_completed_items = v;
    }
    if let Some(v) = show_future_items {
        config.show_future_items = v;
    }
    if let Some(v) = hide_no_date {
        config.hide_no_date = v;
    }
    if let Some(v) = reverse_sort {
        config.reverse_sort = v;
    }
    drop(config);
    state.save_config();
    Ok(true)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            load_file,
            get_file_name,
            has_file_loaded,
            save_file,
            get_items,
            get_item_count,
            add_item,
            complete_item,
            uncomplete_item,
            update_item,
            get_config,
            save_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
