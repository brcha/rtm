//! RTM-33 spike: how closely can egui reproduce the Tauri rtmapp look?
//!
//! Hardcoded sample data, no todotxt dependency. Environment knobs for repeatable screenshots:
//!   SPIKE_THEME=light|dark   SPIKE_DIALOG=1   SPIKE_FOCUS=1   SPIKE_DATEPICKER=1   SPIKE_SHOT=out.png   SPIKE_SHOT_FRAME=40
//!   SPIKE_FONT / SPIKE_FONT_BOLD = font file paths (no font is bundled, see FINDINGS.md)

use jiff::civil::Date;
use eframe::egui::{
    self, Align, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Id, Layout, Margin,
    Modal, Response, RichText, Sense, Shape, Stroke, StrokeKind, TextEdit, Theme, ThemePreference,
    Ui, text::LayoutJob,
};
use egui_extras::DatePickerButton;

struct P {
    bg: Color32,
    header: Color32,
    panel: Color32,
    border: Color32,
    input_bg: Color32,
    input_border: Color32,
    text: Color32,
    muted: Color32,
    info_bg: Color32,
    done_bg: Color32,
    done_text: Color32,
    project: Color32,
    context: Color32,
    due: Color32,
    rec: Color32,
    thresh: Color32,
    focus: Color32,
    dialog: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const LIGHT: P = P {
    bg: rgb(0xf5, 0xf5, 0xf5),
    header: rgb(0x24, 0x29, 0x2e),
    panel: rgb(0xff, 0xff, 0xff),
    border: rgb(0xe1, 0xe4, 0xe8),
    input_bg: rgb(0xff, 0xff, 0xff),
    input_border: rgb(0xd1, 0xd5, 0xda),
    text: rgb(0x21, 0x35, 0x47),
    muted: rgb(0x58, 0x60, 0x69),
    info_bg: rgb(0xf6, 0xf8, 0xfa),
    done_bg: rgb(0xf6, 0xf8, 0xfa),
    done_text: rgb(0x6a, 0x73, 0x7d),
    project: rgb(0x03, 0x66, 0xd6),
    context: rgb(0x6a, 0x73, 0x7d),
    due: rgb(0xe7, 0x4c, 0x3c),
    rec: rgb(0x9b, 0x59, 0xb6),
    thresh: rgb(0x95, 0xa5, 0xa6),
    focus: rgb(0x03, 0x66, 0xd6),
    dialog: rgb(0xff, 0xff, 0xff),
};

const DARK: P = P {
    bg: rgb(0x0d, 0x11, 0x17),
    header: rgb(0x16, 0x1b, 0x22),
    panel: rgb(0x16, 0x1b, 0x22),
    border: rgb(0x30, 0x36, 0x3d),
    input_bg: rgb(0x0d, 0x11, 0x17),
    input_border: rgb(0x30, 0x36, 0x3d),
    text: rgb(0xc9, 0xd1, 0xd9),
    muted: rgb(0x8b, 0x94, 0x9e),
    info_bg: rgb(0x21, 0x26, 0x2d),
    done_bg: rgb(0x21, 0x26, 0x2d),
    done_text: rgb(0x8b, 0x94, 0x9e),
    project: rgb(0x58, 0xa6, 0xff),
    context: rgb(0x8b, 0x94, 0x9e),
    due: rgb(0xf8, 0x51, 0x49),
    rec: rgb(0xbc, 0x8c, 0xff),
    thresh: rgb(0x8b, 0x94, 0x9e),
    focus: rgb(0x58, 0xa6, 0xff),
    dialog: rgb(0x16, 0x1b, 0x22),
};

const GREEN: Color32 = rgb(0x28, 0xa7, 0x45);
const GREEN_HOVER: Color32 = rgb(0x21, 0x88, 0x38);
const BLUE: Color32 = rgb(0x03, 0x66, 0xd6);
const BLUE_HOVER: Color32 = rgb(0x02, 0x56, 0xb9);
const GREY: Color32 = rgb(0x6a, 0x73, 0x7d);
const GREY_HOVER: Color32 = rgb(0x58, 0x60, 0x69);

fn pal(ui: &Ui) -> &'static P {
    if ui.visuals().dark_mode { &DARK } else { &LIGHT }
}

struct Sample {
    done: bool,
    prio: Option<char>,
    text: &'static str,
    projects: &'static [&'static str],
    contexts: &'static [&'static str],
    due: Option<&'static str>,
    rec: Option<&'static str>,
    thresh: Option<&'static str>,
}

