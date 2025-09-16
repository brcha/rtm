use chrono::Local;
use eframe::egui;
use todotxt::{TodoItem, TodoLibrary};

struct TodoApp {
    lib: TodoLibrary,
    new_item: String,
    file_name: String,
    show_completed_items: bool,
    show_future_items: bool,
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

            // List tasks
            egui::ScrollArea::vertical().show(ui, |ui| {
                let items = self.lib.list_items();
                let mut to_complete = None;
                let today = Local::now().date_naive();
                for (i, item) in items.iter().enumerate() {
                    if !self.show_future_items && item.due.map_or(false, |d| d > today) {
                        continue;
                    }
                    if !self.show_completed_items && item.done {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Complete").clicked() {
                            to_complete = Some(i);
                        }
                        ui.label(item.to_string());
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
