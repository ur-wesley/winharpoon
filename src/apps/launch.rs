use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Com::{
    CoCreateInstance, CLSCTX_INPROC_SERVER,
};
use windows::Win32::System::Threading::{
    CreateProcessW, CREATE_BREAKAWAY_FROM_JOB, CREATE_UNICODE_ENVIRONMENT, PROCESS_CREATION_FLAGS,
    PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::log;
use crate::util;

use super::AppEntry;

pub fn looks_like_shell_injection(args: &str) -> bool {
    let lower = args.to_ascii_lowercase();
    lower.contains('&')
        || lower.contains('|')
        || lower.contains(';')
        || lower.contains("shell:")
        || lower.contains("cmd ")
        || lower.contains("/c ")
        || lower.contains("powershell")
}

pub fn is_allowed_target(target: &Path) -> bool {
    if target.as_os_str().is_empty() || !target.is_absolute() {
        return false;
    }
    matches!(
        target
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "exe" | "msi" | "bat" | "cmd"
    )
}

pub fn launch(entry: &AppEntry) -> bool {
    if let Some(aumid) = &entry.aumid {
        if launch_aumid(aumid, &entry.args) {
            return true;
        }
        log::debug(format!(
            "apps: AUMID launch failed for {aumid}, falling back"
        ));
    }
    if entry.source_lnk.is_file() {
        shell_open(&entry.source_lnk)
    } else {
        launch_path(&entry.target, &entry.args)
    }
}

pub fn launch_path(target: &Path, args: &str) -> bool {
    launch_detached(target, args)
}

fn shell_open(path: &Path) -> bool {
    let op = util::wide("open");
    let file = util::wide(&path.to_string_lossy());
    unsafe {
        let result = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
        let ok = (result.0 as isize) > 32;
        if ok {
            log::debug(format!("apps: launched {}", path.display()));
        } else {
            log::warn(format!(
                "apps: launch failed {} code={:?}",
                path.display(),
                result.0
            ));
        }
        ok
    }
}

fn launch_detached(target: &Path, args: &str) -> bool {
    // Security: only launch absolute paths with plausible executable extensions.
    // Blocks cache/.lnk poisoning like target=evil.exe or target=doc.pdf.
    if !is_allowed_target(target) {
        log::warn(format!("apps: refusing target {}", target.display()));
        return false;
    }
    // Security: warn on shell-metachar args from untrusted .lnk files.
    // Still launches (compat) but makes injection visible in logs.
    let trimmed = args.trim();
    if !trimmed.is_empty() && looks_like_shell_injection(trimmed) {
        log::warn(format!(
            "apps: suspicious args for {}: {trimmed}",
            target.display()
        ));
    }
    // Use application-name form of CreateProcessW so the exe path can't be
    // reinterpreted as part of the command line (arg-injection hardening).
    let target_wide = util::wide(&target.to_string_lossy());
    let mut args_wide;
    let cmd_ptr = if trimmed.is_empty() {
        std::ptr::null_mut()
    } else {
        args_wide = util::wide(trimmed);
        args_wide.as_mut_ptr()
    };
    let working_dir = target
        .parent()
        .map(|p| util::wide(&p.to_string_lossy()))
        .unwrap_or_default();

    unsafe {
        let si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();
        let flags =
            PROCESS_CREATION_FLAGS(CREATE_BREAKAWAY_FROM_JOB.0 | CREATE_UNICODE_ENVIRONMENT.0);
        let result = if cmd_ptr.is_null() {
            CreateProcessW(
                PCWSTR(target_wide.as_ptr()),
                None,
                None,
                None,
                false,
                flags,
                None,
                if working_dir.is_empty() {
                    PCWSTR::null()
                } else {
                    PCWSTR(working_dir.as_ptr())
                },
                &si,
                &mut pi,
            )
        } else {
            CreateProcessW(
                PCWSTR(target_wide.as_ptr()),
                Some(windows::core::PWSTR(cmd_ptr)),
                None,
                None,
                false,
                flags,
                None,
                if working_dir.is_empty() {
                    PCWSTR::null()
                } else {
                    PCWSTR(working_dir.as_ptr())
                },
                &si,
                &mut pi,
            )
        };
        if result.is_ok() {
            let _ = CloseHandle(pi.hProcess);
            let _ = CloseHandle(pi.hThread);
            log::debug(format!("apps: launched detached {}", target.display()));
            true
        } else {
            log::warn(format!("apps: detached launch failed {}", target.display()));
            false
        }
    }
}

fn launch_aumid(aumid: &str, args: &str) -> bool {    use windows::Win32::UI::Shell::IApplicationActivationManager;

    let activator: IApplicationActivationManager = unsafe {
        match CoCreateInstance(
            &windows::Win32::UI::Shell::ApplicationActivationManager,
            None,
            CLSCTX_INPROC_SERVER,
        ) {
            Ok(a) => a,
            Err(e) => {
                log::debug(format!("apps: AUMID activator CoCreate failed {e:?}"));
                return false;
            }
        }
    };

    let aumid_wide = util::wide(aumid);
    let args_wide = util::wide(args);
    unsafe {
        let hr = activator.ActivateApplication(
            PCWSTR(aumid_wide.as_ptr()),
            PCWSTR(args_wide.as_ptr()),
            windows::Win32::UI::Shell::ACTIVATEOPTIONS(0),
        );
        if let Err(e) = hr {
            log::debug(format!("apps: ActivateApplication failed {e:?} for {aumid}"));
            return false;
        }
        log::debug(format!("apps: activated AUMID {aumid}"));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{is_allowed_target, looks_like_shell_injection};
    use std::path::Path;

    #[test]
    fn flags_shell_metachars() {
        assert!(looks_like_shell_injection("/c payload"));
        assert!(looks_like_shell_injection("a & b"));
        assert!(looks_like_shell_injection("shell:appsfolder\\x"));
        assert!(looks_like_shell_injection("powershell -e x"));
        assert!(!looks_like_shell_injection("--new-window https://x"));
    }

    #[test]
    fn rejects_relative_and_non_exe() {
        assert!(!is_allowed_target(Path::new("evil.exe")));
        assert!(!is_allowed_target(Path::new(r"C:\doc.pdf")));
        assert!(!is_allowed_target(Path::new("")));
        assert!(is_allowed_target(Path::new(r"C:\a\b.exe")));
        assert!(is_allowed_target(Path::new(r"C:\a\b.bat")));
    }
}