fn samples() -> Vec<Sample> {
    vec![
        Sample { done: false, prio: Some('A'), text: "Pay the electricity bill", projects: &["Home"], contexts: &["online"], due: Some("2026-11-01"), rec: Some("+1m"), thresh: None },
        Sample { done: false, prio: Some('B'), text: "Prepare the quarterly report for the whole team and send it around for review before the Friday meeting", projects: &["Work", "Reports"], contexts: &["office"], due: Some("2026-11-04"), rec: None, thresh: Some("2026-10-30") },
        Sample { done: false, prio: Some('C'), text: "Water the plants", projects: &[], contexts: &["home"], due: None, rec: Some("1w"), thresh: None },
        Sample { done: false, prio: None, text: "Item with no priority and no dates", projects: &[], contexts: &[], due: None, rec: None, thresh: None },
        Sample { done: true, prio: None, text: "Buy groceries", projects: &["Home"], contexts: &["shop"], due: Some("2026-10-25"), rec: None, thresh: None },
    ]
}

struct App {
    shot: Option<String>,
    frame: u32,
    items: Vec<Sample>,
    new_item: String,
    show_completed: bool,
    show_future: bool,
    hide_no_date: bool,
    reverse: bool,
    dialog: bool,
    focus_once: bool,
    datepicker: bool,
    due_rect: Option<egui::Rect>,
    hook_frame: u32,
    shot_frame: u32,
    d_desc: String,
    d_prio: usize,
    d_due: Date,
    d_thresh: Date,
    d_rec: String,
    d_projects: String,
    d_contexts: String,
}

fn load_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    if let Ok(bytes) = std::env::var("SPIKE_FONT").and_then(|p| std::fs::read(p).map_err(|_| std::env::VarError::NotPresent)) {
        fonts.font_data.insert("ui".into(), FontData::from_owned(bytes).into());
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "ui".into());
    }
    // egui has no font weights: a bold face has to be a separate family.
    let bold = std::env::var("SPIKE_FONT_BOLD").ok().and_then(|p| std::fs::read(p).ok());
    let bold_name = if let Some(bytes) = bold {
        fonts.font_data.insert("ui-bold".into(), FontData::from_owned(bytes).into());
        "ui-bold"
    } else {
        "Ubuntu-Light"
    };
    fonts.families.insert(FontFamily::Name("bold".into()), vec![bold_name.into()]);
    ctx.set_fonts(fonts);
}

fn style_for(theme: Theme, p: &P) -> egui::Visuals {
    let mut v = if theme == Theme::Dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.bg;
    v.window_fill = p.dialog;
    v.extreme_bg_color = p.input_bg;
    v.override_text_color = Some(p.text);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.selection.bg_fill = p.focus;
    v.window_corner_radius = 8.0.into();
    v.window_stroke = if theme == Theme::Dark { Stroke::new(1.0, p.border) } else { Stroke::NONE };
    v.window_shadow = egui::Shadow { offset: [0, 4], blur: 20, spread: 0, color: Color32::from_black_alpha(51) };
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
        w.corner_radius = 6.0.into();
    }
    v
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        load_fonts(&cc.egui_ctx);
        cc.egui_ctx.set_visuals_of(Theme::Light, style_for(Theme::Light, &LIGHT));
        cc.egui_ctx.set_visuals_of(Theme::Dark, style_for(Theme::Dark, &DARK));
        cc.egui_ctx.set_theme(match std::env::var("SPIKE_THEME").as_deref() {
            Ok("dark") => ThemePreference::Dark,
            Ok("light") => ThemePreference::Light,
            _ => ThemePreference::System,
        });
        // CSS: root font-size 14px, buttons 0.9rem, small chips 0.8-0.85rem.
        cc.egui_ctx.all_styles_mut(|s| {
            use egui::TextStyle::*;
            s.text_styles.insert(Body, FontId::proportional(14.0));
            s.text_styles.insert(Button, FontId::proportional(12.6));
            s.text_styles.insert(Small, FontId::proportional(11.9));
            s.text_styles.insert(Heading, FontId::proportional(17.5));
            s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        });
        let d = jiff::civil::date(2026, 11, 1);
        Self {
            shot: std::env::var("SPIKE_SHOT").ok(),
            frame: 0,
            items: samples(),
            new_item: String::new(),
            show_completed: true,
            show_future: false,
            hide_no_date: false,
            reverse: false,
            dialog: std::env::var("SPIKE_DIALOG").is_ok() || std::env::var("SPIKE_DATEPICKER").is_ok(),
            datepicker: std::env::var("SPIKE_DATEPICKER").is_ok(),
            due_rect: None,
            hook_frame: 0,
            shot_frame: std::env::var("SPIKE_SHOT_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(40),
            focus_once: std::env::var("SPIKE_FOCUS").is_ok(),
            d_desc: "Pay the electricity bill".into(),
            d_prio: 1,
            d_due: d,
            d_thresh: d,
            d_rec: "+1m".into(),
            d_projects: "Home".into(),
            d_contexts: "online".into(),
        }
    }
}

