//! The date fields of the edit dialog: a text input with a calendar popup below it, styled like the
//! flatpickr popup `rtmapp` uses (`rtmapp/src/assets/flatpickr.min.css`). Typing a date still works,
//! as flatpickr's `allowInput: true` allows. Deliberate differences: the dark theme uses the app's dark
//! palette (flatpickr is light only), a Clear button empties the field, the week starts on the
//! locale's first day (flatpickr: Sunday), and the month is stepped with arrows (no month dropdown).

use std::sync::OnceLock;

use chrono::{Datelike, Months, NaiveDate, Weekday};
use eframe::egui::{
    self, Align2, Color32, FontId, Frame, Key, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, vec2,
};
use egui::{Popup, PopupCloseBehavior, SetOpenCommand};

use crate::theme::{self, Palette};

const DATE_FORMAT: &str = "%Y-%m-%d";

// flatpickr: 307.875 px wide, a 34 px month header, a 28 px weekday row and 39 px round days.
const WIDTH: f32 = 307.875;
const HEADER: f32 = 34.0;
const WEEKDAYS: f32 = 28.0;
const DAY: f32 = 39.0;
const FOOTER: f32 = 34.0;
/// Touchpad scroll distance that counts as one wheel notch on the year.
const POINTS_PER_NOTCH: f32 = 50.0;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// What the user did in the popup.
enum Pick {
    Day(NaiveDate),
    Clear,
}

/// A date text field with the calendar popup. The popup opens when the field is clicked or gets
/// focus, and closes on a pick, Clear, Escape, Tab or a click outside it.
pub fn date_field(ui: &mut Ui, p: &Palette, text: &mut String, hint: &str) -> Response {
    let response = theme::text_input(ui, p, text, hint, 9, p.focus);
    let ctx = ui.ctx().clone();
    let popup = Popup::from_response(&response);
    let popup_id = popup.get_id();
    let view_id = popup_id.with("view");

    let open_now = response.clicked() || response.gained_focus();
    // The month shown: the field's date, or this month. A valid date typed while open moves it.
    let typed = NaiveDate::parse_from_str(text.trim(), DATE_FORMAT).ok();
    if (open_now && !Popup::is_id_open(&ctx, popup_id)) || (response.changed() && typed.is_some()) {
        let date = typed.unwrap_or_else(today);
        ui.data_mut(|d| d.insert_temp(view_id, first_of_month(date)));
    }

    let shown = popup
        .open_memory(open_now.then_some(SetOpenCommand::Bool(true)))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .gap(2.0)
        .frame(frame(p))
        .show(|ui| month(ui, p, view_id, typed));
    // A click in the dialog moves its layer to the top; as a sublayer the popup stays above it.
    if let Some(shown) = &shown {
        ctx.set_sublayer(ui.layer_id(), shown.response.layer_id);
    }
    let picked = shown.and_then(|r| r.inner);
    match picked {
        Some(Pick::Day(date)) => *text = date.format(DATE_FORMAT).to_string(),
        Some(Pick::Clear) => text.clear(),
        None => {}
    }
    if picked.is_some() || (response.lost_focus() && ui.input(|i| i.key_pressed(Key::Tab))) {
        Popup::close_id(&ctx, popup_id);
    } else if response.clicked() {
        // The popup counts a click on its own field as outside and closes; keep it open.
        Popup::open_id(&ctx, popup_id);
    }
    response
}

/// CSS `.flatpickr-calendar`: 5 px corners, a 1 px outline and a soft shadow.
fn frame(p: &Palette) -> Frame {
    Frame::new()
        .fill(p.cal_bg)
        .stroke(Stroke::new(1.0, p.cal_border))
        .corner_radius(5.0)
        .shadow(egui::Shadow {
            offset: [0, 3],
            blur: 13,
            spread: 0,
            color: Color32::from_black_alpha(20),
        })
}

