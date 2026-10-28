use eframe::egui;

struct App;

impl eframe::App for App {
    fn ui(&mut self, _ui: &mut egui::Ui, _frame: &mut eframe::Frame) {}
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rusty Todo.txt Manager")
            .with_inner_size([900.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native("rtmgui", options, Box::new(|_cc| Ok(Box::new(App))))
}
