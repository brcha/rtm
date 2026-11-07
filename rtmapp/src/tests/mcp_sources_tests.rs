use super::*;

use rtmsync::Vcs;

/// A config path in a fresh scratch directory under the system temp directory.
fn temp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rtmapp-mcp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("config.toml")
}

fn cleanup(path: &Path) {
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn every_file_with_the_active_one_first() {
    let path = temp_path("order");
    std::fs::write(
        &path,
        r#"
file_name = "/todo/work.txt"
sync = false

[[files]]
path = "/todo/home.txt"
name = "Home"
vcs = "pijul"

[[files]]
path = "/todo/work.txt"
vcs = "Git"
"#,
    )
    .unwrap();
    let files = sources(path.clone())().unwrap();

    let paths: Vec<_> = files.iter().map(|file| file.path.clone()).collect();
    assert_eq!(
        paths,
        [
            PathBuf::from("/todo/work.txt"),
            PathBuf::from("/todo/home.txt")
        ]
    );
    assert_eq!(files[0].name, None);
    assert_eq!(files[1].name.as_deref(), Some("Home"));
    assert_eq!(files[0].vcs, Some(Vcs::Git));
    assert_eq!(files[1].vcs, Some(Vcs::Pijul));
    assert!(files.iter().all(|file| !file.sync));
    cleanup(&path);
}

#[test]
fn a_config_changed_on_disk_is_read_again() {
    let path = temp_path("changed");
    std::fs::write(&path, "file_name = \"/todo/home.txt\"\n").unwrap();
    let sources = sources(path.clone());
    assert_eq!(sources().unwrap().len(), 1);

    std::fs::write(
        &path,
        "file_name = \"/todo/home.txt\"\n\n[[files]]\npath = \"/todo/work.txt\"\n",
    )
    .unwrap();
    let files = sources().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, PathBuf::from("/todo/home.txt"));
    cleanup(&path);
}

#[test]
fn an_old_config_with_only_file_name_gives_that_file() {
    let path = temp_path("old");
    std::fs::write(&path, "file_name = \"/todo/todo.txt\"\nvcs = \"git\"\n").unwrap();
    let files = sources(path.clone())().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, PathBuf::from("/todo/todo.txt"));
    assert_eq!(files[0].vcs, Some(Vcs::Git));
    assert!(files[0].sync);
    cleanup(&path);
}

#[test]
fn no_files_is_an_error_for_the_client() {
    let path = temp_path("none");
    // A missing config gives the defaults, which list no files.
    let error = sources(path.clone())().unwrap_err();
    assert_eq!(error, "No todo.txt files are configured in rtmapp");
    cleanup(&path);
}
