use eframe::egui::{self, Align, Color32, Frame, Layout, Margin, RichText, Stroke, Ui};

use crate::theme::{self, Palette};

/// Window shell: header, sidebar and an empty item area. The item list and the file handling come
/// in the next commit.
pub struct App;

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        App
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let p = theme::palette(ui);
        egui::Panel::top("header")
            .frame(Frame::NONE)
            .show(ui, |ui| header(ui, p));
        egui::Panel::left("sidebar")
            .exact_size(220.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(p.panel).inner_margin(16.0))
            .show(ui, |ui| {
                sidebar(ui, p);
                // Right border, drawn on the outer edge of the panel's margin.
                let edge = ui.max_rect().expand(16.0);
                ui.painter().vline(
                    edge.right() - 0.5,
                    edge.y_range(),
                    Stroke::new(1.0, p.border),
                );
            });
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg).inner_margin(16.0))
            .show(ui, |ui| content(ui, p));
    }
}

fn header(ui: &mut Ui, p: &Palette) {
    Frame::new()
        .fill(p.header)
        .inner_margin(Margin::symmetric(20, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // CSS: h1 is 1.25rem with a 1.5 line height.
            ui.set_min_height(26.25);
            ui.label(
                RichText::new("Rusty Todo.txt Manager")
                    .font(theme::bold(17.5))
                    .color(Color32::WHITE),
            );
        });
}

fn sidebar(ui: &mut Ui, p: &Palette) {
    ui.spacing_mut().item_spacing.y = 16.0;
    Frame::new()
        .fill(p.info_bg)
        .corner_radius(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("No file loaded").size(11.9).color(p.muted));
        });
    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
        ui.label(RichText::new("Total items: 0").size(11.9).color(p.muted));
    });
}

fn content(ui: &mut Ui, p: &Palette) {
    Frame::new()
        .fill(p.panel)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(6.0)
        .show(ui, |ui| {
            // The list box fills the content area, as in the web app.
            ui.set_min_size(ui.available_size());
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No file loaded. Use 'Load File' to select a todo.txt file.")
                        .color(p.muted),
                );
            });
        });
}
