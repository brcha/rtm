use super::*;

/// `plain_path` of `path`, as a string.
fn plain(path: &str) -> String {
    plain_path(Path::new(path)).to_string_lossy().into_owned()
}

#[cfg(windows)]
#[test]
fn a_verbatim_drive_path_becomes_plain() {
    assert_eq!(plain(r"\\?\C:\Users\a\todo.txt"), r"C:\Users\a\todo.txt");
    // The standard library reads the drive letter in upper case.
    assert_eq!(plain(r"\\?\d:\todo list.txt"), r"D:\todo list.txt");
    assert_eq!(plain(r"\\?\C:\"), r"C:\");
}

#[cfg(windows)]
#[test]
fn a_verbatim_unc_path_becomes_plain() {
    assert_eq!(
        plain(r"\\?\UNC\server\share\todo\todo.txt"),
        r"\\server\share\todo\todo.txt"
    );
}

#[cfg(windows)]
#[test]
fn a_path_whose_plain_form_names_another_file_stays_verbatim() {
    for path in [
        r"\\?\C:\a\..\todo.txt",
        r"\\?\C:\a\.\todo.txt",
        r"\\?\C:\todo.txt.",
        r"\\?\C:\todo.txt ",
        r"\\?\C:\dir\CON",
        r"\\?\C:\dir\con.txt",
        r"\\?\C:\dir\COM1.txt",
        r"\\?\C:\dir\lpt9",
        r"\\?\C:\a/b\todo.txt",
        r"\\?\UNC\server\share\NUL.txt",
    ] {
        assert_eq!(plain(path), path);
    }
}

#[cfg(windows)]
#[test]
fn names_that_only_look_like_devices_are_converted() {
    assert_eq!(plain(r"\\?\C:\console.txt"), r"C:\console.txt");
    assert_eq!(plain(r"\\?\C:\COM10.txt"), r"C:\COM10.txt");
    assert_eq!(plain(r"\\?\C:\.config\todo.txt"), r"C:\.config\todo.txt");
}

#[cfg(windows)]
#[test]
fn other_forms_are_returned_unchanged() {
    for path in [
        r"\\?\c:foo",
        r"\\.\pipe\todo",
        r"\\?\GLOBALROOT\Device\todo.txt",
        r"C:\already\plain.txt",
        r"\\server\share\todo.txt",
        r"todo.txt",
    ] {
        assert_eq!(plain(path), path);
    }
}

#[test]
fn a_unix_path_is_returned_unchanged() {
    assert_eq!(plain("/home/user/todo.txt"), "/home/user/todo.txt");
}
