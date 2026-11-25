use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::mem::size_of;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::ptr::null;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;
use windows_sys::Win32::System::Threading::{
    CREATE_NEW_CONSOLE, CREATE_NO_WINDOW, CREATE_SUSPENDED, CreateProcessW, GetExitCodeProcess,
    PROCESS_INFORMATION, ResumeThread, STARTUPINFOW, TerminateProcess, WaitForSingleObject,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VK_CONTROL,
    VK_LWIN, VK_MENU, VK_SHIFT,
};

use crate::core::workspace::actions::{Failure, Success};
use crate::core::workspace::dual::DualConfig;
use crate::core::workspace::user_actions::{Action, Script, UserAction};
use crate::core::{DataError, Result as DataResult};

use super::{
    Application, Foreground, ensure_input, input_process, keys_clear, launch, send, send_chord,
    shell,
};

pub fn execute_user(
    spec: &UserAction,
    target: Option<(Foreground, Application, Option<Foreground>)>,
    owner: isize,
    cancelled: impl Fn() -> bool,
) -> Result<Success, Failure> {
    if cancelled() {
        return Err(Failure::Cancelled);
    }
    if !super::current_input_desktop() {
        return Err(Failure::Permission);
    }
    if !spec.valid() || !spec.action.ready() {
        return Err(Failure::Unavailable);
    }
    match &spec.action {
        Action::InsertText { text } => {
            let (window, app, focus) = target.ok_or(Failure::TargetChanged)?;
            let process = input_process(window, &app)?;
            let focus = focus
                .or_else(|| window.focus())
                .ok_or(Failure::TargetChanged)?;
            // Each CRLF pair must become one character. Native Edit controls
            // expand both a CR and an LF into a newline when sent separately.
            let packets = text
                .replace("\r\n", "\n")
                .encode_utf16()
                .flat_map(|unit| [unicode(unit, false), unicode(unit, true)])
                .collect::<Vec<_>>();
            ensure_input(window, Some(focus), &process, &app)?;
            keys_clear(&[])?;
            if cancelled() {
                return Err(Failure::Cancelled);
            }
            let accepted = send(&packets);
            if accepted == packets.len() {
                Ok(Success::TextSubmitted)
            } else {
                if accepted % 2 == 1 {
                    send(&packets[accepted..accepted + 1]);
                }
                Err(Failure::Keyboard)
            }
        }
        Action::SendKeys { steps } => {
            let (window, app, focus) = target.ok_or(Failure::TargetChanged)?;
            let process = input_process(window, &app)?;
            let focus = focus
                .or_else(|| window.focus())
                .ok_or(Failure::TargetChanged)?;
            for step in steps {
                let key = step.parts().ok_or(Failure::Keyboard)?;
                let wait = Instant::now() + Duration::from_millis(u64::from(step.wait_ms));
                loop {
                    if cancelled() {
                        return Err(Failure::Cancelled);
                    }
                    ensure_input(window, Some(focus), &process, &app)?;
                    let remaining = wait.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    std::thread::sleep(remaining.min(Duration::from_millis(20)));
                }
                let mut keys = [(2, VK_CONTROL), (1, VK_MENU), (4, VK_SHIFT), (8, VK_LWIN)]
                    .into_iter()
                    .filter_map(|(mask, vk)| (key.modifiers & mask != 0).then_some(vk))
                    .collect::<Vec<_>>();
                keys.push(key.code as u16);
                send_chord(&keys)?;
            }
            Ok(Success::ShortcutSubmitted)
        }
        Action::RunScript(script) => {
            run_script(script, || cancelled() || !super::current_input_desktop())
        }
        Action::OpenTarget { target } => {
            let _apartment = shell::Apartment::new().map_err(|_| Failure::Shell)?;
            let result = if target.starts_with("https://") || target.starts_with("http://") {
                launch::open_url(target)
            } else {
                launch::open_native(target, owner, false)
            };
            result.map(|_| Success::Opened).map_err(|_| Failure::Shell)
        }
    }
}

fn unicode(unit: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wScan: unit,
                dwFlags: KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 },
                ..Default::default()
            },
        },
    }
}

pub fn review_scripts(config: &mut DualConfig) -> DataResult<()> {
    for item in config.actions.values_mut() {
        if let Action::RunScript(script) = &mut item.action
            && std::mem::take(&mut script.review)
        {
            let (_, digest) = open_script(script)
                .map_err(|error| DataError::new("verify Workspace script", error))?;
            script.digest = Some(digest);
        }
    }
    Ok(())
}

fn open_script(script: &Script) -> io::Result<(File, String)> {
    // Deny writes and replacement while cmd.exe opens the same reviewed file.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&script.path)?;
    let metadata = file.metadata()?;
    const MAX_BYTES: u64 = 2 * 1024 * 1024;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err(io::Error::other(
            "a local batch file of at most 2 MiB is required",
        ));
    }
    let mut hash = Sha256::new();
    // Bind authorization to the selected file, arguments and execution settings.
    let parameters = serde_json::to_vec(&(
        &script.path,
        &script.args,
        &script.directory,
        script.timeout_secs,
        script.show_console,
    ))?;
    hash.update((parameters.len() as u64).to_le_bytes());
    hash.update(parameters);
    let mut input = (&file).take(MAX_BYTES);
    let mut buffer = [0u8; 8192];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok((file, format!("{:x}", hash.finalize())))
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: a non-null handle is owned by the action worker until this drop.
        unsafe { CloseHandle(self.0) };
    }
}

