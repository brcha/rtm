// Embeds the app icon and version information in rtmapp.exe, so Explorer, the taskbar and Task Manager
// show them. Checks the target, not the host (`#[cfg(windows)]` would test the machine running this
// script).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    winresource::WindowsResource::new()
        .set_icon("assets/icons/icon.ico")
        .set("ProductName", "Rusty Todo.txt Manager")
        .set("FileDescription", "Rusty Todo.txt Manager")
        .compile()
        .expect("failed to compile the Windows resources");
}
