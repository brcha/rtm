//! The item list: one row per todo item, laid out like the web app's `renderItems`.

use std::ops::Range;

use chrono::{Datelike, NaiveDate};
use eframe::egui::{
    self, Color32, FontId, Frame, Galley, Margin, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui,
    epaint::TextShape,
    text::{CCursor, LayoutJob, TextFormat},
};
use todotxt::{TodoContext, TodoItem, TodoProject};

use crate::config::AppConfig;
use crate::theme::{self, Palette};

const DATE_FORMAT: &str = "%Y-%m-%d";

/// The file indices of the items to show, filtered and sorted like the Tauri app's `get_items`:
/// items without a due date first, then by due date, then by priority (none last). One deliberate
/// difference: dates run earliest first, and `reverse_sort` gives latest first (the Tauri app had
/// it the other way round).
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

/// What a click on a row asks for, by the item's index in the file.
pub enum RowAction {
    /// The checkbox: complete or uncomplete.
    Toggle(usize),
    /// The text: open the edit dialog.
    Edit(usize),
    /// A `+project` in the text: toggle it in the filter.
    Project(TodoProject),
    /// A `@context` in the text: toggle it in the filter.
    Context(TodoContext),
}

/// A `+project` or `@context` in a row's text, by its index in the item's list, or a link in the
/// description, by its byte range there.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RowTag {
    Project(usize),
    Context(usize),
    Link { start: usize, end: usize },
}

/// Where a tag or link is in a row's text: its characters (`+name`, without the space before
/// it).
#[derive(Debug)]
struct TagSpan {
    tag: RowTag,
    chars: Range<usize>,
}

/// Whether two tag names are the same tag: case-insensitive, as in `todotxt`.
pub fn same_tag(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Rows for `visible`, the file indices of the items to show, in that order, with the tags in the
/// selection (`projects`, `contexts`) in bold. Returns what was clicked this frame.
pub fn show(
    ui: &mut Ui,
    p: &Palette,
    items: &[TodoItem],
    visible: &[usize],
    projects: &[TodoProject],
    contexts: &[TodoContext],
) -> Option<RowAction> {
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
                            if checkbox(ui, p, item, index) {
                                clicked = Some(RowAction::Toggle(index));
                            }
                            ui.add_space(10.0);
                            // Filled in below once the label's rect is known.
                            let hover_bg = ui.painter().add(Shape::Noop);
                            let wrap = ui.available_width() - 8.0;
                            let (job, tags) = row_job(p, item, wrap, projects, contexts);
                            // Labels inside a horizontal layout do not wrap unless told to. Not
                            // selectable, or a click would start a text selection instead. Laid
                            // out here and painted below, as `Label::ui` does, so that a click can
                            // be mapped to the tag under the pointer.
                            let (pos, galley, text) = egui::Label::new(job)
                                .wrap_mode(egui::TextWrapMode::Wrap)
                                .selectable(false)
                                .sense(Sense::click())
                                .layout_in_ui(ui);
                            text.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::Label,
                                    ui.is_enabled(),
                                    galley.text(),
                                )
                            });
                            let text = text.on_hover_cursor(egui::CursorIcon::PointingHand);
                            let hovered =
                                (text.hover_pos()).and_then(|at| tag_at(&galley, pos, at, &tags));
                            if ui.is_rect_visible(text.rect) {
                                // The hovered tag is underlined glyph by glyph: `LayoutJob`
                                // merges it with the space before it into one section. Links
                                // are always underlined, through their format.
                                let underlined = hovered.and_then(|span| match span.tag {
                                    RowTag::Project(_) => Some((span, p.project)),
                                    RowTag::Context(_) => Some((span, p.context)),
                                    RowTag::Link { .. } => None,
                                });
                                if let Some((span, color)) = underlined {
                                    for glyph in glyph_rects(&galley, span.chars.clone()) {
                                        let glyph = glyph.translate(pos.to_vec2());
                                        ui.painter().hline(
                                            glyph.x_range(),
                                            glyph.bottom() - 1.5,
                                            Stroke::new(1.0, color),
                                        );
                                    }
                                }
                                // `Label::ui`'s underline while the text has keyboard focus.
                                let color = ui.style().interact(&text).text_color();
                                let underline = if text.has_focus() || text.highlighted() {
                                    Stroke::new(1.0, color)
                                } else {
                                    Stroke::NONE
                                };
                                ui.painter().add(
                                    TextShape::new(pos, galley, color).with_underline(underline),
                                );
                            }
                            if text.hovered() {
                                // CSS `.item-text:hover`: 3 % black behind the text.
                                ui.painter().set(
                                    hover_bg,
                                    Shape::rect_filled(
                                        text.rect.expand(2.0),
                                        2.0,
                                        Color32::from_black_alpha(8),
                                    ),
                                );
                            }
                            if text.clicked() {
                                match hovered.map(|span| span.tag) {
                                    Some(RowTag::Project(i)) => {
                                        clicked =
                                            Some(RowAction::Project(item.projects[i].clone()));
                                    }
                                    Some(RowTag::Context(i)) => {
                                        clicked =
                                            Some(RowAction::Context(item.contexts[i].clone()));
                                    }
                                    // Handed to the OS's handler; the item is left alone.
                                    Some(RowTag::Link { start, end }) => ui.ctx().open_url(
                                        egui::OpenUrl::new_tab(&item.description[start..end]),
                                    ),
                                    None => clicked = Some(RowAction::Edit(index)),
                                }
                            }
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

