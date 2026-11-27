//! The About dialog, opened from the header's menu: the app's name, its version, and links to the
//! website, its problem report page and the licence.

use eframe::egui::{self, Align, Color32, Layout, Modal, RichText, Ui};

use crate::theme::{self, Palette};

const WEBSITE: &str = "https://rtm.brcha.com";
const REPORT: &str = "https://rtm.brcha.com/report/";
/// The licence page the website's footer links to.
const LICENCE: &str = "https://brcha.mit-license.org/@2023";

/// The modal, framed like the other dialogs. Returns true when Close, Escape or a click on the
/// backdrop closed it.
pub fn show(ctx: &egui::Context, p: &Palette) -> bool {
    let mut close = false;
    let modal = Modal::new(egui::Id::unique("about-dialog"))
        .frame(theme::dialog_frame(p))
        .backdrop_color(Color32::from_black_alpha(128))
        .show(ctx, |ui| {
            ui.set_width(360.0);
            close = contents(ui, p);
        });
    close || modal.should_close()
}

fn contents(ui: &mut Ui, p: &Palette) -> bool {
    ui.spacing_mut().item_spacing.y = 8.0;
    ui.label(
        RichText::new("Rusty Todo.txt Manager")
            .font(theme::bold(17.5))
            .color(p.text),
    );
    ui.label(
        RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .size(13.3)
            .color(p.muted),
    );
    ui.add_space(8.0);
    for (text, url) in [
        ("Website", WEBSITE),
        ("Report a problem", REPORT),
        ("MIT licence", LICENCE),
    ] {
        ui.hyperlink_to(RichText::new(text).size(13.3).color(p.link), url);
    }
    ui.add_space(16.0);
    let mut close = false;
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        close = theme::primary_button(ui, "Close").clicked();
    });
    close
}
