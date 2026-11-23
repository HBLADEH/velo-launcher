//! On-demand process presence. Compare file identities rather than executable
//! names, and treat incomplete checks as unknown instead of starting twice.

use std::io;
use std::mem::size_of;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use crate::core::workspace::config::AppKey;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_INVALID_PARAMETER, ERROR_NO_MORE_FILES, FILETIME,
    GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::{
    GetLengthSid, GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, IsValidSid,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TOKEN_USER, TokenIntegrityLevel, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_ID_INFO, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FileIdInfo, GetFileInformationByHandleEx, OPEN_EXISTING,
};
use windows_sys::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, GetProcessTimes, OpenProcess,
    OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

const PROCESS_LIMIT: usize = 8192;

struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(handle: HANDLE) -> io::Result<Self> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(handle))
        }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: constructed only from an owned valid handle, never a pseudo handle.
        unsafe { CloseHandle(self.0) };
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    Running,
    NotRunning,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FileIdentity {
    volume: u64,
    index: [u8; 16],
}

pub(crate) fn file_identity(path: &Path) -> io::Result<FileIdentity> {
    let wide: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    if !path.is_absolute() || wide[..wide.len() - 1].contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "absolute path required",
        ));
    }
    // SAFETY: wide is NUL-terminated for this synchronous call. The returned
    // file handle is owned here; sharing flags preserve other users of the file.
    let file = OwnedHandle::new(unsafe {
        CreateFileW(
            wide.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        )
    })?;
    let mut info = FILE_ID_INFO::default();
    // SAFETY: info is writable and file stays alive until after the call.
    if unsafe {
        GetFileInformationByHandleEx(
            file.0,
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if info.FileId.Identifier == [0; 16] {
        return Err(io::Error::other("file identity unavailable"));
    }
    Ok(FileIdentity {
        volume: info.VolumeSerialNumber,
        index: info.FileId.Identifier,
    })
}

fn exited(process: HANDLE) -> bool {
    let mut code = 0;
    // SAFETY: borrowed limited-query handle and live output slot. Only a code
    // other than STILL_ACTIVE proves exit; query failures remain inconclusive.
    unsafe { GetExitCodeProcess(process, &mut code) != 0 && code != 259 }
}

fn session(pid: u32) -> io::Result<u32> {
    let mut id = 0;
    // SAFETY: id is a live output slot; no pointer is retained.
    if unsafe { ProcessIdToSessionId(pid, &mut id) } == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(id)
    }
}

fn user(process: HANDLE) -> io::Result<Vec<u8>> {
    let mut token = null_mut();
    // SAFETY: process is a live query handle or the current-process pseudo handle;
    // the successful token output becomes owned immediately.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = OwnedHandle::new(token)?;
    let mut length = 0;
    // SAFETY: null/zero queries only the required buffer length.
    unsafe { GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut length) };
    if !(size_of::<TOKEN_USER>() as u32..=4096).contains(&length) {
        return Err(io::Error::last_os_error());
    }
    // TOKEN_USER contains a pointer. usize storage guarantees its alignment.
    let mut buffer = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    // SAFETY: aligned buffer has at least length bytes, and token is still alive.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful TokenUser writes TOKEN_USER and its SID into buffer.
    // Copy the SID bytes while buffer lives; never retain its pointer.
    unsafe {
        let info = &*buffer.as_ptr().cast::<TOKEN_USER>();
        let size = GetLengthSid(info.User.Sid) as usize;
        if !(8..=68).contains(&size) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid process identity",
            ));
        }
        Ok(std::slice::from_raw_parts(info.User.Sid.cast::<u8>(), size).to_vec())
    }
}

fn image(process: HANDLE) -> io::Result<FileIdentity> {
    file_identity(&image_path(process)?)
}

fn image_path(process: HANDLE) -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    // SAFETY: buffer has length UTF-16 slots and caller holds process alive.
    if unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..length as usize],
    )))
}

pub(super) struct AppProcess(OwnedHandle);

