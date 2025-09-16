use chrono::{Datelike, Local};
use eframe::egui;
use todotxt::{TodoItem, TodoLibrary};

struct TodoApp {
    lib: TodoLibrary,
    new_item: String,
    file_name: String,
    show_completed_items: bool,
    show_future_items: bool,
    reverse_sort: bool,
}

impl TodoApp {
    fn new(_cc: &eframe::CreationContext<'_>, file_name: String) -> Self {
        let mut lib = TodoLibrary::new(file_name.clone());
        lib.load().unwrap_or(());
        Self {
            lib,
            new_item: String::new(),
            file_name,
            show_completed_items: false,
            show_future_items: false,
            reverse_sort: false,
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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Rust Todo.txt Manager");

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

            ui.checkbox(&mut self.show_completed_items, "Show completed items");
            ui.checkbox(&mut self.show_future_items, "Show future items");
            if ui
                .button(if self.reverse_sort {
                    "Normal Order"
                } else {
                    "Reverse Order"
                })
                .clicked()
            {
                self.reverse_sort = !self.reverse_sort;
            }

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
                        if ui.button("Complete").clicked() {
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

            ui.separator();

            ui.label(format!(
                "Total items: {} (file: {})",
                self.lib.item_count(),
                self.file_name
            ));
        });
    }
}

fn main() -> eframe::Result<()> {
    // // Force X11 backend to avoid Wayland errors on some systems
    // unsafe {
    //     std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    // }

    let file_name = std::env::var("TODOTXT").unwrap_or_else(|_| "todo.txt".to_string());
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        &format!("Rust Todo.txt Manager - {}", file_name),
        options,
        Box::new(move |cc| Box::new(TodoApp::new(cc, file_name.clone()))),
    )
}