/// Painted, because the ☐ and ☑ glyphs are missing from most fonts (see "Look" in
/// `rtmapp/AGENTS.md`). Returns whether it was clicked. `index` is the item's file index, for a
/// stable widget id.
fn checkbox(ui: &mut Ui, p: &Palette, item: &TodoItem, index: usize) -> bool {
    let done = item.done;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::hover());
    // The CSS button is padded around its glyph: a larger hit area that leaves the layout alone.
    let response = ui.interact(
        rect.expand(3.0),
        ui.make_persistent_id(("toggle", index)),
        Sense::click(),
    );
    // A painted box has no accessible name; screen readers (and the UI tests) find it by the item.
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, true, done, &item.description)
    });
    let response = response
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
/// item is struck through entirely, as the CSS `text-decoration` on the whole text box does. Tags in
/// the selection (`projects`, `contexts`) are bold; links in the description are underlined in the
/// link colour. Also returns where the tags and links are.
fn row_job(
    p: &Palette,
    item: &TodoItem,
    wrap: f32,
    projects: &[TodoProject],
    contexts: &[TodoContext],
) -> (LayoutJob, Vec<TagSpan>) {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let main = if item.done { p.done_text } else { p.text };
    let mut chars = 0;
    // Appends `text` and returns its characters.
    let mut add = |job: &mut LayoutJob, text: &str, font: FontId, color: Color32| {
        let mut format = TextFormat::simple(font, color);
        if item.done {
            format.strikethrough = Stroke::new(1.0, color);
        }
        job.append(text, 0.0, format);
        let start = chars;
        chars += text.chars().count();
        start..chars
    };

    if let Some(n) = item.priority.priority {
        let color = theme::priority_color(n).unwrap_or(main);
        let letter = char::from(b'A' + n.min(25));
        add(&mut job, &format!("({letter}) "), theme::bold(11.9), color);
    }
    let mut tags = Vec::new();
    let description = &item.description;
    let mut plain = 0;
    for link in links(description) {
        add(
            &mut job,
            &description[plain..link.start],
            FontId::proportional(14.0),
            main,
        );
        let chars = add(
            &mut job,
            &description[link.clone()],
            FontId::proportional(14.0),
            p.link,
        );
        // The link colour differs from the text's, so `append` gave the link its own section.
        if let Some(section) = job.sections.last_mut() {
            section.format.underline = Stroke::new(1.0, p.link);
        }
        let tag = RowTag::Link {
            start: link.start,
            end: link.end,
        };
        tags.push(TagSpan { tag, chars });
        plain = link.end;
    }
    add(
        &mut job,
        &description[plain..],
        FontId::proportional(14.0),
        main,
    );
    let projects = (item.projects.iter().enumerate()).map(|(i, project)| {
        let selected = projects.iter().any(|s| same_tag(&s.name, &project.name));
        (
            RowTag::Project(i),
            format!("+{}", project.name),
            selected,
            p.project,
        )
    });
    let contexts = (item.contexts.iter().enumerate()).map(|(i, context)| {
        let selected = contexts.iter().any(|s| same_tag(&s.name, &context.name));
        (
            RowTag::Context(i),
            format!("@{}", context.name),
            selected,
            p.context,
        )
    });
    for (tag, text, selected, color) in projects.chain(contexts) {
        let font = if selected {
            theme::bold(11.9)
        } else {
            FontId::proportional(11.9)
        };
        add(&mut job, " ", FontId::proportional(11.9), color);
        let chars = add(&mut job, &text, font, color);
        tags.push(TagSpan { tag, chars });
    }
    if let Some(due) = item.due {
        add(
            &mut job,
            &format!(" due:{}", due.format(DATE_FORMAT)),
            FontId::proportional(11.2),
            p.due,
        );
    }
    if let Some(recurrence) = &item.recurrence {
        add(
            &mut job,
            &format!(" rec:{recurrence}"),
            FontId::proportional(11.2),
            p.recurrence,
        );
    }
    if let Some(threshold) = item.threshold {
        add(
            &mut job,
            &format!(" t:{}", threshold.format(DATE_FORMAT)),
            FontId::proportional(11.2),
            p.threshold,
        );
    }
    (job, tags)
}

