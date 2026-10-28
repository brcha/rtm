//! Colours, fonts and egui styling that reproduce the look of the Tauri `rtmapp`
//! (`rtmapp/src/styles.css`), as worked out in `spikes/egui-theme`.

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

/// The blue "Load File" button with its CSS hover colour.
pub fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(16.0, 10.0);
        let widgets = &mut ui.visuals_mut().widgets;
        for (state, fill) in [
            (&mut widgets.inactive, rgb(0x03, 0x66, 0xd6)),
            (&mut widgets.hovered, rgb(0x02, 0x56, 0xb9)),
            (&mut widgets.active, rgb(0x02, 0x56, 0xb9)),
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
