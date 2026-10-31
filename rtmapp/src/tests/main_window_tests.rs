use super::*;

#[test]
fn embedded_icon_decodes() {
    let icon = eframe::icon_data::from_png_bytes(ICON_PNG).unwrap();
    assert_eq!((icon.width, icon.height), (256, 256));
}

#[cfg(windows)]
#[test]
fn app_user_model_id_is_set() {
    use windows_sys::Win32::{
        System::Com::CoTaskMemFree, UI::Shell::GetCurrentProcessExplicitAppUserModelID,
    };

    assert!(set_app_user_model_id() >= 0);
    let mut id: windows_sys::core::PWSTR = std::ptr::null_mut();
    // SAFETY: on success Windows returns a NUL-terminated string allocated with CoTaskMemAlloc,
    // read here up to the NUL and then freed once.
    let read = unsafe {
        assert!(GetCurrentProcessExplicitAppUserModelID(&mut id) >= 0);
        let len = (0..).take_while(|&i| *id.add(i) != 0).count();
        let read = String::from_utf16(std::slice::from_raw_parts(id, len));
        CoTaskMemFree(id.cast::<std::ffi::c_void>());
        read
    };
    assert_eq!(read.unwrap(), APP_USER_MODEL_ID);
}
