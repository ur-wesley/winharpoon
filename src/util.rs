pub fn wide(input: &str) -> Vec<u16> {
    input.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn from_wide(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(buffer.get(..end).unwrap_or_default())
}

/// True while the given virtual key is physically held down.
///
/// Shared by the low-level hooks and the modifier queries so the
/// `GetAsyncKeyState` high-bit test exists in exactly one place.
pub fn is_vk_down(vk: u16) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { GetAsyncKeyState(crate::win_cast::vk_to_i32(vk)) < 0 }
}

/// True if any of the given virtual keys (usually a base key plus its
/// left/right variants) is physically held down.
pub fn any_vk_down(vks: &[u16]) -> bool {
    vks.iter().any(|&vk| is_vk_down(vk))
}

pub fn keyboard_modifiers() -> u32 {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
        VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
    };

    let mut mods = 0u32;
    if any_vk_down(&[VK_LWIN.0, VK_RWIN.0]) {
        mods |= MOD_WIN.0;
    }
    if any_vk_down(&[VK_SHIFT.0, VK_LSHIFT.0, VK_RSHIFT.0]) {
        mods |= MOD_SHIFT.0;
    }
    if any_vk_down(&[VK_CONTROL.0, VK_LCONTROL.0, VK_RCONTROL.0]) {
        mods |= MOD_CONTROL.0;
    }
    if any_vk_down(&[VK_MENU.0, VK_LMENU.0, VK_RMENU.0]) {
        mods |= MOD_ALT.0;
    }
    mods
}

pub fn is_modifier_vk(vk: u32) -> bool {
    matches!(
        vk,
        0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5
    )
}

pub fn release_stuck_modifier_keys() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_KEYUP};

    crate::log::debug("release_stuck_modifier_keys");
    unsafe {
        for vk in [
            0x5B_u8, 0x5C, 0x10, 0x11, 0x12, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5,
        ] {
            keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
        }
    }
}
