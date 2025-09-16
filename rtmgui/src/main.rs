use chrono::{Datelike, Local};
use eframe::egui;
use rfd;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use todotxt::{TodoItem, TodoLibrary};

#[derive(Serialize, Deserialize)]
struct AppConfig {
    file_name: Option<String>,
    show_completed_items: bool,
    show_future_items: bool,
    reverse_sort: bool,
}

struct TodoApp {
    lib: TodoLibrary,
    new_item: String,
    file_name: Option<PathBuf>,
    show_completed_items: bool,
    show_future_items: bool,
    reverse_sort: bool,
    config_path: PathBuf,
}

impl TodoApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = Self::config_path();
        let config: AppConfig = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or(AppConfig {
                file_name: None,
                show_completed_items: false,
                show_future_items: false,
                reverse_sort: false,
            });
        let file_path_buf = if let Some(s) = config.file_name.clone() {
            PathBuf::from(s).canonicalize().unwrap()
        } else {
            PathBuf::from("todo.txt")
        };
        let file_path = file_path_buf.to_string_lossy().to_string();
        let mut lib = TodoLibrary::new(file_path.clone());
        let has_file = lib.load().is_ok();
        let actual_file_name = if has_file {
            Some(file_path_buf)
        } else {
            config.file_name.as_ref().map(|s| PathBuf::from(s))
        };
        Self {
            lib,
            new_item: String::new(),
            file_name: actual_file_name.map(|s| PathBuf::from(s)),
            show_completed_items: config.show_completed_items,
            show_future_items: config.show_future_items,
            reverse_sort: config.reverse_sort,
            config_path,
        }
    }

    fn config_path() -> std::path::PathBuf {
        let mut path = dirs::config_dir().unwrap_or_else(|| std::env::temp_dir());
        path.push("rtm");
        std::fs::create_dir_all(&path).unwrap();
        path.push("config.toml");
        path
    }

    fn save_config(&self) {
        let config = AppConfig {
            file_name: self
                .file_name
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
            show_completed_items: self.show_completed_items,
            show_future_items: self.show_future_items,
            reverse_sort: self.reverse_sort,
        };
        if let Ok(toml_str) = toml::to_string(&config) {
            let _ = std::fs::write(&self.config_path, toml_str);
        }
    }

    fn save(&mut self) {
        self.lib
            .save()
            .unwrap_or_else(|e| eprintln!("Save error: {:?}", e));
    }
}

impl eframe::App for TodoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.heading("Rusty Todo.txt Manager");
        });

        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.label(format!(
                "Total items: {} (file: {})",
                self.lib.item_count(),
                self.file_name
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ));
        });

        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            if ui.button("Load").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    let canonical_path = path.canonicalize().unwrap_or(path);
                    let canonical_str = canonical_path.to_string_lossy().to_string();
                    self.lib = TodoLibrary::new(canonical_str);
                    if self.lib.load().is_ok() {
                        self.file_name = Some(canonical_path);
                        self.save_config();
                    } else {
                        eprintln!("Failed to load file");
                    }
                }
            }
            if !self.file_name.is_some() {
                return;
            }

            if ui
                .checkbox(&mut self.show_completed_items, "Show completed items")
                .clicked()
            {
                self.save_config();
            }
            if ui
                .checkbox(&mut self.show_future_items, "Show future items")
                .clicked()
            {
                self.save_config();
            }

            if ui
                .button(if self.reverse_sort {
                    "Normal Order"
                } else {
                    "Reverse Order"
                })
                .clicked()
            {
                self.reverse_sort = !self.reverse_sort;
                self.save_config();
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.file_name.is_some() {
                ui.label("No file loaded. Use 'Load' to select a todo.txt file.");
                return;
            }

            ui.horizontal(|ui| {
                ui.label("New Item:");
                ui.text_edit_singleline(&mut self.new_item);
                if ui.button("Add").clicked() && !self.new_item.is_empty() {
                    if let Ok(item) = self.new_item.parse::<TodoItem>() {
                        self.lib.add_item(item);
                        self.new_item.clear();
                        self.save();
                    } else {
                        eprintln!("Parse error");
                    }
                }
            });

            ui.separator();

            // List tasks
            egui::ScrollArea::vertical().show(ui, |ui| {
                let items = self.lib.list_items();
                let mut sorted_items: Vec<(usize, &TodoItem)> = items.iter().enumerate().collect();
                let today = Local::now().date_naive();
                sorted_items.sort_by_key(|(_, item)| {
                    let priority_key = item.priority.priority.map(|p| p as i32).unwrap_or(26);
                    match item.due {
                        None => (0i32, 0i64, priority_key),
                        Some(d) => {
                            let date_key = if self.reverse_sort {
                                d.num_days_from_ce() as i64
                            } else {
                                -d.num_days_from_ce() as i64
                            };
                            (1i32, date_key, priority_key)
                        }
                    }
                });

                // Apply filters after sorting
                let filtered_items: Vec<_> = sorted_items
                    .into_iter()
                    .filter(|(_, item)| {
                        !(item.due.map_or(false, |d| d > today) && !self.show_future_items)
                            && !(item.done && !self.show_completed_items)
                    })
                    .collect();

                let mut to_complete = None;
                for (original_i, item) in filtered_items {
                    ui.horizontal(|ui| {
                        if !item.done &&"S" ui.button("Complete").clicked() {
                            to_complete = Some(original_i);
                        }
                        ui.label(item.to_string());
                    });
                }
                if let Some(idx) = to_complete {
                    if let Some(_) = self.lib.complete_item(idx) {
                        self.save();
                        // Reload from file to refresh the list (important for recurring tasks)
                        self.lib.load().unwrap_or(());
                    } else {
                        eprintln!("Complete failed");
                    }
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    // // Force X11 backend to avoid Wayland errors on some systems
    // unsafe {
    //     std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    // }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Rusty Todo.txt Manager",
        options,
        Box::new(|cc| Box::new(TodoApp::new(cc))),
    )
}
