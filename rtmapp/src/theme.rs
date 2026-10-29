//! Colours, fonts and egui styling that reproduce the look of the former Tauri
//! app (`rtmapp/src/styles.css` at 5076649); see "Look" in `rtmapp/AGENTS.md`.

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke, Theme, Ui,
};

/// Colours for one theme. Fields are added as the widgets that use them are ported.
pub struct Palette {
    pub bg: Color32,
    pub header: Color32,
    pub panel: Color32,
    pub border: Color32,
    pub info_bg: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub done_bg: Color32,
    pub done_text: Color32,
    pub project: Color32,
    pub context: Color32,
    pub due: Color32,
    pub recurrence: Color32,
    pub threshold: Color32,
    pub input_bg: Color32,
    pub input_border: Color32,
    /// Focused form field border and glow (`.form-group input:focus`).
    pub focus: Color32,
    pub dialog_bg: Color32,
    /// Transparent in light: only the dark CSS gives the dialog a border.
    pub dialog_border: Color32,
    /// The calendar popup. Light copies flatpickr's CSS (its translucent colours blended over white);
    /// dark uses the app's dark palette, as flatpickr has no dark theme.
    pub cal_bg: Color32,
    pub cal_border: Color32,
    /// Month name, year and arrows.
    pub cal_header: Color32,
    pub cal_weekday: Color32,
    pub cal_text: Color32,
    /// Days of the previous and next month.
    pub cal_dim: Color32,
    pub cal_hover: Color32,
    /// Today's ring, and its fill on hover.
    pub cal_today: Color32,
    pub cal_selected: Color32,
    pub cal_selected_text: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const LIGHT: Palette = Palette {
    bg: rgb(0xf5, 0xf5, 0xf5),
    header: rgb(0x24, 0x29, 0x2e),
    panel: rgb(0xff, 0xff, 0xff),
    border: rgb(0xe1, 0xe4, 0xe8),
    info_bg: rgb(0xf6, 0xf8, 0xfa),
    text: rgb(0x21, 0x35, 0x47),
    muted: rgb(0x58, 0x60, 0x69),
    done_bg: rgb(0xf6, 0xf8, 0xfa),
    done_text: rgb(0x6a, 0x73, 0x7d),
    project: rgb(0x03, 0x66, 0xd6),
    context: rgb(0x6a, 0x73, 0x7d),
    due: rgb(0xe7, 0x4c, 0x3c),
    recurrence: rgb(0x9b, 0x59, 0xb6),
    threshold: rgb(0x95, 0xa5, 0xa6),
    input_bg: rgb(0xff, 0xff, 0xff),
    input_border: rgb(0xd1, 0xd5, 0xda),
    focus: BLUE,
    dialog_bg: rgb(0xff, 0xff, 0xff),
    dialog_border: Color32::TRANSPARENT,
    cal_bg: rgb(0xff, 0xff, 0xff),
    cal_border: rgb(0xe6, 0xe6, 0xe6),
    cal_header: rgb(0x1a, 0x1a, 0x1a),
    cal_weekday: rgb(0x75, 0x75, 0x75),
    cal_text: rgb(0x39, 0x39, 0x39),
    cal_dim: rgb(0xc4, 0xc4, 0xc4),
    cal_hover: rgb(0xe6, 0xe6, 0xe6),
    cal_today: rgb(0x95, 0x9e, 0xa9),
    cal_selected: rgb(0x56, 0x9f, 0xf7),
    cal_selected_text: rgb(0xff, 0xff, 0xff),
};

const DARK: Palette = Palette {
    bg: rgb(0x0d, 0x11, 0x17),
    header: rgb(0x16, 0x1b, 0x22),
    panel: rgb(0x16, 0x1b, 0x22),
    border: rgb(0x30, 0x36, 0x3d),
    info_bg: rgb(0x21, 0x26, 0x2d),
    text: rgb(0xc9, 0xd1, 0xd9),
    muted: rgb(0x8b, 0x94, 0x9e),
    done_bg: rgb(0x21, 0x26, 0x2d),
    done_text: rgb(0x8b, 0x94, 0x9e),
    project: rgb(0x58, 0xa6, 0xff),
    context: rgb(0x8b, 0x94, 0x9e),
    due: rgb(0xf8, 0x51, 0x49),
    recurrence: rgb(0xbc, 0x8c, 0xff),
    threshold: rgb(0x8b, 0x94, 0x9e),
    input_bg: rgb(0x0d, 0x11, 0x17),
    input_border: rgb(0x30, 0x36, 0x3d),
    focus: rgb(0x58, 0xa6, 0xff),
    dialog_bg: rgb(0x16, 0x1b, 0x22),
    dialog_border: rgb(0x30, 0x36, 0x3d),
    cal_bg: rgb(0x16, 0x1b, 0x22),
    cal_border: rgb(0x30, 0x36, 0x3d),
    cal_header: rgb(0xc9, 0xd1, 0xd9),
    cal_weekday: rgb(0x8b, 0x94, 0x9e),
    cal_text: rgb(0xc9, 0xd1, 0xd9),
    cal_dim: rgb(0x4c, 0x52, 0x5a),
    cal_hover: rgb(0x21, 0x26, 0x2d),
    cal_today: rgb(0x8b, 0x94, 0x9e),
    cal_selected: rgb(0x58, 0xa6, 0xff),
    cal_selected_text: rgb(0x0d, 0x11, 0x17),
};

/// Done-item checkbox colour; the same in both themes.
pub const GREEN: Color32 = rgb(0x28, 0xa7, 0x45);

/// Priority colour for A, B and C. Other letters have no colour in the CSS and use the text colour.
pub fn priority_color(priority: u8) -> Option<Color32> {
    match priority {
        0 => Some(rgb(0xe7, 0x4c, 0x3c)),
        1 => Some(rgb(0xe6, 0x7e, 0x22)),
        2 => Some(rgb(0xf3, 0x9c, 0x12)),
        _ => None,
    }
}

/// CSS `#0366d6`: the primary button and the focused input border (both themes for the add row).
pub const BLUE: Color32 = rgb(0x03, 0x66, 0xd6);

/// The blue "Load File" button with its CSS hover colour.
pub fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    filled_button(ui, text, BLUE, rgb(0x02, 0x56, 0xb9))
}

