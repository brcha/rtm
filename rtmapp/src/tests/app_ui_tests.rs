//! The app driven headless through its AccessKit tree with egui_kittest: real clicks and typing,
//! then the todo.txt and config on disk are checked.

use egui::{Key, Modifiers, accesskit::Role};
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
/// adjusts the config first; sync and the update check are off, and the release list is on a
/// closed port. The window has `main`'s size, so the edit dialog fits.
fn harness(
    dir: &Path,
    contents: &str,
    config: impl FnOnce(&mut AppConfig),
) -> Harness<'static, App> {
    harness_at(dir, contents, closed_url(), config)
}

/// `harness` with the release list at `update_url`.
fn harness_at(
    dir: &Path,
    contents: &str,
    update_url: String,
    config: impl FnOnce(&mut AppConfig),
) -> Harness<'static, App> {
    let file = dir.join("todo.txt");
    std::fs::write(&file, contents).unwrap();
    let mut settings = AppConfig {
        file_name: Some(file.to_string_lossy().into_owned()),
        sync: false,
        check_for_updates: false,
        ..AppConfig::default()
    };
    config(&mut settings);
    let config_path = dir.join("config.toml");
    settings.save(&config_path).unwrap();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 650.0))
        // The modal's fade and a focused text cursor take more frames than the default 4.
        .with_max_steps(60)
        .build_eframe(|cc| App::with_config(cc, config_path, update_url));
    harness.run();
    harness
}

