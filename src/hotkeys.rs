use std::collections::HashMap;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostMessageW, PostQuitMessage,
    RegisterClassW, TranslateMessage, WM_DESTROY, WM_HOTKEY, WM_TIMER, WNDCLASSW, WS_OVERLAPPED,
};

use crate::app::AppState;
use crate::config::{ConfigValidationError, HotkeyBinding};
use crate::log;

const CLASS_NAME: &str = "WinHarpoonHotkeyWindow";
const WM_RELOAD: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 1;
pub const WM_MARKS_KEY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 2;
pub const WM_JUMP_KEY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 3;
pub const WM_APP_MENU: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 4;
const WM_QUIT_APP: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 5;
pub const WM_LAUNCHER_KEY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_USER + 6;
pub const MARKS_POLL_TIMER_ID: usize = 9001;

static HOTKEY_HWND: AtomicIsize = AtomicIsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    Launcher,
    SameAppNext,
    SameAppPrev,
    MarkNext,
    MarkPrev,
    ToggleMark,
    Mark(u8),
    Jump(u8),
    MarksSwitcherNext,
    MarksSwitcherPrev,
    LaunchFavorite(usize),
}

#[derive(Debug, Clone)]
pub struct HotkeyRegistrationResult {
    pub chord: String,
    pub label: String,
    pub success: bool,
    pub error: Option<String>,
}

pub struct HotkeyManager {
    hwnd: HWND,
    id_map: HashMap<i32, HotkeyAction>,
    results: Vec<HotkeyRegistrationResult>,
}

unsafe impl Send for HotkeyManager {}
unsafe impl Sync for HotkeyManager {}

impl HotkeyManager {
    pub fn register(state: &Arc<Mutex<AppState>>, bindings: &[HotkeyBinding]) -> Self {
        log::debug(format!("HotkeyManager::register with {} bindings", bindings.len()));
        let hwnd = unsafe { create_message_window() };
        HOTKEY_HWND.store(hwnd.0 as isize, Ordering::SeqCst);
        log::debug(format!("hotkey hwnd: {:?}", hwnd.0));
        let mut manager = Self {
            hwnd,
            id_map: HashMap::new(),
            results: Vec::new(),
        };
        manager.apply_bindings(bindings);
        let conflict_count = manager.conflict_count();
        {
            let mut s = state.lock();
            s.hotkey_conflicts = conflict_count;
            s.registration_results = manager.results.clone();
        }
        manager.report_conflicts();
        manager
    }

    pub fn reload(&mut self, state: &Arc<Mutex<AppState>>, bindings: &[HotkeyBinding]) {
        log::debug(format!("HotkeyManager::reload with {} bindings", bindings.len()));
        self.unregister_all();
        self.apply_bindings(bindings);
        let conflict_count = self.conflict_count();
        {
            let mut s = state.lock();
            s.hotkey_conflicts = conflict_count;
            s.registration_results = self.results.clone();
        }
        self.report_conflicts();
    }

    pub fn conflict_count(&self) -> usize {
        self.results.iter().filter(|r| !r.success).count()
    }

    fn apply_bindings(&mut self, bindings: &[HotkeyBinding]) {
        self.results.clear();
        self.id_map.clear();
        let mut next_id: i32 = 1;

        for binding in bindings {
            if matches!(
                binding.action,
                HotkeyAction::Launcher
                    | HotkeyAction::Jump(_)
                    | HotkeyAction::MarksSwitcherNext
                    | HotkeyAction::MarksSwitcherPrev
            ) {
                log::debug(format!(
                    "skip RegisterHotKey for hook-managed binding {}",
                    binding.label
                ));
                continue;
            }
            let Some(parsed) = &binding.parsed else {
                log::debug(format!(
                    "skip binding {} (empty or unparseable): {:?}",
                    binding.label, binding.chord
                ));
                continue;
            };
            let id = next_id;
            next_id += 1;
            let ok = unsafe {
                RegisterHotKey(
                    Some(self.hwnd),
                    id,
                    HOT_KEY_MODIFIERS(parsed.modifiers),
                    parsed.vk as u32,
                )
                .is_ok()
            };
            let error = if ok {
                self.id_map.insert(id, binding.action);
                log::debug(format!(
                    "RegisterHotKey ok id={id} label={} chord={} mods=0x{:X} vk=0x{:X}",
                    binding.label, binding.chord, parsed.modifiers, parsed.vk
                ));
                None
            } else {
                log::warn(format!(
                    "RegisterHotKey failed id={id} label={} chord={}",
                    binding.label, binding.chord
                ));
                Some("already registered by another application".into())
            };
            self.results.push(HotkeyRegistrationResult {
                chord: binding.chord.clone(),
                label: binding.label.clone(),
                success: ok,
                error,
            });
        }
    }