/// The month view: header, weekday row, day grid and the Clear button.
fn month(ui: &mut Ui, p: &Palette, view_id: egui::Id, selected: Option<NaiveDate>) -> Option<Pick> {
    let view = ui
        .data(|d| d.get_temp::<NaiveDate>(view_id))
        .unwrap_or_else(|| first_of_month(today()));
    let mut next_view = view;
    let mut pick = None;
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
    // Everything here senses `Sense::CLICK` without `FOCUSABLE`, so Tab skips the popup and goes
    // from the date field to the next field, as in flatpickr.

    // Header: ‹ Month Year ›, with the year's up and down arrows shown on hover as in flatpickr.
    let (header, _) = ui.allocate_exact_size(vec2(WIDTH, HEADER), Sense::hover());
    let arrow_color = p.cal_header;
    let prev = Rect::from_min_size(header.min, vec2(HEADER, HEADER));
    let next = Rect::from_min_size(header.right_top() - vec2(HEADER, 0.0), vec2(HEADER, HEADER));
    if arrow_button(ui, view_id.with("prev"), prev, arrow_color, -1.0).clicked() {
        next_view = step(view, -1);
    }
    if arrow_button(ui, view_id.with("next"), next, arrow_color, 1.0).clicked() {
        next_view = step(view, 1);
    }
    let painter = ui.painter();
    let size = 18.9; // 135 % of 14 px
    let month_galley = painter.layout_no_wrap(
        MONTHS[view.month0() as usize].to_owned(),
        theme::bold(size),
        p.cal_header,
    );
    let year_galley = painter.layout_no_wrap(
        view.year().to_string(),
        FontId::proportional(size),
        p.cal_header,
    );
    let gap = 6.0;
    let total = month_galley.size().x + gap + year_galley.size().x;
    let left = header.center().x - total / 2.0;
    let top = header.center().y - month_galley.size().y / 2.0;
    let year_pos = Pos2::new(left + month_galley.size().x + gap, top);
    painter.galley(Pos2::new(left, top), month_galley, p.cal_header);
    let year_rect = Rect::from_min_size(year_pos, year_galley.size() + vec2(14.0, 0.0));
    painter.galley(year_pos, year_galley, p.cal_header);
    // The spinner is always there to click; the arrows are drawn only while the year is hovered.
    // The pointer test covers the arrows too (a hover response would not, as they sit on top).
    let year_hovered = ui.rect_contains_pointer(year_rect);
    let arrows = Rect::from_min_max(
        Pos2::new(year_rect.right() - 12.0, year_rect.top()),
        year_rect.right_bottom(),
    );
    let (up, down) = arrows.split_top_bottom_at_fraction(0.5);
    let arrow = year_hovered.then_some(arrow_color);
    if spin_button(ui, view_id.with("year-up"), up, arrow, true).clicked() {
        next_view = step(view, 12);
    }
    if spin_button(ui, view_id.with("year-down"), down, arrow, false).clicked() {
        next_view = step(view, -12);
    }
    // The mouse wheel over the year steps it too: up is the next year.
    if year_hovered {
        // Raw wheel events in notches (the smoothed delta spreads a notch over frames). A wheel
        // sends whole lines; a touchpad sends a stream of points, gathered here until they add up
        // to a notch, so one swipe does not skip many years.
        let notches: f32 = ui.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::MouseWheel { delta, unit, .. } => Some(match unit {
                        egui::MouseWheelUnit::Point => delta.y / POINTS_PER_NOTCH,
                        _ => delta.y,
                    }),
                    _ => None,
                })
                .sum()
        });
        let wheel_id = view_id.with("wheel");
        let total = ui.data(|d| d.get_temp::<f32>(wheel_id)).unwrap_or(0.0) + notches;
        let steps = total.trunc();
        if steps != 0.0 {
            next_view = step(view, 12 * steps.signum() as i32);
        }
        ui.data_mut(|d| d.insert_temp(wheel_id, if steps != 0.0 { 0.0 } else { total }));
    }

    // Weekday row: CSS 90 %, bolder.
    let first = first_weekday_cached();
    let (row, _) = ui.allocate_exact_size(vec2(WIDTH, WEEKDAYS), Sense::hover());
    let column = WIDTH / 7.0;
    let mut weekday = first;
    for i in 0..7 {
        let center = Pos2::new(row.left() + column * (i as f32 + 0.5), row.center().y);
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            short_name(weekday),
            theme::bold(12.6),
            p.cal_weekday,
        );
        weekday = weekday.succ();
    }

    // Day grid: six weeks, days of the other months dimmed.
    let (days, _) = ui.allocate_exact_size(vec2(WIDTH, DAY * 6.0), Sense::hover());
    let today = today();
    for (i, date) in grid(view, first).into_iter().enumerate() {
        let center = Pos2::new(
            days.left() + column * ((i % 7) as f32 + 0.5),
            days.top() + DAY * ((i / 7) as f32 + 0.5),
        );
        let cell = Rect::from_center_size(center, vec2(DAY, DAY));
        let response = ui
            .interact(cell, view_id.with(("day", i)), Sense::CLICK)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let is_selected = Some(date) == selected;
        let (fill, ring, text) = if is_selected {
            (p.cal_selected, p.cal_selected, p.cal_selected_text)
        } else if date == today && response.hovered() {
            (p.cal_today, p.cal_today, p.cal_selected_text)
        } else if response.hovered() {
            let text = if date.month() == view.month() {
                p.cal_text
            } else {
                p.cal_dim
            };
            (p.cal_hover, p.cal_hover, text)
        } else {
            let text = if date.month() == view.month() {
                p.cal_text
            } else {
                p.cal_dim
            };
            let ring = if date == today {
                p.cal_today
            } else {
                Color32::TRANSPARENT
            };
            (Color32::TRANSPARENT, ring, text)
        };
        let painter = ui.painter();
        painter.circle(center, DAY / 2.0 - 0.5, fill, Stroke::new(1.0, ring));
        painter.text(
            center,
            Align2::CENTER_CENTER,
            date.day().to_string(),
            FontId::proportional(14.0),
            text,
        );
        if response.clicked() {
            pick = Some(Pick::Day(date));
        }
    }

    // Clear: not in flatpickr; empties the field.
    let (footer, _) = ui.allocate_exact_size(vec2(WIDTH, FOOTER), Sense::hover());
    let clear = ui
        .interact(
            footer.shrink2(vec2(WIDTH / 2.0 - 30.0, 4.0)),
            view_id.with("clear"),
            Sense::CLICK,
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let color = if clear.hovered() {
        p.focus
    } else {
        p.cal_weekday
    };
    ui.painter().text(
        footer.center(),
        Align2::CENTER_CENTER,
        "Clear",
        FontId::proportional(13.3),
        color,
    );
    if clear.clicked() {
        pick = Some(Pick::Clear);
    }

    if next_view != view {
        ui.data_mut(|d| d.insert_temp(view_id, next_view));
    }
    pick
}