/// Coloured button with a CSS-style hover colour.
fn button(ui: &mut Ui, text: &str, bg: Color32, hover: Color32) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(16.0, 10.0);
        let w = &mut ui.visuals_mut().widgets;
        for (s, c) in [(&mut w.inactive, bg), (&mut w.hovered, hover), (&mut w.active, hover)] {
            s.weak_bg_fill = c;
            s.bg_fill = c;
            s.bg_stroke = Stroke::NONE;
            s.expansion = 0.0;
            s.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        }
        ui.button(RichText::new(text).size(12.6).strong())
    })
    .inner
}

/// Text input with the CSS border and the 3 px focus glow.
fn input(ui: &mut Ui, salt: &str, text: &mut String, hint: &str, pad_y: f32, request_focus: bool) -> Response {
    let p = pal(ui);
    let id = ui.make_persistent_id(salt);
    ui.spacing_mut().interact_size.y = 15.0;
    let focused = ui.memory(|m| m.has_focus(id));
    let border = if focused { p.focus } else { p.input_border };
    let out = Frame::new()
        .fill(p.input_bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(6.0)
        .inner_margin(Margin::symmetric(12, pad_y as i8))
        .show(ui, |ui| {
            let r = ui.add(
                TextEdit::singleline(text)
                    .id(id)
                    .hint_text(RichText::new(hint).color(p.muted))
                    .frame(Frame::NONE)
                    .desired_width(f32::INFINITY)
                    .font(FontId::proportional(13.3)),
            );
            if request_focus {
                r.request_focus();
            }
            r
        });
    if focused {
        let glow = Color32::from_rgba_unmultiplied(p.focus.r(), p.focus.g(), p.focus.b(), 77);
        ui.painter().rect_stroke(out.response.rect, 6.0, Stroke::new(3.0, glow), StrokeKind::Outside);
    }
    out.inner
}

fn item_job(p: &P, it: &Sample, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let body = FontId::proportional(14.0);
    let chip = |size: f32| FontId::proportional(size);
    let main = if it.done { p.done_text } else { p.text };
    let mut add = |s: String, f: FontId, c: Color32, strike: bool| {
        let mut fmt = egui::TextFormat::simple(f, c);
        if strike {
            fmt.strikethrough = Stroke::new(1.0, c);
        }
        job.append(&s, 0.0, fmt);
    };
    if let Some(l) = it.prio {
        let c = match l { 'A' => rgb(0xe7, 0x4c, 0x3c), 'B' => rgb(0xe6, 0x7e, 0x22), _ => rgb(0xf3, 0x9c, 0x12) };
        add(format!("({l}) "), FontId::new(11.9, FontFamily::Name("bold".into())), c, false);
    }
    add(it.text.to_string(), body, main, it.done);
    for pr in it.projects { add(format!(" +{pr}"), chip(11.9), p.project, false); }
    for cx in it.contexts { add(format!(" @{cx}"), chip(11.9), p.context, false); }
    if let Some(d) = it.due { add(format!(" due:{d}"), chip(11.2), p.due, false); }
    if let Some(r) = it.rec { add(format!(" rec:{r}"), chip(11.2), p.rec, false); }
    if let Some(t) = it.thresh { add(format!(" t:{t}"), chip(11.2), p.thresh, false); }
    job
}

/// Give egui's stock buttons/combos the bordered "form control" look of the CSS inputs.
fn form_control<R>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
    let p = pal(ui);
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(12.0, 9.0);
        let w = &mut ui.visuals_mut().widgets;
        for s in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
            s.weak_bg_fill = p.input_bg;
            s.bg_fill = p.input_bg;
            s.bg_stroke = Stroke::new(1.0, p.input_border);
            s.corner_radius = 6.0.into();
            s.expansion = 0.0;
        }
        f(ui)
    })
    .inner
}