    fn unregister_all(&mut self) {
        let ids: Vec<_> = self.id_map.keys().copied().collect();
        log::debug(format!("unregister_all: {} hotkeys", ids.len()));
        for id in ids {
            unsafe {
                let _ = UnregisterHotKey(Some(self.hwnd), id);
            }
        }
        self.id_map.clear();
    }

    fn report_conflicts(&self) {
        for result in &self.results {
            if !result.success {
                let msg = format!(
                    "{} ({}) is unavailable — {}",
                    result.chord,
                    result.label,
                    result.error.clone().unwrap_or_default()
                );
                log::warn(&msg);
            }
        }
        log::debug(format!(
            "hotkey registration done: {} ok, {} conflicts",
            self.results.iter().filter(|r| r.success).count(),
            self.conflict_count()
        ));
    }

    pub fn run_message_loop(
        &mut self,
        state: Arc<Mutex<AppState>>,
        on_action: impl Fn(HotkeyAction, &Arc<Mutex<AppState>>),
        on_reload: impl Fn(&mut Self, &Arc<Mutex<AppState>>),
        on_poll: impl Fn(),
    ) {
        log::debug("entering hotkey message loop");
        unsafe {
            let mut msg = std::mem::zeroed();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.message == WM_HOTKEY {
                    let id = msg.wParam.0 as i32;
                    if let Some(action) = self.id_map.get(&id).copied() {
                        log::debug(format!("WM_HOTKEY id={id} action={action:?}"));
                        on_action(action, &state);
                    } else {
                        log::warn(format!("WM_HOTKEY unknown id={id}"));
                    }
                } else if msg.message == WM_RELOAD {
                    log::debug("WM_RELOAD received");
                    on_reload(self, &state);
                } else if msg.message == WM_QUIT_APP {
                    log::debug("WM_QUIT_APP received");
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
                on_poll();
            }
        }
    }
}

pub fn post_reload() {
    let hwnd = HOTKEY_HWND.load(Ordering::SeqCst);
    log::debug(format!("post_reload hwnd={hwnd}"));
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_RELOAD, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn hotkey_hwnd() -> Option<HWND> {
    let hwnd = HOTKEY_HWND.load(Ordering::SeqCst);
    if hwnd == 0 {
        None
    } else {
        Some(HWND(hwnd as *mut _))
    }
}

pub fn post_quit() {
    let hwnd = HOTKEY_HWND.load(Ordering::SeqCst);
    log::debug(format!("post_quit hwnd={hwnd}"));
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(hwnd as *mut _)), WM_QUIT_APP, WPARAM(0), LPARAM(0));
        }
    } else {
        unsafe {
            PostQuitMessage(0);
        }
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        log::debug("HotkeyManager drop");
        self.unregister_all();
    }
}

pub fn report_config_errors(errors: &[ConfigValidationError]) {
    for err in errors {
        match err {
            ConfigValidationError::DuplicateBinding { chord, first, second } => {
                let msg = format!("duplicate binding {chord} for {first} and {second}");
                log::warn(&msg);
                log::notify("WinHarpoon config error", &msg);
            }
            ConfigValidationError::InvalidChord { label, chord, reason } => {
                let msg = format!("invalid chord {chord} for {label}: {reason}");
                log::warn(&msg);
                log::notify("WinHarpoon config error", &msg);
            }
        }
    }
}

unsafe fn create_message_window() -> HWND {
    let class_name = super::util::wide(CLASS_NAME);
    let hinstance = GetModuleHandleW(None).unwrap();

    let wc = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: hinstance.into(),
        lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    RegisterClassW(&wc);

    CreateWindowExW(
        Default::default(),
        windows::core::PCWSTR(class_name.as_ptr()),
        windows::core::PCWSTR(class_name.as_ptr()),
        WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        None,
        None,
        Some(hinstance.into()),
        None,
    )
    .expect("create hotkey window")
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_DESTROY {
        log::debug("hotkey window WM_DESTROY");
        PostQuitMessage(0);
        return LRESULT(0);
    }
    if msg == WM_TIMER && wparam.0 == MARKS_POLL_TIMER_ID {
        crate::marks_switcher::hook::poll_active();
        return LRESULT(0);
    }
    if msg == WM_MARKS_KEY {
        let vk = wparam.0 as u32;
        let key_up = lparam.0 != 0;
        crate::marks_switcher::hook::dispatch_key(vk, key_up);
        return LRESULT(0);
    }
    if msg == WM_JUMP_KEY {
        let slot = wparam.0 as u8;
        crate::marks_switcher::hook::dispatch_jump(slot);
        return LRESULT(0);
    }
    if msg == WM_LAUNCHER_KEY {
        crate::launcher::open();
        return LRESULT(0);
    }
    if msg == WM_APP_MENU {
        let x = wparam.0 as i32;
        let y = lparam.0 as i32;
        crate::apps::hook::dispatch_app_menu(x, y);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
