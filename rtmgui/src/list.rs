//! The item list: one row per todo item, laid out like the web app's `renderItems`.

use chrono::{Datelike, NaiveDate};
use eframe::egui::{
    self, Color32, FontId, Frame, Margin, Sense, Shape, Stroke, StrokeKind, Ui,
    text::{LayoutJob, TextFormat},
};
use todotxt::TodoItem;

use crate::config::AppConfig;
use crate::theme::{self, Palette};

const DATE_FORMAT: &str = "%Y-%m-%d";

/// The file indices of the items to show, filtered and sorted like `rtmapp`'s `get_items`: items
/// without a due date first, then by due date, then by priority (none last). One deliberate
/// difference: dates run earliest first, and `reverse_sort` gives latest first (`rtmapp` has it the
/// other way round).
pub fn visible(items: &[TodoItem], config: &AppConfig, today: NaiveDate) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..items.len())
        .filter(|&i| {
            let item = &items[i];
            (config.show_future_items || item.due.is_none_or(|due| due <= today))
                && (config.show_completed_items || !item.done)
                && !(config.hide_no_date && item.due.is_none())
        })
        .collect();
    // Stable, so equal keys keep file order. `None` sorts before `Some`: undated items first.
    indices.sort_by_key(|&i| {
        let item = &items[i];
        let date = item.due.map(|due| {
            let days = due.num_days_from_ce();
            if config.reverse_sort { -days } else { days }
        });
        (date, item.priority.priority.unwrap_or(u8::MAX))
    });
    indices
}

/// Rows for `visible`, the file indices of the items to show, in that order. Returns the file index
/// of the item whose checkbox was clicked this frame.
pub fn show(ui: &mut Ui, p: &Palette, items: &[TodoItem], visible: &[usize]) -> Option<usize> {
    let mut clicked = None;
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, &index) in visible.iter().enumerate() {
                let item = &items[index];
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
                            if checkbox(ui, p, item.done, index) {
                                clicked = Some(index);
                            }
                            ui.add_space(10.0);
                            let wrap = ui.available_width() - 8.0;
                            // Labels inside a horizontal layout do not wrap unless told to.
                            ui.add(
                                egui::Label::new(row_job(p, item, wrap))
                                    .wrap_mode(egui::TextWrapMode::Wrap),
                            );
                        });
                    });
                if i + 1 < visible.len() {
                    let rect = row.response.rect;
                    ui.painter().hline(
                        rect.x_range(),
                        rect.bottom() - 0.5,
                        Stroke::new(1.0, p.border),
                    );
                }
            }
        });
    clicked
}

/// Painted, because the ☐ and ☑ glyphs are missing from most fonts (see the spike's findings).
/// Returns whether it was clicked. `index` is the item's file index, for a stable widget id.
fn checkbox(ui: &mut Ui, p: &Palette, done: bool, index: usize) -> bool {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
    // The CSS button is padded around its glyph: a larger hit area that leaves the layout alone.
    let response = ui
        .interact(
            rect.expand(3.0),
            ui.make_persistent_id(("toggle", index)),
            Sense::click(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(if done { "Uncomplete" } else { "Complete" });
    // CSS: grey turns green on hover, and green turns grey.
    let (normal, hovered) = if done {
        (theme::GREEN, p.done_text)
    } else {
        (p.done_text, theme::GREEN)
    };
    let color = if response.hovered() { hovered } else { normal };
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
    response.clicked()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 28).unwrap()
    }

    /// Past, today, future and undated items, done and not, with and without priority.
    fn fixture() -> Vec<TodoItem> {
        [
            "Undated",                                            // 0
            "Past due:2026-10-01",                                // 1
            "Today due:2026-10-28",                               // 2
            "Future due:2026-11-15",                              // 3
            "x 2026-10-20 2026-10-19 Done past due:2026-10-02",   // 4
            "x 2026-10-20 2026-10-19 Done undated",               // 5
            "(A) Today high due:2026-10-28",                      // 6
            "x 2026-10-20 2026-10-19 Done future due:2026-12-01", // 7
            "(B) Undated with priority",                          // 8
        ]
        .iter()
        .map(|line| line.parse().unwrap())
        .collect()
    }

    fn config(bits: u8) -> AppConfig {
        AppConfig {
            file_name: None,
            show_completed_items: bits & 1 != 0,
            show_future_items: bits & 2 != 0,
            hide_no_date: bits & 4 != 0,
            reverse_sort: bits & 8 != 0,
        }
    }

    const ALL: u8 = 1 | 2;

    /// `rtmapp`'s filter, copied verbatim from `get_items` in `rtmapp/src-tauri/src/lib.rs`.
    fn rtmapp_shows(item: &TodoItem, config: &AppConfig, today: NaiveDate) -> bool {
        !(item.due.map_or(false, |d| d > today) && !config.show_future_items)
            && !(item.done && !config.show_completed_items)
            && !(config.hide_no_date && item.due.is_none())
    }

    #[test]
    fn filters_match_rtmapp_for_every_flag_combination() {
        let items = fixture();
        for bits in 0..16 {
            let config = config(bits);
            let mut shown = visible(&items, &config, today());
            shown.sort();
            let expected: Vec<usize> = (0..items.len())
                .filter(|&i| rtmapp_shows(&items[i], &config, today()))
                .collect();
            assert_eq!(shown, expected, "flags {bits:04b}");
        }
    }

    #[test]
    fn default_filters_hide_done_and_future_but_keep_today() {
        assert_eq!(visible(&fixture(), &config(0), today()), [8, 0, 1, 6, 2]);
    }

    #[test]
    fn hide_no_date_hides_undated_items() {
        assert_eq!(visible(&fixture(), &config(4), today()), [1, 6, 2]);
    }

    #[test]
    fn sorts_undated_first_then_earliest_due_then_priority() {
        // Undated: 8 (B) first, then 0 and 5 in file order. 6 (A) comes before 2 on the same date.
        assert_eq!(
            visible(&fixture(), &config(ALL), today()),
            [8, 0, 5, 1, 4, 6, 2, 3, 7]
        );
    }

    #[test]
    fn reverse_sort_puts_latest_due_first() {
        assert_eq!(
            visible(&fixture(), &config(ALL | 8), today()),
            [8, 0, 5, 7, 3, 6, 2, 4, 1]
        );
    }
}
