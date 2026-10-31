//! The app driven headless through its AccessKit tree with egui_kittest: real clicks and typing,
//! then the todo.txt and config on disk are checked.

use egui::{Key, accesskit::Role};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

use super::support_tests::*;
use super::*;

const FILTERS: [&str; 4] = [
    "Show completed items",
    "Show future items",
    "Hide items without due date",
    "Reverse sort order",
];

/// The app as `main` starts it, on a temp config naming a temp todo.txt with `contents`. `config`
/// adjusts the config first; sync is off. The window has `main`'s size, so the edit dialog fits.
fn harness(
    dir: &Path,
    contents: &str,
    config: impl FnOnce(&mut AppConfig),
) -> Harness<'static, App> {
    let file = dir.join("todo.txt");
    std::fs::write(&file, contents).unwrap();
    let mut settings = AppConfig {
        file_name: Some(file.to_string_lossy().into_owned()),
        sync: false,
        ..AppConfig::default()
    };
    config(&mut settings);
    let config_path = dir.join("config.toml");
    settings.save(&config_path).unwrap();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 650.0))
        // The modal's fade and a focused text cursor take more frames than the default 4.
        .with_max_steps(60)
        .build_eframe(|cc| App::with_config(cc, config_path));
    harness.run();
    harness
}

/// The items shown, in list order: the names of the row checkboxes.
fn rows(harness: &Harness<'_, App>) -> Vec<String> {
    harness
        .get_all_by_role(Role::CheckBox)
        .filter_map(|node| node.accesskit_node().label())
        .filter(|label| !FILTERS.contains(&label.as_str()))
        .collect()
}

fn today() -> String {
    chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn the_configured_file_opens_at_startup() {
    let dir = temp_dir("ui-startup");
    let harness = harness(&dir, "First item\nSecond item\n", |_| {});

    assert_eq!(rows(&harness), ["First item", "Second item"]);
    harness.get_by_label("Total items: 2");
    assert!(harness.query_by_label_contains("No file loaded").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_chosen_in_the_dialog_opens_and_is_saved_in_the_config() {
    let dir = temp_dir("ui-open");
    let mut harness = harness(&dir, "", |config| config.file_name = None);
    harness.get_by_label_contains("No file loaded. Use 'New File'");

    let chosen = dir.join("sub").join("chosen.txt");
    std::fs::write(&chosen, "From the dialog\n").unwrap();
    // Clicking Load File would start a real file dialog; its result is handed over instead.
    dialog_returned(harness.state_mut(), Some(chosen.clone()), App::open_file);
    harness.run();

    assert_eq!(rows(&harness), ["From the dialog"]);
    let config = AppConfig::load(&dir.join("config.toml"));
    let canonical = chosen.canonicalize().unwrap();
    assert_eq!(config.file_name.as_deref(), canonical.to_str());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn adding_through_enter_and_the_add_button_saves_the_items() {
    let dir = temp_dir("ui-add");
    let mut harness = harness(&dir, "First\n", |_| {});

    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    input.type_text("Second +Proj");
    harness.key_press(Key::Enter);
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("Third");
    harness.get_by_label("Add").click();
    harness.run();

    let today = today();
    assert_eq!(
        file_lines(&dir),
        [
            "First".to_owned(),
            format!("{today} Second +Proj"),
            format!("{today} Third"),
        ]
    );
    assert_eq!(
        harness.get_by_role(Role::TextInput).value().as_deref(),
        Some("")
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_row_checkbox_completes_and_uncompletes() {
    let dir = temp_dir("ui-toggle");
    let mut harness = harness(&dir, "Pay bill\n", |config| {
        config.show_completed_items = true;
    });

    harness
        .get_by_role_and_label(Role::CheckBox, "Pay bill")
        .click();
    harness.run();

    assert_eq!(file_lines(&dir), [format!("x {} Pay bill", today())]);
    let checkbox = harness.get_by_role_and_label(Role::CheckBox, "Pay bill");
    assert_eq!(
        checkbox.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );

    checkbox.click();
    harness.run();

    assert_eq!(file_lines(&dir), ["Pay bill"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn editing_in_the_dialog_saves_the_fields() {
    let dir = temp_dir("ui-edit");
    let mut harness = harness(&dir, "Old text\nOther\n", |_| {});

    harness
        .get_by_role_and_label(Role::Label, "Old text")
        .click();
    harness.run();
    harness.get_by_label("Edit Item");
    let description = harness.get_by_label("Description");
    description.focus();
    harness.key_combination_modifiers(egui::Modifiers::COMMAND, &[Key::A]);
    description.type_text("New text");
    harness.get_by_label("Due Date").focus();
    harness.get_by_label("Due Date").type_text("2026-11-05");
    harness.run();
    // The Due Date field opened its calendar, and a click on Save would only close it.
    harness.key_press(Key::Escape);
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();

    assert_eq!(file_lines(&dir), ["New text due:2026-11-05", "Other"]);
    assert!(harness.query_by_label("Edit Item").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn deleting_an_item_with_sub_items_asks_and_removes_them_all() {
    let dir = temp_dir("ui-delete");
    let uuid = "11111111-1111-1111-1111-111111111111";
    let contents =
        format!("Parent uuid:{uuid}\nChild one sub:{uuid}\nUnrelated\nChild two sub:{uuid}\n");
    let mut harness = harness(&dir, &contents, |_| {});

    harness.get_by_role_and_label(Role::Label, "Parent").click();
    harness.run();
    harness.get_by_label("Delete item").click();
    harness.run();
    harness.get_by_label("Delete this item and 2 sub-items?");

    // Keep goes back to the trash button and writes nothing.
    harness.get_by_label("Keep").click();
    harness.run();
    assert!(
        harness
            .query_by_label("Delete this item and 2 sub-items?")
            .is_none()
    );
    assert_eq!(file_lines(&dir).len(), 4);

    harness.get_by_label("Delete item").click();
    harness.run();
    harness.get_by_label("Delete").click();
    harness.run();

    assert_eq!(file_lines(&dir), ["Unrelated"]);
    assert!(harness.query_by_label("Edit Item").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_filter_and_sort_toggles_change_the_list_and_are_saved() {
    let dir = temp_dir("ui-filters");
    let contents = "x 2026-01-01 Done item\nFuture item due:2099-01-01\nUndated item\n\
                    Late item due:2021-01-01\nEarly item due:2020-01-01\n";
    let mut harness = harness(&dir, contents, |_| {});
    assert_eq!(rows(&harness), ["Undated item", "Early item", "Late item"]);

    harness.get_by_label("Show completed items").click();
    harness.run();
    harness.get_by_label("Show future items").click();
    harness.run();
    assert_eq!(
        rows(&harness),
        [
            "Done item",
            "Undated item",
            "Early item",
            "Late item",
            "Future item"
        ]
    );

    harness.get_by_label("Hide items without due date").click();
    harness.run();
    assert_eq!(rows(&harness), ["Early item", "Late item", "Future item"]);

    harness.get_by_label("Reverse sort order").click();
    harness.run();
    assert_eq!(rows(&harness), ["Future item", "Late item", "Early item"]);

    let config = AppConfig::load(&dir.join("config.toml"));
    assert!(config.show_completed_items && config.show_future_items);
    assert!(config.hide_no_date && config.reverse_sort);
    assert_eq!(
        std::fs::read_to_string(dir.join("todo.txt")).unwrap(),
        contents
    );
    let _ = std::fs::remove_dir_all(dir);
}
