use chrono::{Local, NaiveDate};
use eframe::egui;
use todotxt::{TodoItem, TodoLibrary};

struct TodoApp {
    lib: TodoLibrary,
    new_task: String,
    file_name: String,
    show_completed: bool,
    show_future_tasks: bool,
}

impl TodoApp {
    fn new(_cc: &eframe::CreationContext<'_>, file_name: String) -> Self {
        let mut lib = TodoLibrary::new(file_name.clone());
        lib.load().unwrap_or(());
        Self {
            lib,
            new_task: String::new(),
            file_name,
            show_completed: false,
            show_future_tasks: false,
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
                ui.label("New Task:");
                ui.text_edit_singleline(&mut self.new_task);
                if ui.button("Add").clicked() && !self.new_task.is_empty() {
                    if let Ok(item) = self.new_task.parse::<TodoItem>() {
                        self.lib.add_item(item);
                        self.new_task.clear();
                        self.save();
                    } else {
                        eprintln!("Parse error");
                    }
                }
            });

            ui.separator();

            ui.checkbox(&mut self.show_completed, "Show completed tasks");
            ui.checkbox(&mut self.show_future_tasks, "Show future tasks");

            // List tasks
            egui::ScrollArea::vertical().show(ui, |ui| {
                let items = self.lib.list_items();
                let mut to_complete = None;
                let today = Local::now().date_naive();
                for (i, item) in items.iter().enumerate() {
                    if !self.show_future_tasks && item.due.map_or(false, |d| d > today) {
                        continue;
                    }
                    if !self.show_completed && item.done {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Complete").clicked() {
                            to_complete = Some(i);
                        }
                        ui.label(format!("{}. {}", i + 1, item));
                    });
                }
                if let Some(idx) = to_complete {
                    self.lib
                        .complete_item(idx)
                        .unwrap_or_else(|| eprintln!("Complete failed"));
                    self.save();
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