impl App {
    fn header(&self, ui: &mut Ui) {
        let p = pal(ui);
        Frame::new().fill(p.header).inner_margin(Margin::symmetric(20, 12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(26.25);
            ui.label(RichText::new("Rusty Todo.txt Manager").font(FontId::new(17.5, FontFamily::Name("bold".into()))).color(Color32::WHITE));
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        let p = pal(ui);
        ui.spacing_mut().item_spacing.y = 16.0;
        button(ui, "Load File", BLUE, BLUE_HOVER);
        Frame::new().fill(p.info_bg).corner_radius(6.0).inner_margin(8.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("/home/user/todo.txt").size(11.9).color(p.muted));
        });
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            ui.checkbox(&mut self.show_completed, RichText::new("Show completed items").size(12.6));
            ui.checkbox(&mut self.show_future, RichText::new("Show future items").size(12.6));
            ui.checkbox(&mut self.hide_no_date, RichText::new("Hide items without due date").size(12.6));
            ui.checkbox(&mut self.reverse, RichText::new("Reverse sort order").size(12.6));
        });
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.label(RichText::new(format!("Total items: {}", self.items.len())).size(11.9).color(p.muted));
        });
    }

    fn content(&mut self, ui: &mut Ui) {
        let p = pal(ui);
        ui.horizontal(|ui| {
            let w = ui.available_width() - 60.0 - 8.0;
            let focus = std::mem::take(&mut self.focus_once);
            ui.scope(|ui| {
                ui.set_width(w);
                input(ui, "new-item", &mut self.new_item, "New item (e.g., (A) Task due:2024-01-15 +Project @context)", 10.0, focus);
            });
            button(ui, "Add", GREEN, GREEN_HOVER);
        });
        ui.add_space(8.0);
        let mut open = false;
        Frame::new().fill(p.panel).stroke(Stroke::new(1.0, p.border)).corner_radius(6.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                let n = self.items.len();
                for (i, it) in self.items.iter().enumerate() {
                    let fill = if it.done { p.done_bg } else { Color32::TRANSPARENT };
                    let row = Frame::new().fill(fill).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_min_height(25.0);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            // Painted: the ☐/☑ glyphs are missing from most UI fonts and fall back to tofu.
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), Sense::click());
                            let c = if it.done { GREEN } else { p.done_text };
                            ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.3, c), StrokeKind::Inside);
                            if it.done {
                                let (a, b, d) = (rect.left_top() + egui::vec2(3.0, 7.0), rect.left_top() + egui::vec2(6.0, 10.0), rect.left_top() + egui::vec2(11.0, 4.0));
                                ui.painter().add(Shape::line(vec![a, b, d], Stroke::new(1.6, c)));
                            }
                            ui.add_space(10.0);
                            let hover_bg = ui.painter().add(Shape::Noop);
                            let wrap = ui.available_width() - 8.0;
                            let r = ui.add(egui::Label::new(item_job(p, it, wrap)).wrap_mode(egui::TextWrapMode::Wrap).sense(Sense::click()));
                            if r.hovered() {
                                ui.painter().set(hover_bg, Shape::rect_filled(r.rect.expand(2.0), 2.0, Color32::from_black_alpha(8)));
                            }
                            if r.clicked() { open = true; }
                        });
                    });
                    if i + 1 < n {
                        let y = row.response.rect.bottom();
                        let r = row.response.rect;
                        ui.painter().hline(r.x_range(), y - 0.5, Stroke::new(1.0, p.border));
                    }
                }
            });
        });
        if open { self.dialog = true; }
    }

    fn label(ui: &mut Ui, text: &str) {
        ui.label(RichText::new(text).size(12.6));
        ui.add_space(-2.0);
    }

    fn edit_dialog(&mut self, ctx: &egui::Context) {
        let p = if ctx.global_style().visuals.dark_mode { &DARK } else { &LIGHT };
        let frame = Frame::new()
            .fill(p.dialog)
            .stroke(if p.dialog == DARK.dialog { Stroke::new(1.0, p.border) } else { Stroke::NONE })
            .corner_radius(8.0)
            .inner_margin(24.0)
            .shadow(egui::Shadow { offset: [0, 4], blur: 20, spread: 0, color: Color32::from_black_alpha(51) });
        let m = Modal::new(Id::unique("edit-dialog")).frame(frame).backdrop_color(Color32::from_black_alpha(128)).show(ctx, |ui| {
            ui.set_width(452.0);
            ui.spacing_mut().item_spacing.y = 16.0;
            ui.label(RichText::new("Edit Item").font(FontId::new(17.5, FontFamily::Name("bold".into()))));
            ui.add_space(4.0);
            ui.vertical(|ui| { Self::label(ui, "Description"); input(ui, "d-desc", &mut self.d_desc, "", 8.0, false); });
            ui.columns(2, |c| {
                Self::label(&mut c[0], "Priority");
                let cw = c[0].available_width() - 8.0;
                form_control(&mut c[0], |ui| egui::ComboBox::from_id_salt("prio").width(cw)
                    .selected_text(["None", "(A) High", "(B) Medium", "(C) Low"][self.d_prio])
                    .show_ui(ui, |ui| {
                        for (i, s) in ["None", "(A) High", "(B) Medium", "(C) Low"].iter().enumerate() {
                            ui.selectable_value(&mut self.d_prio, i, *s);
                        }
                    }));
                Self::label(&mut c[1], "Due Date");
                self.due_rect = Some(form_control(&mut c[1], |ui| ui.add(DatePickerButton::new(&mut self.d_due).id_salt("due").format("%Y-%m-%d"))).rect);
            });
            ui.columns(2, |c| {
                Self::label(&mut c[0], "Recurrence");
                input(&mut c[0], "d-rec", &mut self.d_rec, "e.g., 1d, 1w, 1m", 8.0, false);
                Self::label(&mut c[1], "Threshold");
                form_control(&mut c[1], |ui| ui.add(DatePickerButton::new(&mut self.d_thresh).id_salt("thr").format("%Y-%m-%d")));
            });
            ui.vertical(|ui| { Self::label(ui, "Projects (comma separated)"); input(ui, "d-proj", &mut self.d_projects, "e.g., Work, Personal", 8.0, false); });
            ui.vertical(|ui| { Self::label(ui, "Contexts (comma separated)"); input(ui, "d-ctx", &mut self.d_contexts, "e.g., home, office", 8.0, false); });
            ui.add_space(4.0);
            let mut close = false;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, "Save", GREEN, GREEN_HOVER).clicked() { close = true; }
                if button(ui, "Cancel", GREY, GREY_HOVER).clicked() { close = true; }
            });
            close
        });
        if m.inner || m.should_close() { self.dialog = false; }
    }
}