fn run_script(script: &Script, cancelled: impl Fn() -> bool) -> Result<Success, Failure> {
    let (_file, digest) = open_script(script).map_err(|_| Failure::ScriptUnavailable)?;
    if script.digest.as_deref() != Some(digest.as_str()) {
        return Err(Failure::ScriptUntrusted);
    }
    if cancelled() {
        return Err(Failure::Cancelled);
    }
    let mut system = [0u16; 260];
    // SAFETY: the system directory is written into a live UTF-16 buffer.
    let length = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), system.len() as u32) } as usize;
    if length == 0 || length >= system.len() {
        return Err(Failure::ScriptUnavailable);
    }
    let cmd = format!("{}\\cmd.exe", String::from_utf16_lossy(&system[..length]));
    // /S removes the outer pair and preserves the quoted path/arguments. The
    // validator rejects substitutions and command metacharacters before here.
    let arguments = std::iter::once(script.path.as_str())
        .chain(script.args.iter().map(String::as_str))
        .map(|arg| format!("\"{arg}\""))
        .collect::<Vec<_>>()
        .join(" ");
    let mut command = wide(&format!("\"{cmd}\" /D /V:OFF /S /C \"{arguments}\""));
    let directory = if script.directory.is_empty() {
        Path::new(&script.path)
            .parent()
            .and_then(Path::to_str)
            .ok_or(Failure::ScriptUnavailable)?
    } else {
        &script.directory
    };
    // SAFETY: the job is private to this action. Its final handle terminates the
    // whole process tree on cancellation, timeout, completion or caller exit.
    unsafe {
        let job = CreateJobObjectW(null(), null());
        if job.is_null() {
            return Err(Failure::ScriptUnavailable);
        }
        let job = Handle(job);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            return Err(Failure::ScriptUnavailable);
        }
        let startup = STARTUPINFOW {
            cb: size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut info = PROCESS_INFORMATION::default();
        let flags = CREATE_SUSPENDED
            | if script.show_console {
                CREATE_NEW_CONSOLE
            } else {
                CREATE_NO_WINDOW
            };
        if CreateProcessW(
            wide(&cmd).as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            0,
            flags,
            null(),
            wide(directory).as_ptr(),
            &startup,
            &mut info,
        ) == 0
        {
            return Err(Failure::ScriptUnavailable);
        }
        let process = Handle(info.hProcess);
        let thread = Handle(info.hThread);
        if AssignProcessToJobObject(job.0, process.0) == 0 {
            TerminateProcess(process.0, 1);
            return Err(Failure::ScriptUnavailable);
        }
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        if ResumeThread(thread.0) == u32::MAX {
            return Err(Failure::ScriptUnavailable);
        }
        let deadline = Instant::now() + Duration::from_secs(u64::from(script.timeout_secs));
        loop {
            if cancelled() {
                return Err(Failure::Cancelled);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Failure::ScriptTimeout);
            }
            match WaitForSingleObject(process.0, left.as_millis().clamp(1, 50) as u32) {
                WAIT_TIMEOUT => continue,
                WAIT_OBJECT_0 => {
                    let mut code = 0;
                    if GetExitCodeProcess(process.0, &mut code) == 0 {
                        return Err(Failure::ScriptUnavailable);
                    }
                    return if code == 0 {
                        Ok(Success::ScriptCompleted)
                    } else {
                        Err(Failure::ScriptExit(code))
                    };
                }
                _ => return Err(Failure::ScriptUnavailable),
            }
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_batch_passes_arguments_and_changed_file_requires_review() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("sample folder");
        std::fs::create_dir(&path).unwrap();
        let file = path.join("literal.cmd");
        std::fs::write(
            &file,
            b"@echo off\r\nif \"%~1\"==\"example with spaces\" exit /b 0\r\nexit /b 7\r\n",
        )
        .unwrap();
        let mut script = Script {
            path: file.to_string_lossy().into_owned(),
            args: vec!["example with spaces".into()],
            directory: String::new(),
            timeout_secs: 5,
            show_console: false,
            digest: None,
            review: false,
            unknown: Default::default(),
        };
        script.digest = Some(open_script(&script).unwrap().1);
        assert_eq!(run_script(&script, || false), Ok(Success::ScriptCompleted));
        script.args = vec!["other".into()];
        assert_eq!(run_script(&script, || false), Err(Failure::ScriptUntrusted));
        script.digest = Some(open_script(&script).unwrap().1);
        assert_eq!(run_script(&script, || false), Err(Failure::ScriptExit(7)));
        std::fs::write(&file, b"@exit /b 0\r\n").unwrap();
        assert_eq!(run_script(&script, || false), Err(Failure::ScriptUntrusted));
    }
}
