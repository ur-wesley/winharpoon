use eframe::egui;

use crate::config::chord_from_vk_mods;
use crate::util;

pub fn egui_key_to_vk(key: egui::Key) -> u32 {
    match key {
        egui::Key::A => 0x41,
        egui::Key::B => 0x42,
        egui::Key::C => 0x43,
        egui::Key::D => 0x44,
        egui::Key::E => 0x45,
        egui::Key::F => 0x46,
        egui::Key::G => 0x47,
        egui::Key::H => 0x48,
        egui::Key::I => 0x49,
        egui::Key::J => 0x4A,
        egui::Key::K => 0x4B,
        egui::Key::L => 0x4C,
        egui::Key::M => 0x4D,
        egui::Key::N => 0x4E,
        egui::Key::O => 0x4F,
        egui::Key::P => 0x50,
        egui::Key::Q => 0x51,
        egui::Key::R => 0x52,
        egui::Key::S => 0x53,
        egui::Key::T => 0x54,
        egui::Key::U => 0x55,
        egui::Key::V => 0x56,
        egui::Key::W => 0x57,
        egui::Key::X => 0x58,
        egui::Key::Y => 0x59,
        egui::Key::Z => 0x5A,
        egui::Key::Num0 => 0x30,
        egui::Key::Num1 => 0x31,
        egui::Key::Num2 => 0x32,
        egui::Key::Num3 => 0x33,
        egui::Key::Num4 => 0x34,
        egui::Key::Num5 => 0x35,
        egui::Key::Num6 => 0x36,
        egui::Key::Num7 => 0x37,
        egui::Key::Num8 => 0x38,
        egui::Key::Num9 => 0x39,
        egui::Key::F1 => 0x70,
        egui::Key::F2 => 0x71,
        egui::Key::F3 => 0x72,
        egui::Key::F4 => 0x73,
        egui::Key::F5 => 0x74,
        egui::Key::F6 => 0x75,
        egui::Key::F7 => 0x76,
        egui::Key::F8 => 0x77,
        egui::Key::F9 => 0x78,
        egui::Key::F10 => 0x79,
        egui::Key::F11 => 0x7A,
        egui::Key::F12 => 0x7B,
        egui::Key::Space => 0x20,
        egui::Key::Tab => 0x09,
        egui::Key::Backspace => 0x08,
        egui::Key::Enter => 0x0D,
        egui::Key::Escape => 0x1B,
        egui::Key::ArrowLeft => 0x25,
        egui::Key::ArrowUp => 0x26,
        egui::Key::ArrowRight => 0x27,
        egui::Key::ArrowDown => 0x28,
        egui::Key::OpenBracket => 0xDB,
        egui::Key::CloseBracket => 0xDD,
        egui::Key::Backtick => 0xC0,
        egui::Key::Minus => 0xBD,
        egui::Key::Equals => 0xBB,
        _ => 0,
    }
}

pub fn mods_from_egui(modifiers: egui::Modifiers) -> u32 {
    let mut mods = 0u32;
    if modifiers.ctrl {
        mods |= 0x0002;
    }
    if modifiers.alt {
        mods |= 0x0001;
    }
    if modifiers.shift {
        mods |= 0x0004;
    }
    if modifiers.command {
        mods |= 0x0008;
    }
    mods
}

pub fn chord_from_egui_key(key: egui::Key, modifiers: egui::Modifiers) -> Option<String> {
    let vk = egui_key_to_vk(key);
    if vk == 0 || util::is_modifier_vk(vk) {
        return None;
    }
    Some(chord_from_vk_mods(
        windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk as u16),
        mods_from_egui(modifiers) | 0x4000,
    ))
}

pub fn chord_from_egui_key_win32(key: egui::Key) -> Option<String> {
    let vk = egui_key_to_vk(key);
    if vk == 0 || util::is_modifier_vk(vk) {
        return None;
    }
    let mods = util::keyboard_modifiers();
    Some(chord_from_vk_mods(
        windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk as u16),
        mods,
    ))
}

pub enum ChordCaptureResult {
    Captured(String),
    Cancelled,
    Pending,
}

pub fn poll_chord_capture(ctx: &egui::Context, use_win32_mods: bool) -> ChordCaptureResult {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return ChordCaptureResult::Cancelled;
    }
    for event in ctx.input(|i| i.events.clone()) {
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = event
        {
            if key == egui::Key::Escape {
                continue;
            }
            let chord = if use_win32_mods {
                chord_from_egui_key_win32(key)
            } else {
                chord_from_egui_key(key, modifiers)
            };
            if let Some(chord) = chord {
                return ChordCaptureResult::Captured(chord);
            }
        }
    }
    ChordCaptureResult::Pending
}