impl eframe::App for App {
    // The date picker's open state is private to egui_extras, so a screenshot of the popup needs a real click.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.hook_frame += 1;
        let (Some(r), true) = (self.due_rect, self.datepicker) else { return };
        let pos = r.center();
        let ev = |pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        match self.hook_frame {
            25 => raw.events.extend([egui::Event::PointerMoved(pos), ev(true)]),
            26 => raw.events.push(ev(false)),
            _ => {}
        }
    }

    fn ui(&mut self, ui: &mut Ui, _f: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame += 1;
        let p = if ctx.global_style().visuals.dark_mode { &DARK } else { &LIGHT };
        egui::Panel::top("header").frame(Frame::NONE).show(ui, |ui| self.header(ui));
        egui::Panel::left("sidebar")
            .exact_size(220.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(p.panel).inner_margin(16.0))
            .show(ui, |ui| {
                self.sidebar(ui);
                let r = ui.max_rect().expand(16.0);
                ui.painter().vline(r.right() - 0.5, r.y_range(), Stroke::new(1.0, p.border));
            });
        egui::CentralPanel::default().frame(Frame::new().fill(p.bg).inner_margin(16.0)).show(ui, |ui| self.content(ui));
        let ctx = &ctx;
        if self.dialog { self.edit_dialog(ctx); }

        if let Some(path) = self.shot.clone() {
            if self.frame == self.shot_frame {
                ctx.request_screenshot(move |img| {
                    let raw: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                    image::save_buffer(&path, &raw, img.size[0] as u32, img.size[1] as u32, image::ColorType::Rgba8).unwrap();
                });
            }
            if self.frame == self.shot_frame + 20 { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
            ctx.request_repaint();
        }
    }
}

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 700.0]).with_title("Rusty Todo.txt Manager"),
        ..Default::default()
    };
    eframe::run_native("rtmgui-spike", opts, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