/// The green "Add" button with its CSS hover colour.
pub fn success_button(ui: &mut Ui, text: &str) -> egui::Response {
    filled_button(ui, text, GREEN, rgb(0x21, 0x88, 0x38))
}

/// The grey "Cancel" button with its CSS hover colour (`.btn-secondary`, the same in both themes).
pub fn secondary_button(ui: &mut Ui, text: &str) -> egui::Response {
    filled_button(ui, text, rgb(0x6a, 0x73, 0x7d), rgb(0x58, 0x60, 0x69))
}

/// A single-line input like the CSS ones: 1 px border, 6 px corners, 12 px side padding and
/// `pad_y` above and below, and when focused a `focus` border with a soft 3 px glow.
pub fn text_input(
    ui: &mut Ui,
    p: &Palette,
    text: &mut String,
    hint: &str,
    pad_y: i8,
    focus: Color32,
) -> egui::Response {
    let response = ui
        .scope(|ui| {
            let visuals = ui.visuals_mut();
            visuals.selection.stroke = Stroke::new(1.0, focus);
            let widgets = &mut visuals.widgets;
            for state in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                state.bg_stroke = Stroke::new(1.0, p.input_border);
                state.corner_radius = 6.0.into();
                state.expansion = 0.0;
            }
            ui.add(
                egui::TextEdit::singleline(text)
                    .hint_text(hint)
                    .font(FontId::proportional(13.3))
                    .margin(egui::Margin::symmetric(12, pad_y))
                    .background_color(p.input_bg)
                    .desired_width(ui.available_width()),
            )
        })
        .inner;
    if response.has_focus() {
        let glow = Color32::from_rgba_unmultiplied(focus.r(), focus.g(), focus.b(), 77);
        ui.painter().rect_stroke(
            response.rect,
            6.0,
            Stroke::new(3.0, glow),
            egui::StrokeKind::Outside,
        );
    }
    response
}

fn filled_button(ui: &mut Ui, text: &str, fill: Color32, hover: Color32) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(16.0, 10.0);
        let widgets = &mut ui.visuals_mut().widgets;
        for (state, fill) in [
            (&mut widgets.inactive, fill),
            (&mut widgets.hovered, hover),
            (&mut widgets.active, hover),
        ] {
            state.weak_bg_fill = fill;
            state.bg_fill = fill;
            state.bg_stroke = Stroke::NONE;
            state.expansion = 0.0;
            state.corner_radius = 6.0.into();
            state.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        }
        ui.button(egui::RichText::new(text).size(12.6).strong())
    })
    .inner
}

/// The palette for the theme `ui` is currently drawn with.
pub fn palette(ui: &Ui) -> &'static Palette {
    if ui.visuals().dark_mode {
        &DARK
    } else {
        &LIGHT
    }
}

/// Inter SemiBold, for the title, headings and priorities (egui has no font weights).
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

/// Installs the fonts, both themes and the text sizes. Call once at startup. The theme follows the
/// system preference (egui's default).
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.set_visuals_of(Theme::Light, visuals(Theme::Light, &LIGHT));
    ctx.set_visuals_of(Theme::Dark, visuals(Theme::Dark, &DARK));
    // CSS: root font-size 14 px, buttons 0.9rem, small text 0.85rem.
    ctx.all_styles_mut(|style| {
        use egui::TextStyle::{Body, Button, Heading, Small};
        style.text_styles.insert(Body, FontId::proportional(14.0));
        style.text_styles.insert(Button, FontId::proportional(12.6));
        style.text_styles.insert(Small, FontId::proportional(11.9));
        style
            .text_styles
            .insert(Heading, FontId::proportional(17.5));
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    });
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Inter-Regular".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.otf")).into(),
    );
    fonts.font_data.insert(
        "Inter-SemiBold".into(),
        FontData::from_static(include_bytes!("../assets/fonts/Inter-SemiBold.otf")).into(),
    );
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "Inter-Regular".into());
    // The bold family falls back to the proportional fonts for glyphs Inter lacks.
    let mut bold = vec!["Inter-SemiBold".to_owned()];
    bold.extend(proportional.iter().cloned());
    fonts.families.insert(FontFamily::Name("bold".into()), bold);
    fonts
}

fn visuals(theme: Theme, p: &Palette) -> egui::Visuals {
    let mut v = match theme {
        Theme::Dark => egui::Visuals::dark(),
        Theme::Light => egui::Visuals::light(),
    };
    v.panel_fill = p.bg;
    v.override_text_color = Some(p.text);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v
}