/// A month arrow: a 14 px chevron pointing left (`dir < 0`) or right, as flatpickr's SVG.
fn arrow_button(ui: &Ui, id: egui::Id, rect: Rect, color: Color32, dir: f32) -> Response {
    let response = ui
        .interact(rect, id, Sense::CLICK)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let color = if response.hovered() {
        ui.visuals().strong_text_color()
    } else {
        color
    };
    let c = rect.center();
    let (w, h) = (-3.5 * dir, 7.0);
    ui.painter().add(Shape::line(
        vec![
            Pos2::new(c.x + w, c.y - h),
            Pos2::new(c.x - w, c.y),
            Pos2::new(c.x + w, c.y + h),
        ],
        Stroke::new(1.5, color),
    ));
    response
}

/// One half of the year spinner: a small filled triangle pointing up or down, drawn when `color`
/// is set.
fn spin_button(ui: &Ui, id: egui::Id, rect: Rect, color: Option<Color32>, up: bool) -> Response {
    let response = ui
        .interact(rect, id, Sense::CLICK)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let Some(color) = color else {
        return response;
    };
    let c = rect.center();
    let (w, h) = (3.0, if up { -2.0 } else { 2.0 });
    ui.painter().add(Shape::convex_polygon(
        vec![
            Pos2::new(c.x - w, c.y - h),
            Pos2::new(c.x + w, c.y - h),
            Pos2::new(c.x, c.y + h),
        ],
        color,
        Stroke::NONE,
    ));
    response
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

/// `view` moved by `months`; unchanged at the ends of chrono's range.
fn step(view: NaiveDate, months: i32) -> NaiveDate {
    let n = Months::new(months.unsigned_abs());
    if months < 0 {
        view.checked_sub_months(n)
    } else {
        view.checked_add_months(n)
    }
    .unwrap_or(view)
}

/// The 42 days shown for the month of `view`: six weeks starting on the `first` weekday on or
/// before the 1st, as flatpickr shows the end of the previous month and the start of the next.
fn grid(view: NaiveDate, first: Weekday) -> [NaiveDate; 42] {
    let start = first_of_month(view);
    let back = (start.weekday().num_days_from_monday() + 7 - first.num_days_from_monday()) % 7;
    let start = start - chrono::Days::new(u64::from(back));
    std::array::from_fn(|i| start + chrono::Days::new(i as u64))
}

fn short_name(day: Weekday) -> &'static str {
    match day {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

/// The week start for this run, looked up once.
fn first_weekday_cached() -> Weekday {
    static FIRST: OnceLock<Weekday> = OnceLock::new();
    *FIRST.get_or_init(|| first_weekday(&locale()))
}

/// The locale that governs dates. On Unix `LC_ALL`, then `LC_TIME`, then `LANG`, because
/// `sys-locale` reads the messages category there; elsewhere (and as the fallback) `sys-locale`.
fn locale() -> String {
    #[cfg(unix)]
    for var in ["LC_ALL", "LC_TIME", "LANG"] {
        if let Ok(value) = std::env::var(var)
            && !value.is_empty()
        {
            return value;
        }
    }
    sys_locale::get_locale().unwrap_or_default()
}

/// The first day of the week for `locale` (POSIX `en_US.UTF-8` or BCP 47 `en-US`), from CLDR's week
/// data. `C`, `POSIX`, an empty or an unparsable locale start on Sunday, as in the US.
fn first_weekday(locale: &str) -> Weekday {
    // POSIX `language_REGION.codeset@modifier` to BCP 47 `language-REGION`.
    let tag = locale
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-");
    if matches!(tag.as_str(), "" | "C" | "POSIX") {
        return Weekday::Sun;
    }
    let Ok(locale) = icu_locale_core::Locale::try_from_str(&tag) else {
        return Weekday::Sun;
    };
    let Ok(info) = icu_calendar::week::WeekInformation::try_new((&locale).into()) else {
        return Weekday::Sun;
    };
    use icu_calendar::types::Weekday as W;
    match info.first_weekday {
        W::Monday => Weekday::Mon,
        W::Tuesday => Weekday::Tue,
        W::Wednesday => Weekday::Wed,
        W::Thursday => Weekday::Thu,
        W::Friday => Weekday::Fri,
        W::Saturday => Weekday::Sat,
        W::Sunday => Weekday::Sun,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, DATE_FORMAT).unwrap()
    }

    #[test]
    fn first_weekday_follows_the_locale() {
        for (locale, day) in [
            ("en_US.UTF-8", Weekday::Sun),
            ("en-US", Weekday::Sun),
            ("sr_RS.UTF-8", Weekday::Mon),
            ("sr-Latn-RS", Weekday::Mon),
            ("de_DE@euro", Weekday::Mon),
            ("ar_EG.UTF-8", Weekday::Sat),
            ("en-US-u-fw-mon", Weekday::Mon),
            ("C", Weekday::Sun),
            ("C.UTF-8", Weekday::Sun),
            ("POSIX", Weekday::Sun),
            ("", Weekday::Sun),
            ("not a locale!", Weekday::Sun),
        ] {
            assert_eq!(first_weekday(locale), day, "{locale:?}");
        }
    }

    #[test]
    fn grid_starts_on_the_first_weekday() {
        // 2026-11-01 is a Sunday: no leading days when the week starts on Sunday.
        let november = grid(date("2026-11-15"), Weekday::Sun);
        assert_eq!(november[0], date("2026-11-01"));
        assert_eq!(november[41], date("2026-12-12"));
        // With Monday first, the grid starts on the last Monday of October.
        assert_eq!(
            grid(date("2026-11-01"), Weekday::Mon)[0],
            date("2026-10-26")
        );
        // February 2028 (leap year) starts on a Tuesday.
        let february = grid(date("2028-02-29"), Weekday::Mon);
        assert_eq!(february[0], date("2028-01-31"));
        assert!(february.contains(&date("2028-02-29")));
    }

    #[test]
    fn stepping_crosses_years() {
        assert_eq!(step(date("2026-01-01"), -1), date("2025-12-01"));
        assert_eq!(step(date("2026-12-01"), 1), date("2027-01-01"));
        assert_eq!(step(date("2026-03-01"), -12), date("2025-03-01"));
    }
}