impl AppProcess {
    pub(super) fn open(pid: u32) -> io::Result<Self> {
        // SAFETY: only a limited-query handle is needed, and it is owned here.
        OwnedHandle::new(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })
            .map(Self)
    }

    pub(super) fn born(&self) -> Option<u64> {
        let mut code = 0;
        let [mut creation, mut exit, mut kernel, mut user] = [FILETIME::default(); 4];
        // SAFETY: all timestamps live on this stack and the process handle is owned.
        (unsafe {
            GetExitCodeProcess(self.0.0, &mut code) != 0
                && code == 259
                && GetProcessTimes(self.0.0, &mut creation, &mut exit, &mut kernel, &mut user) != 0
        })
        .then(|| (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }

    pub(super) fn key(&self) -> Option<AppKey> {
        self.key_impl(true)
    }

    pub(super) fn key_without_disk(&self) -> Option<AppKey> {
        self.key_impl(false)
    }

    fn key_impl(&self, canonicalize: bool) -> Option<AppKey> {
        let mut len = 0;
        // SAFETY: the first call queries capacity. The second receives that many UTF-16 units.
        if unsafe { GetApplicationUserModelId(self.0.0, &mut len, null_mut()) }
            == ERROR_INSUFFICIENT_BUFFER
            && (2..=1024).contains(&len)
        {
            let mut id = vec![0; len as usize];
            if unsafe { GetApplicationUserModelId(self.0.0, &mut len, id.as_mut_ptr()) } == 0 {
                let end = id.iter().position(|&unit| unit == 0)?;
                return AppKey::package(&String::from_utf16(&id[..end]).ok()?);
            }
        }
        let path = image_path(self.0.0).ok()?;
        let path = if canonicalize {
            path.canonicalize().unwrap_or(path)
        } else {
            path
        };
        // These generic hosts do not identify the application behind the window.
        let name = path.file_name()?.to_str()?;
        if [
            "ApplicationFrameHost.exe",
            "RuntimeBroker.exe",
            "ShellExperienceHost.exe",
        ]
        .iter()
        .any(|host| name.eq_ignore_ascii_case(host))
        {
            return None;
        }
        AppKey::executable(&path)
    }

    pub(super) fn integrity(&self) -> Option<u32> {
        integrity(self.0.0)
    }

    pub(super) fn own_integrity() -> Option<u32> {
        // SAFETY: GetCurrentProcess is a pseudo handle and is never closed here.
        integrity(unsafe { GetCurrentProcess() })
    }
}

fn integrity(process: HANDLE) -> Option<u32> {
    let mut token = null_mut();
    // SAFETY: either a live owned query handle or the current process pseudo handle.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return None;
    }
    let token = OwnedHandle::new(token).ok()?;
    let mut size = 0u32;
    // SAFETY: null/zero queries the needed capacity, and the token stays alive.
    unsafe { GetTokenInformation(token.0, TokenIntegrityLevel, null_mut(), 0, &mut size) };
    if !(size_of::<TOKEN_MANDATORY_LABEL>() as u32..=4096).contains(&size) {
        return None;
    }
    let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
    // SAFETY: the buffer is aligned and long enough for the label and its SID.
    unsafe {
        if GetTokenInformation(
            token.0,
            TokenIntegrityLevel,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            return None;
        }
        let label = &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
        let sid = label.Label.Sid;
        if sid.is_null() || IsValidSid(sid) == 0 {
            return None;
        }
        let count = *GetSidSubAuthorityCount(sid);
        (count != 0).then(|| *GetSidSubAuthority(sid, u32::from(count) - 1))
    }
}

enum Entry {
    Known(FileIdentity),
    UnknownFile(FileIdentity),
    UnknownName(String),
}

fn inspect_process(
    pid: u32,
    current_session: u32,
    current_user: &[u8],
) -> io::Result<Option<Entry>> {
    // SAFETY: request a new limited-query handle, never elevation.
    let process =
        match OwnedHandle::new(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) }) {
            Ok(process) => process,
            // ToolHelp is a snapshot: this PID can disappear before OpenProcess.
            Err(error) if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
    let id = match image(process.0) {
        Ok(id) => id,
        Err(_) if exited(process.0) => return Ok(None),
        Err(error) => return Err(error),
    };
    match (session(pid), user(process.0)) {
        (Ok(session), Ok(user)) if session == current_session && user == current_user => {
            Ok(Some(Entry::Known(id)))
        }
        (Ok(_), Ok(_)) => Ok(None),
        _ if exited(process.0) => Ok(None),
        _ => Ok(Some(Entry::UnknownFile(id))),
    }
}

pub struct Snapshot {
    entries: Vec<Entry>,
}