/// A release-list URL nothing listens on, so a check through it fails.
fn closed_url() -> String {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    format!("http://127.0.0.1:{port}/releases.json")
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
fn a_config_from_before_the_file_list_is_migrated_on_the_next_save_only() {
    let dir = temp_dir("ui-old-config");
    let mut harness = harness(&dir, "First item\n", |config| {
        config.vcs = Some("git".into());
    });
    let before = std::fs::read(dir.join("config.toml")).unwrap();
    harness.run();

    assert_eq!(rows(&harness), ["First item"]);
    assert_eq!(std::fs::read(dir.join("config.toml")).unwrap(), before);
    let text = String::from_utf8(before).unwrap();
    assert!(
        text.contains("vcs = \"git\"") && !text.contains("[[files]]"),
        "{text}"
    );

    harness
        .get_by_role_and_label(Role::CheckBox, "Reverse sort order")
        .click();
    harness.run();

    let text = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    let table: toml::Table = text.parse().unwrap();
    let file = dir.join("todo.txt").canonicalize().unwrap();
    assert_eq!(table["file_name"].as_str(), file.to_str());
    assert!(!table.contains_key("vcs"), "{text}");
    let Some([entry]) = table
        .get("files")
        .and_then(|files| files.as_array())
        .map(Vec::as_slice)
    else {
        panic!("one entry: {text}");
    };
    assert_eq!(entry.get("path").and_then(|v| v.as_str()), file.to_str());
    assert_eq!(entry.get("vcs").and_then(|v| v.as_str()), Some("git"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_choice_of_a_file_named_through_a_detour_is_found_after_startup() {
    let dir = temp_dir("ui-detour");
    let harness = harness(&dir, "First item\n", |config| {
        let detour = dir.join("sub").join("..").join("todo.txt");
        config.file_name = Some(detour.to_string_lossy().into_owned());
        config.vcs = Some("pijul".into());
    });

    let app = harness.state();
    let file = app
        .file()
        .unwrap()
        .library
        .as_ref()
        .unwrap()
        .file_name
        .clone();
    assert_eq!(
        Path::new(&file),
        dir.join("todo.txt").canonicalize().unwrap()
    );
    assert_eq!(app.config.file_name.as_deref(), Some(file.as_str()));
    assert_eq!(app.config.files.len(), 1);
    assert_eq!(
        sync::chosen(&app.config, Path::new(&file)),
        Some(rtmsync::Vcs::Pijul)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_app_line_and_the_file_line_are_both_shown() {
    let dir = temp_dir("ui-errors");
    let mut harness = harness(&dir, "First item\n", |_| {});

    harness.state_mut().error = Some("Could not save the config: test".to_owned());
    harness.state_mut().file_mut().unwrap().error = Some("Not committed: test".to_owned());
    harness.run();

    let app_line = harness
        .get_by_label("Could not save the config: test")
        .rect();
    let file_line = harness.get_by_label("Not committed: test").rect();
    assert!(
        app_line.bottom() <= file_line.top(),
        "{app_line:?} {file_line:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_chosen_in_the_dialog_opens_and_is_saved_in_the_config() {
    let dir = temp_dir("ui-open");
    let mut harness = harness(&dir, "", |config| config.file_name = None);
    harness.get_by_label_contains("No file loaded. Use New File or Load File");

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

// The menu and the Settings dialog (RTM-134).

/// Opens the title bar's menu.
fn open_menu(harness: &mut Harness<'_, App>) {
    harness.get_by_label("Menu").click();
    harness.run();
}

/// The menu entry named `name`. Its accessible name ends in the shortcut, which differs between
/// macOS and the others, so only the start is matched.
fn entry<'h>(harness: &'h Harness<'_, App>, name: &'static str) -> egui_kittest::Node<'h> {
    harness.get_by(move |node| {
        node.role() == Role::Button && node.label().is_some_and(|label| label.starts_with(name))
    })
}

#[test]
fn the_menu_has_the_file_entries_settings_and_quit() {
    let dir = temp_dir("ui-menu");
    let mut harness = harness(&dir, "Alpha\n", |_| {});
    // The sidebar no longer has the file buttons.
    assert!(harness.query_by_label("New File").is_none());
    assert!(harness.query_by_label("Load File").is_none());

    open_menu(&mut harness);
    for name in ["New File…", "Load File…", "Settings…", "Quit"] {
        assert!(
            !self::entry(&harness, name).accesskit_node().is_disabled(),
            "{name}"
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_file_entries_are_disabled_while_a_file_dialog_is_open() {
    let dir = temp_dir("ui-menu-dialog");
    let mut harness = harness(&dir, "Alpha\n", |_| {});
    // A dialog that has not returned yet.
    let (_send, receive) = mpsc::channel();
    harness.state_mut().dialog = Some((receive, App::open_file));
    open_menu(&mut harness);
    for name in ["New File…", "Load File…"] {
        assert!(
            self::entry(&harness, name).accesskit_node().is_disabled(),
            "{name}"
        );
    }
    assert!(!entry(&harness, "Settings…").accesskit_node().is_disabled());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn settings_saves_sync_and_the_interval() {
    let dir = temp_dir("ui-settings");
    let mut harness = harness(&dir, "Alpha\n", |config| config.sync = true);
    open_menu(&mut harness);
    entry(&harness, "Settings…").click();
    harness.run();

    let interval = harness.get_by_label("Sync every");
    interval.focus();
    harness.run();
    harness.key_combination_modifiers(Modifiers::COMMAND, &[Key::A]);
    harness.get_by_label("Sync every").type_text("25");
    harness.key_press(Key::Enter);
    harness.run();
    harness
        .get_by_role_and_label(Role::CheckBox, "Sync through the file's repository")
        .click();
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();

    assert!(harness.query_by_label("Settings").is_none());
    let config = AppConfig::load(&dir.join("config.toml"));
    assert!(!config.sync);
    assert_eq!(config.sync_interval_minutes, 25);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cancel_and_escape_leave_the_config_alone() {
    let dir = temp_dir("ui-settings-cancel");
    let mut harness = harness(&dir, "Alpha\n", |_| {});
    let before = std::fs::read_to_string(dir.join("config.toml")).unwrap();
    for close in [None, Some(Key::Escape)] {
        open_menu(&mut harness);
        entry(&harness, "Settings…").click();
        harness.run();
        harness
            .get_by_role_and_label(Role::CheckBox, "Sync through the file's repository")
            .click();
        harness.run();
        match close {
            None => harness.get_by_label("Cancel").click(),
            Some(key) => harness.key_press(key),
        }
        harness.run();
        assert!(harness.state().settings.is_none());
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("config.toml")).unwrap(),
        before
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_interval_above_the_field_range_survives_a_save() {
    let dir = temp_dir("ui-settings-long");
    let mut harness = harness(&dir, "Alpha\n", |config| {
        config.sync_interval_minutes = 2880;
    });
    open_menu(&mut harness);
    entry(&harness, "Settings…").click();
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();
    let config = AppConfig::load(&dir.join("config.toml"));
    assert_eq!(config.sync_interval_minutes, 2880);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_settings_shortcut_opens_settings_but_not_over_the_edit_dialog() {
    let dir = temp_dir("ui-settings-shortcut");
    let mut harness = harness(&dir, "Alpha\n", |_| {});
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings.is_some());
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().settings.is_none());

    harness.state_mut().open_editor(0);
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn quit_closes_the_window_through_the_normal_close() {
    let dir = temp_dir("ui-quit");
    let mut harness = harness(&dir, "Alpha\n", |_| {});
    open_menu(&mut harness);
    entry(&harness, "Quit").click();
    // The frame that handles the click sends the command.
    harness.step();
    let commands = &harness.output().viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(
        commands.contains(&egui::ViewportCommand::Close),
        "{commands:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

// Tabs (RTM-127).

/// The harness on `todo.txt` and `work.txt` ("Report"), both in the session, `work.txt` shown
/// when `work_active`, labelled "Work".
fn two_tabs(dir: &Path, work_active: bool) -> Harness<'static, App> {
    let work = dir.join("work.txt");
    std::fs::write(&work, "Report\n").unwrap();
    harness(dir, "Alpha\n", |config| {
        let todo = config.file_name.clone().unwrap();
        let work = work.to_string_lossy().into_owned();
        config.add_file(&todo);
        config.add_file(&work);
        config.files.last_mut().unwrap().name = Some("Work".into());
        if work_active {
            config.set_active(&work);
        }
    })
}

fn tab<'h>(harness: &'h Harness<'_, App>, name: &'h str) -> egui_kittest::Node<'h> {
    harness.get_by_role_and_label(Role::Tab, name)
}

/// Clicks the close button of the tab `name`, then answers the question with `answer`.
fn close_tab(harness: &mut Harness<'_, App>, name: &str, answer: &str) {
    let close = format!("Close {name}");
    harness.get_by_role_and_label(Role::Button, &close).click();
    harness.run();
    harness.get_by_label(&format!("Close {name}?"));
    harness.get_by_role_and_label(Role::Button, answer).click();
    harness.run();
}

#[test]
fn every_saved_file_has_a_tab_and_the_saved_one_is_shown() {
    let dir = temp_dir("ui-tabs-startup");
    let harness = two_tabs(&dir, true);

    assert!(
        !tab(&harness, "todo.txt")
            .accesskit_node()
            .is_selected()
            .unwrap_or(false)
    );
    assert_eq!(
        tab(&harness, "Work").accesskit_node().is_selected(),
        Some(true)
    );
    assert_eq!(rows(&harness), ["Report"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn clicking_a_tab_shows_its_file_and_quick_add_goes_there() {
    let dir = temp_dir("ui-tabs-switch");
    let mut harness = two_tabs(&dir, false);
    assert_eq!(rows(&harness), ["Alpha"]);

    tab(&harness, "Work").click();
    harness.run();
    assert_eq!(rows(&harness), ["Report"]);
    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    input.type_text("Send invoice");
    harness.key_press(Key::Enter);
    harness.run();

    let work = std::fs::read_to_string(dir.join("work.txt")).unwrap();
    assert!(work.contains("Send invoice"), "{work}");
    assert_eq!(file_lines(&dir), ["Alpha"]);
    let config = AppConfig::load(&dir.join("config.toml"));
    let canonical = dir.join("work.txt").canonicalize().unwrap();
    assert_eq!(config.file_name.as_deref(), canonical.to_str());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_a_tab_removes_it_and_its_entry() {
    let dir = temp_dir("ui-tabs-close");
    let mut harness = two_tabs(&dir, true);

    close_tab(&mut harness, "Work", "Cancel");
    tab(&harness, "Work");
    close_tab(&mut harness, "Work", "Close tab");

    assert!(harness.query_by_role_and_label(Role::Tab, "Work").is_none());
    assert_eq!(rows(&harness), ["Alpha"]);
    let config = AppConfig::load(&dir.join("config.toml"));
    let [entry] = config.files.as_slice() else {
        panic!("one entry: {:?}", config.files);
    };
    assert!(entry.path.ends_with("todo.txt"), "{entry:?}");
    assert_eq!(config.file_name.as_ref(), Some(&entry.path));

    close_tab(&mut harness, "todo.txt", "Close tab");
    harness.get_by_label_contains("No file loaded. Use New File or Load File");
    assert_eq!(AppConfig::load(&dir.join("config.toml")).files, []);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_add_button_offers_new_file_and_load_file() {
    let dir = temp_dir("ui-tabs-add");
    let mut harness = harness(&dir, "Alpha\n", |_| {});

    harness.get_by_label("Open a file").click();
    harness.run();

    for name in ["New File…", "Load File…"] {
        assert!(
            !self::entry(&harness, name).accesskit_node().is_disabled(),
            "{name}"
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_tab_cannot_be_closed_while_the_edit_dialog_is_open() {
    let dir = temp_dir("ui-tabs-editing");
    let mut harness = two_tabs(&dir, true);

    harness.get_by_role_and_label(Role::Label, "Report").click();
    harness.run();

    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Close Work")
            .accesskit_node()
            .is_disabled()
    );
    let _ = std::fs::remove_dir_all(dir);
}

// Moving an item to another open file (RTM-128).

/// Opens the edit dialog on the row `item`.
fn edit_row(harness: &mut Harness<'_, App>, item: &str) {
    harness.get_by_role_and_label(Role::Label, item).click();
    harness.run();
}

#[test]
fn move_to_is_hidden_with_a_single_file() {
    let dir = temp_dir("ui-move-single");
    let mut harness = harness(&dir, "Alpha\n", |_| {});

    edit_row(&mut harness, "Alpha");

    assert!(harness.query_by_label("Move to…").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_item_moves_to_another_tab_after_asking() {
    let dir = temp_dir("ui-move");
    let mut harness = two_tabs(&dir, false);

    edit_row(&mut harness, "Alpha");
    harness.get_by_label("Move to…").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Work").click();
    harness.run();
    harness.get_by_label("Move this item to Work?");
    harness.get_by_role_and_label(Role::Button, "Keep").click();
    harness.run();
    assert!(harness.query_by_label("Move this item to Work?").is_none());

    harness.get_by_label("Move to…").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Work").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Move").click();
    harness.run();

    assert!(harness.query_by_label("Edit Item").is_none());
    assert_eq!(file_lines(&dir), Vec::<String>::new());
    let work = std::fs::read_to_string(dir.join("work.txt")).unwrap();
    assert!(
        work.lines().nth(1).is_some_and(|l| l.ends_with("Alpha")),
        "{work}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

// The update notice and its settings (RTM-138).

/// Answers one request with a release list holding the stable release `version`.
#[cfg(feature = "update-check")]
fn serve_release(version: &str) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/releases.json", listener.local_addr().unwrap());
    let body = format!(
        r#"{{"schema": 1, "releases": [{{"version": "{version}", "channel": "stable", "date": "2026-11-06", "url": "https://rtm.example/news/{version}/"}}]}}"#
    );
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                break;
            }
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
    });
    (url, server)
}

#[cfg(feature = "update-check")]
const NOTICE: &str = "Rusty Todo.txt Manager 999.0.0 is available";

/// Steps frames until the notice shows, for 5 s at most: the check runs on its own thread.
#[cfg(feature = "update-check")]
fn wait_for_notice(harness: &mut Harness<'_, App>) {
    let start = Instant::now();
    while harness.query_by_label(NOTICE).is_none() {
        assert!(start.elapsed() < Duration::from_secs(5), "no notice");
        std::thread::sleep(Duration::from_millis(10));
        harness.step();
    }
}

#[cfg(feature = "update-check")]
#[test]
fn a_newer_release_shows_the_notice_download_opens_it_and_close_hides_it() {
    let dir = temp_dir("ui-update-notice");
    let (url, server) = serve_release("999.0.0");
    let mut harness = harness_at(&dir, "Alpha\n", url, |config| {
        config.check_for_updates = true;
    });
    // Due at once instead of a few seconds after start.
    harness.state_mut().updates.set(true, Instant::now());
    wait_for_notice(&mut harness);
    server.join().unwrap();

    harness.get_by_label("Download").click();
    harness.step();
    let opened: Vec<_> = harness
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(opened, ["https://rtm.example/news/999.0.0/"]);
    harness.run();
    // Download leaves the notice, so the page can be opened again.
    harness.get_by_label(NOTICE);

    harness.get_by_label("Close the update notice").click();
    harness.run();
    assert!(harness.query_by_label(NOTICE).is_none());
    // A later answer does not bring it back in this run.
    harness.state_mut().updates.notice = Some(Notice {
        version: "999.0.1".to_owned(),
        url: "https://rtm.example/news/999.0.1/".to_owned(),
    });
    harness.run();
    assert!(harness.query_by_label_contains("is available").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(feature = "update-check")]
#[test]
fn settings_turns_the_check_on_and_picks_the_channel() {
    let dir = temp_dir("ui-update-settings");
    let mut harness = harness(&dir, "Alpha\n", |_| {});
    open_menu(&mut harness);
    entry(&harness, "Settings…").click();
    harness.run();
    assert!(
        harness
            .get_by_role_and_label(Role::RadioButton, "Beta")
            .accesskit_node()
            .is_disabled(),
        "the channel waits for the check"
    );
    harness
        .get_by_role_and_label(Role::CheckBox, "Check for updates")
        .click();
    harness.run();
    harness
        .get_by_role_and_label(Role::RadioButton, "Beta")
        .click();
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();

    let config = AppConfig::load(&dir.join("config.toml"));
    assert!(config.check_for_updates);
    assert_eq!(config.update_channel, "beta");
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(feature = "update-check")]
#[test]
fn turning_the_check_off_hides_the_notice() {
    let dir = temp_dir("ui-update-off");
    let mut harness = harness(&dir, "Alpha\n", |config| config.check_for_updates = true);
    harness.state_mut().updates.notice = Some(Notice {
        version: "999.0.0".to_owned(),
        url: "https://rtm.example/news/999.0.0/".to_owned(),
    });
    harness.run();
    harness.get_by_label(NOTICE);

    open_menu(&mut harness);
    entry(&harness, "Settings…").click();
    harness.run();
    harness
        .get_by_role_and_label(Role::CheckBox, "Check for updates")
        .click();
    harness.run();
    harness.get_by_label("Save").click();
    harness.run();

    assert!(harness.query_by_label(NOTICE).is_none());
    assert!(!AppConfig::load(&dir.join("config.toml")).check_for_updates);
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(not(feature = "update-check"))]
#[test]
fn a_build_without_the_check_has_no_update_settings() {
    let dir = temp_dir("ui-update-none");
    let mut harness = harness(&dir, "Alpha\n", |_| {});
    open_menu(&mut harness);
    entry(&harness, "Settings…").click();
    harness.run();
    harness.get_by_label("Sync every");
    assert!(harness.query_by_label("Check for updates").is_none());
    let _ = std::fs::remove_dir_all(dir);
}

// Projects and contexts in the sidebar (RTM-120).

/// The sidebar's tag rows, as `<tag> <open count>`, in order: each tag label followed by its count.
fn tag_rows(harness: &Harness<'_, App>) -> Vec<String> {
    let texts: Vec<String> = harness
        .get_all_by_role(Role::Label)
        .filter_map(|node| {
            let node = node.accesskit_node();
            node.label().or_else(|| node.value())
        })
        .collect();
    texts
        .windows(2)
        .filter(|pair| pair[0].starts_with(['+', '@']) && pair[1].parse::<usize>().is_ok())
        .map(|pair| format!("{} {}", pair[0], pair[1]))
        .collect()
}

#[test]
fn the_sidebar_lists_projects_and_contexts_with_open_counts() {
    let dir = temp_dir("ui-tags");
    let contents = "Write the report +work @office\nCall the client +Work @phone\n\
                    x 2026-01-01 Plant bulbs +garden\n";
    let harness = harness(&dir, contents, |_| {});
    assert_eq!(
        tag_rows(&harness),
        ["+garden 0", "+work 2", "@office 1", "@phone 1"]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_sidebar_follows_changes_and_tabs() {
    let dir = temp_dir("ui-tags-follow");
    let mut harness = two_tabs(&dir, false);
    harness.get_by_label("No projects");
    harness.get_by_label("No contexts");

    let input = harness.get_by_role(Role::TextInput);
    input.focus();
    input.type_text("Water plants +home");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(tag_rows(&harness), ["+home 1"]);

    harness
        .get_by_role_and_label(Role::CheckBox, "Water plants")
        .click();
    harness.run();
    assert_eq!(tag_rows(&harness), ["+home 0"]);

    tab(&harness, "Work").click();
    harness.run();
    assert!(tag_rows(&harness).is_empty());
    harness.get_by_label("No projects");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_collapsed_section_is_saved_and_stays_collapsed() {
    let dir = temp_dir("ui-tags-collapse");
    let contents = "Write the report +work @office\n";
    let mut harness = harness(&dir, contents, |_| {});

    harness.get_by_label("Projects").click();
    harness.run();
    assert_eq!(tag_rows(&harness), ["@office 1"]);
    let config = AppConfig::load(&dir.join("config.toml"));
    assert!(config.projects_collapsed && !config.contexts_collapsed);

    // A new start on the saved config.
    let config_path = dir.join("config.toml");
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 650.0))
        .with_max_steps(60)
        .build_eframe(|cc| App::with_config(cc, config_path, closed_url()));
    harness.run();
    assert_eq!(tag_rows(&harness), ["@office 1"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_long_tag_list_scrolls_above_the_total() {
    let dir = temp_dir("ui-tags-overflow");
    let contents: String = (0..80)
        .map(|i| format!("Item {i} +project{i:02}\n"))
        .collect();
    let harness = harness(&dir, &contents, |_| {});
    let total = harness.get_by_label("Total items: 80");
    let bottom = total.accesskit_node().bounding_box().unwrap().y1;
    assert!(bottom <= 650.0, "{bottom}");
    let _ = std::fs::remove_dir_all(dir);
}
