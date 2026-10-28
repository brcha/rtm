//! The item list: one row per todo item, laid out like the web app's `renderItems`.

use eframe::egui::{
    self, Color32, FontId, Frame, Margin, Sense, Shape, Stroke, StrokeKind, Ui,
    text::{LayoutJob, TextFormat},
};
use todotxt::TodoItem;

use crate::theme::{self, Palette};

const DATE_FORMAT: &str = "%Y-%m-%d";

/// Read-only rows, in file order.
pub fn show(ui: &mut Ui, p: &Palette, items: &[TodoItem]) {
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, item) in items.iter().enumerate() {
                let fill = if item.done {
                    p.done_bg
                } else {
                    Color32::TRANSPARENT
                };
                let row = Frame::new()
                    .fill(fill)
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        // CSS: the row is as high as its 25 px text box plus the padding.
                        ui.set_min_height(25.0);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            checkbox(ui, p, item.done);
                            ui.add_space(10.0);
                            let wrap = ui.available_width() - 8.0;
                            // Labels inside a horizontal layout do not wrap unless told to.
                            ui.add(
                                egui::Label::new(row_job(p, item, wrap))
                                    .wrap_mode(egui::TextWrapMode::Wrap),
                            );
                        });
                    });
                if i + 1 < items.len() {
                    let rect = row.response.rect;
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom() - 0.5,
                        Stroke::new(1.0, p.border),
                    );
                }
            }
        });
}

/// Painted, because the ☐ and ☑ glyphs are missing from most fonts (see the spike's findings).
fn checkbox(ui: &mut Ui, p: &Palette, done: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
    let color = if done { theme::GREEN } else { p.done_text };
    ui.painter()
        .rect_stroke(rect, 2.0, Stroke::new(1.3, color), StrokeKind::Inside);
    if done {
        let origin = rect.left_top();
        ui.painter().add(Shape::line(
            vec![
                origin + egui::vec2(3.0, 7.0),
                origin + egui::vec2(6.0, 10.0),
                origin + egui::vec2(11.0, 4.0),
            ],
            Stroke::new(1.6, color),
        ));
    }
}

/// The text of a row: priority, description and the `+project @context due: rec: t:` chips. A done
/// item is struck through entirely, as the CSS `text-decoration` on the whole text box does.
fn row_job(p: &Palette, item: &TodoItem, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let main = if item.done { p.done_text } else { p.text };
    let mut add = |text: String, font: FontId, color: Color32| {
        let mut format = TextFormat::simple(font, color);
        if item.done {
            format.strikethrough = Stroke::new(1.0, color);
        }
        job.append(&text, 0.0, format);
    };

    if let Some(n) = item.priority.priority {
        let color = theme::priority_color(n).unwrap_or(main);
        let letter = char::from(b'A' + n.min(25));
        add(format!("({letter}) "), theme::bold(11.9), color);
    }
    add(item.description.clone(), FontId::proportional(14.0), main);
    for project in &item.projects {
        add(
            format!(" +{}", project.name),
            FontId::proportional(11.9),
            p.project,
        );
    }
    for context in &item.contexts {
        add(
            format!(" @{}", context.name),
            FontId::proportional(11.9),
            p.context,
        );
    }
    if let Some(due) = item.due {
        add(
            format!(" due:{}", due.format(DATE_FORMAT)),
            FontId::proportional(11.2),
            p.due,
        );
    }
    if let Some(recurrence) = &item.recurrence {
        add(
            format!(" rec:{recurrence}"),
            FontId::proportional(11.2),
            p.recurrence,
        );
    }
    if let Some(threshold) = item.threshold {
        add(
            format!(" t:{}", threshold.format(DATE_FORMAT)),
            FontId::proportional(11.2),
            p.threshold,
        );
    }
    job
}