impl Snapshot {
    /// Background-only, no polling. Incomplete enumeration is an error so a
    /// partial process list can never be mistaken for complete absence.
    pub fn capture() -> io::Result<Self> {
        // SAFETY: borrow the current-process pseudo handle; it must not be closed.
        let (current_session, current_user) =
            unsafe { (session(GetCurrentProcessId())?, user(GetCurrentProcess())?) };
        // SAFETY: ToolHelp returns a snapshot handle owned by this scope.
        let snapshot =
            OwnedHandle::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) })?;
        let mut raw = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..PROCESSENTRY32W::default()
        };
        let mut entries = Vec::new();
        // SAFETY: raw has the required dwSize and snapshot is alive.
        let mut found = unsafe { Process32FirstW(snapshot.0, &mut raw) } != 0;
        let mut count = 0;
        while found {
            count += 1;
            if count > PROCESS_LIMIT {
                return Err(io::Error::other("process snapshot limit exceeded"));
            }
            if raw.th32ProcessID != 0
                && !session(raw.th32ProcessID).is_ok_and(|id| id != current_session)
            {
                let name = String::from_utf16_lossy(
                    &raw.szExeFile[..raw
                        .szExeFile
                        .iter()
                        .position(|c| *c == 0)
                        .unwrap_or(raw.szExeFile.len())],
                );
                match inspect_process(raw.th32ProcessID, current_session, &current_user) {
                    Ok(Some(entry)) => entries.push(entry),
                    Ok(None) => {}
                    Err(_) => entries.push(Entry::UnknownName(name)),
                }
            }
            // SAFETY: same writable structure and still-owned snapshot.
            found = unsafe { Process32NextW(snapshot.0, &mut raw) } != 0;
        }
        // SAFETY: immediately read the error of the failed enumeration call.
        if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { entries })
    }

    pub fn presence(&self, executable: &Path) -> Presence {
        let Ok(target) = file_identity(executable) else {
            return Presence::Unknown;
        };
        if self
            .entries
            .iter()
            .any(|entry| matches!(entry, Entry::Known(id) if *id == target))
        {
            return Presence::Running;
        }
        let name = executable
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if self.entries.iter().any(|entry| match entry {
            Entry::UnknownFile(id) => *id == target,
            Entry::UnknownName(other) => other.to_lowercase() == name,
            Entry::Known(_) => false,
        }) {
            Presence::Unknown
        } else {
            Presence::NotRunning
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_snapshot_finds_this_process_but_not_a_same_name_file_elsewhere() {
        let executable = std::env::current_exe().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let other = directory.path().join(executable.file_name().unwrap());
        std::fs::write(&other, b"synthetic, never executable").unwrap();
        let snapshot = Snapshot::capture().unwrap();
        assert_eq!(snapshot.presence(&executable), Presence::Running);
        assert_eq!(snapshot.presence(&other), Presence::NotRunning);
        assert_eq!(
            snapshot.presence(&directory.path().join("missing.exe")),
            Presence::Unknown
        );
    }

    #[test]
    fn different_name_hard_link_matches_the_running_image() {
        let executable = std::env::current_exe().unwrap();
        let directory = tempfile::tempdir_in(executable.parent().unwrap()).unwrap();
        let alias = directory.path().join("different-image-name.exe");
        std::fs::hard_link(&executable, &alias).unwrap();
        assert_eq!(
            Snapshot::capture().unwrap().presence(&alias),
            Presence::Running
        );
    }

    #[test]
    fn related_access_failures_are_unknown_without_poisoning_unrelated_members() {
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("game.exe");
        std::fs::write(&executable, b"synthetic").unwrap();
        let id = file_identity(&executable).unwrap();
        let mut snapshot = Snapshot {
            entries: vec![Entry::UnknownName("other.exe".into())],
        };
        assert_eq!(snapshot.presence(&executable), Presence::NotRunning);
        snapshot.entries.push(Entry::UnknownName("GAME.EXE".into()));
        assert_eq!(snapshot.presence(&executable), Presence::Unknown);
        snapshot.entries = vec![Entry::UnknownFile(id)];
        assert_eq!(snapshot.presence(&executable), Presence::Unknown);
        snapshot.entries.push(Entry::Known(id));
        assert_eq!(snapshot.presence(&executable), Presence::Running);
    }
}