/// What a link starts with, in any case. `file://` is left out: the file may come from a shared
/// repository, and a click would hand a local path to the OS to open or run.
const LINK_SCHEMES: [&str; 3] = ["http://", "https://", "mailto:"];

/// The byte ranges of the links in `text`. A link starts with one of `LINK_SCHEMES` at the start
/// of a word or after a character that is not an ASCII letter or digit (`(https://…`,
/// `url:https://…`, whose `url:` stays outside), and ends with the word, less trailing
/// punctuation; a trailing `)` goes only while the link has more `)` than `(`, so
/// `…/Rust_(language)` keeps its own (GitHub's autolink rule). A scheme alone is no link.
// shortcut: one link per word, so two URLs joined by a comma make one link; scan the rest of the
// word if that turns up in real lists.
fn links(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    for word in text.split(char::is_whitespace) {
        // `split` yields slices of `text`, so the pointers give the word's offset.
        let offset = word.as_ptr() as usize - text.as_ptr() as usize;
        let mut before = None;
        let start = word.char_indices().find_map(|(i, c)| {
            let starts_here = !before.is_some_and(|b: char| b.is_ascii_alphanumeric());
            before = Some(c);
            let scheme = (LINK_SCHEMES.iter()).find(|scheme| {
                (word.get(i..i + scheme.len())).is_some_and(|w| w.eq_ignore_ascii_case(scheme))
            });
            scheme
                .filter(|_| starts_here)
                .map(|scheme| (i, scheme.len()))
        });
        let Some((start, scheme)) = start else {
            continue;
        };
        let mut end = word.len();
        while let Some(last) = word[start..end].chars().next_back() {
            let link = &word[start..end];
            let trailing = match last {
                '.' | ',' | ':' | ';' | '!' | '?' | '\'' | '"' | '*' | '_' | '~' | ']' | '>' => {
                    true
                }
                ')' => link.matches(')').count() > link.matches('(').count(),
                _ => false,
            };
            if !trailing {
                break;
            }
            end -= last.len_utf8();
        }
        if end > start + scheme {
            found.push(offset + start..offset + end);
        }
    }
    found
}

/// The tag whose glyphs are under `at`, in `galley` painted at `origin`. Checked glyph by glyph, so
/// that the space around the text (after a short line, between rows) hits no tag.
fn tag_at<'a>(galley: &Galley, origin: Pos2, at: Pos2, tags: &'a [TagSpan]) -> Option<&'a TagSpan> {
    let at = (at - origin).to_pos2();
    (tags.iter()).find(|span| glyph_rects(galley, span.chars.clone()).any(|r| r.contains(at)))
}

/// The rectangles of the characters `chars` in `galley`, relative to its origin, as high as their
/// rows.
fn glyph_rects(galley: &Galley, chars: Range<usize>) -> impl Iterator<Item = Rect> + '_ {
    chars.filter_map(|c| {
        let start = galley.pos_from_cursor(CCursor::new(c));
        let end = galley.pos_from_cursor(CCursor::new(c + 1));
        // The end of the last character on a row is on that row; skip anything else.
        (start.min.y == end.min.y)
            .then(|| Rect::from_min_max(start.min, egui::pos2(end.min.x, start.max.y)))
    })
}

#[cfg(test)]
#[path = "tests/list_filter_tests.rs"]
mod filter_tests;

#[cfg(test)]
#[path = "tests/list_rows_tests.rs"]
mod rows_tests;

#[cfg(test)]
#[path = "tests/list_links_tests.rs"]
mod links_tests;
