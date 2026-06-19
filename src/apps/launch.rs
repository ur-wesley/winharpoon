use std::path::Path;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{
    CreateProcessW, CREATE_BREAKAWAY_FROM_JOB, CREATE_UNICODE_ENVIRONMENT, PROCESS_CREATION_FLAGS,
    PROCESS_INFORMATION, STARTUPINFOW,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::log;
use crate::util;

use super::AppEntry;

pub fn launch(entry: &AppEntry) -> bool {
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
    let mut command = format!("\"{}\"", target.display());
    let trimmed = args.trim();
    if !trimmed.is_empty() {
        command.push(' ');
        command.push_str(trimmed);
    }
    let mut command_wide = util::wide(&command);
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
        let result = CreateProcessW(
            None,
            Some(PWSTR(command_wide.as_mut_ptr())),
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
        );
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
