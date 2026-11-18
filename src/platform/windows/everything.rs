//! Official Everything Unicode Query2 WM_COPYDATA IPC.
//! Protocol values/layouts follow the official MIT-licensed SDK header.

use std::cell::{Cell, RefCell};
use std::env;
use std::ffi::c_void;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{ERROR_TIMEOUT, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::COPYDATASTRUCT;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    FindWindowW, HWND_MESSAGE, PM_REMOVE, PeekMessageW, RegisterClassW, SMTO_ABORTIFHUNG,
    SMTO_BLOCK, SendMessageTimeoutW, TranslateMessage, UnregisterClassW, WM_COPYDATA, WM_USER,
    WNDCLASSW,
};

use crate::core::file_search::{self, FileHit};

const EVERYTHING_CLASS: &str = "EVERYTHING_TASKBAR_NOTIFICATION";
const QUERY2_UNICODE: usize = 18;
const DB_LOADED: usize = 401;
const QUERY_FULL_PATH: u32 = 4;
const SORT_NAME_ASCENDING: u32 = 1;
const FOLDER: u32 = 1;
const MAX_REPLY_BYTES: usize = 4 * 1024 * 1024;
pub type QueryResult = Result<(Vec<FileHit>, usize), Error>;
type Notify = dyn Fn(u64, String, u32, QueryResult) + Send + Sync;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NotRunning,
    NotIndexed,
    Incompatible,
    Timeout,
    Protocol,
    Ipc,
    Window,
    Launch,
    Cancelled,
}

#[repr(C, packed(1))]
struct QueryHeader {
    reply_window: u32,
    reply_message: u32,
    search_flags: u32,
    offset: u32,
    max_results: u32,
    request_flags: u32,
    sort_type: u32,
}
#[repr(C, packed(1))]
struct ListHeader {
    total: u32,
    count: u32,
    offset: u32,
    flags: u32,
    sort: u32,
}
#[repr(C, packed(1))]
struct ItemHeader {
    flags: u32,
    data_offset: u32,
}

thread_local! {
    static REPLY: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
    static EXPECTED_REPLY: Cell<u32> = const { Cell::new(0) };
    static EXPECTED_SENDER: Cell<isize> = const { Cell::new(0) };
}
static QUERY_IDS: AtomicU32 = AtomicU32::new(0);
static ACTIVE_WORKERS: AtomicU32 = AtomicU32::new(0);
/// Counts active IPC threads, including destruction of their native windows.
pub fn active_workers() -> u32 {
    ACTIVE_WORKERS.load(Ordering::Acquire)
}
struct ActiveWorker;
impl Drop for ActiveWorker {
    fn drop(&mut self) {
        ACTIVE_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

struct Request {
    query: String,
    offset: u32,
    generation: u64,
    epoch: u64,
}
struct Pending {
    latest: Option<Request>,
    closed: bool,
}
struct Mailbox {
    pending: Mutex<Pending>,
    wake: Condvar,
    epoch: AtomicU64,
}

pub struct Worker {
    mailbox: Arc<Mailbox>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn start(
        notify: impl Fn(u64, String, u32, Result<(Vec<FileHit>, usize), Error>) + Send + Sync + 'static,
    ) -> std::io::Result<Self> {
        let mailbox = Arc::new(Mailbox {
            pending: Mutex::new(Pending {
                latest: None,
                closed: false,
            }),
            wake: Condvar::new(),
            epoch: AtomicU64::new(0),
        });
        let worker_mailbox = mailbox.clone();
        let thread = std::thread::Builder::new()
            .name("velo-everything-ipc".into())
            .spawn(move || worker(worker_mailbox, Box::new(notify)))?;
        Ok(Self {
            mailbox,
            thread: Some(thread),
        })
    }
    pub fn query(&self, query: String, offset: u32, generation: u64) -> bool {
        let Ok(mut pending) = self.mailbox.pending.lock() else {
            return false;
        };
        if pending.closed {
            return false;
        }
        pending.latest = Some(Request {
            query,
            offset,
            generation,
            epoch: self.mailbox.epoch.fetch_add(1, Ordering::AcqRel) + 1,
        });
        self.mailbox.wake.notify_one();
        true
    }
    pub fn stop(&mut self) {
        self.request_stop();
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            eprintln!("event=everything_worker_shutdown_error");
        }
    }
    pub fn request_stop(&self) {
        self.mailbox.epoch.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut pending) = self.mailbox.pending.lock() {
            pending.closed = true;
            pending.latest = None;
            self.mailbox.wake.notify_all();
        }
    }
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn accepting(&self) -> bool {
        self.mailbox
            .pending
            .lock()
            .is_ok_and(|pending| !pending.closed)
    }
    pub fn cancel(&self) {
        self.mailbox.epoch.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut pending) = self.mailbox.pending.lock() {
            pending.latest = None;
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}

fn take_request(mailbox: &Mailbox, timeout: Duration) -> Option<Request> {
    let deadline = Instant::now() + timeout;
    let mut pending = mailbox.pending.lock().ok()?;
    loop {
        if pending.closed {
            return None;
        }
        if let Some(request) = pending.latest.take() {
            return Some(request);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            pending.closed = true;
            return None;
        }
        let (next, result) = mailbox.wake.wait_timeout(pending, remaining).ok()?;
        pending = next;
        if result.timed_out() && pending.latest.is_none() {
            pending.closed = true;
            return None;
        }
    }
}

fn coalesce(mailbox: &Mailbox, mut request: Request) -> Option<Request> {
    let deadline = Instant::now() + file_search::QUERY_DEBOUNCE;
    let mut pending = mailbox.pending.lock().ok()?;
    loop {
        if pending.closed {
            return None;
        }
        if let Some(latest) = pending.latest.take() {
            request = latest;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Some(request);
        }
        let (next, _) = mailbox.wake.wait_timeout(pending, remaining).ok()?;
        pending = next;
    }
}

fn worker(mailbox: Arc<Mailbox>, notify: Box<Notify>) {
    ACTIVE_WORKERS.fetch_add(1, Ordering::AcqRel);
    let _active = ActiveWorker;
    while let Some(request) = take_request(&mailbox, file_search::IDLE_SHUTDOWN)
        .and_then(|request| coalesce(&mailbox, request))
    {
        if request.query.is_empty() {
            continue;
        }
        // A reply HWND belongs to one query. Retiring it before going idle
        // prevents a late synchronous WM_COPYDATA from blocking Everything
        // while this worker waits on its request mailbox.
        let result = create_receiver().and_then(|hwnd| {
            let result = search(
                hwnd,
                &request.query,
                request.offset,
                Some((&mailbox.epoch, request.epoch)),
            );
            EXPECTED_REPLY.with(|expected| expected.set(0));
            destroy_receiver(hwnd);
            REPLY.with(|reply| reply.borrow_mut().take());
            result
        });
        if mailbox.epoch.load(Ordering::Acquire) == request.epoch {
            notify(request.generation, request.query, request.offset, result);
        }
    }
    if let Ok(mut pending) = mailbox.pending.lock() {
        pending.closed = true;
        pending.latest = None;
    }
}

fn search(
    receiver: HWND,
    query: &str,
    offset: u32,
    cancellation: Option<(&AtomicU64, u64)>,
) -> Result<(Vec<FileHit>, usize), Error> {
    let class = crate::platform::windows::instance::wide(EVERYTHING_CLASS);
    // SAFETY: class is NUL terminated and only an existing local HWND is read.
    let target = unsafe { FindWindowW(class.as_ptr(), null()) };
    if target.is_null() {
        return Err(Error::NotRunning);
    }
    let deadline = Instant::now() + file_search::QUERY_TIMEOUT;
    let mut loaded = 0usize;
    let timeout = remaining_ms(deadline)?;
    // SAFETY: reads the database state of the discovered endpoint, with a deadline.
    let checked = unsafe {
        SendMessageTimeoutW(
            target,
            WM_USER,
            DB_LOADED,
            0,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            timeout,
            &mut loaded,
        )
    };
    if checked == 0 {
        return Err(Error::Timeout);
    }
    if loaded == 0 {
        let version = |id| -> Result<usize, Error> {
            let mut value = 0;
            // SAFETY: version queries only write the live stack result.
            let received = unsafe {
                SendMessageTimeoutW(
                    target,
                    WM_USER,
                    id,
                    0,
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    remaining_ms(deadline)?,
                    &mut value,
                )
            };
            if received == 0 {
                Err(Error::Timeout)
            } else {
                Ok(value)
            }
        };
        return if (version(0)?, version(1)?) < (1, 4) {
            Err(Error::Incompatible)
        } else {
            Err(Error::NotIndexed)
        };
    }

    REPLY.with(|reply| *reply.borrow_mut() = None);
    let reply_id = QUERY_IDS
        .fetch_add(1, Ordering::Relaxed)
        .wrapping_add(1)
        .max(1);
    EXPECTED_REPLY.with(|expected| expected.set(reply_id));
    EXPECTED_SENDER.with(|expected| expected.set(target as isize));
    let header = QueryHeader {
        reply_window: receiver as usize as u32,
        reply_message: reply_id,
        search_flags: 0,
        offset,
        max_results: file_search::PAGE_SIZE,
        request_flags: QUERY_FULL_PATH,
        sort_type: SORT_NAME_ASCENDING,
    };
    // SAFETY: the packed header contains only fully initialized u32 fields.
    let mut request = unsafe {
        std::slice::from_raw_parts(
            (&header as *const QueryHeader).cast::<u8>(),
            std::mem::size_of::<QueryHeader>(),
        )
        .to_vec()
    };
    for unit in query.encode_utf16().chain([0]) {
        request.extend(unit.to_le_bytes());
    }
    let mut copydata = COPYDATASTRUCT {
        dwData: QUERY2_UNICODE,
        cbData: u32::try_from(request.len()).map_err(|_| Error::Protocol)?,
        lpData: request.as_mut_ptr().cast::<c_void>(),
    };
    let mut accepted = 0usize;
    let timeout = remaining_ms(deadline)?;
    // SAFETY: Windows copies this owned packet synchronously before returning.
    let sent = unsafe {
        SendMessageTimeoutW(
            target,
            WM_COPYDATA,
            receiver as usize,
            (&mut copydata as *mut COPYDATASTRUCT) as LPARAM,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            timeout,
            &mut accepted,
        )
    };
    if sent == 0 {
        return Err(
            if unsafe { windows_sys::Win32::Foundation::GetLastError() } == ERROR_TIMEOUT {
                Error::Timeout
            } else {
                Error::Ipc
            },
        );
    }
    if accepted == 0 {
        return Err(Error::Incompatible);
    }
    loop {
        if cancellation.is_some_and(|(epoch, expected)| epoch.load(Ordering::Acquire) != expected) {
            EXPECTED_REPLY.with(|expected| expected.set(0));
            return Err(Error::Cancelled);
        }
        let mut message = windows_sys::Win32::UI::WindowsAndMessaging::MSG::default();
        // SAFETY: dispatch only messages for this thread's own receiver window.
        while unsafe { PeekMessageW(&mut message, receiver, WM_COPYDATA, WM_COPYDATA, PM_REMOVE) }
            != 0
        {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if let Some(bytes) = REPLY.with(|reply| reply.borrow_mut().take()) {
            EXPECTED_REPLY.with(|expected| expected.set(0));
            return parse_reply(&bytes, offset);
        }
        if Instant::now() >= deadline {
            EXPECTED_REPLY.with(|expected| expected.set(0));
            return Err(Error::Timeout);
        }
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn remaining_ms(deadline: Instant) -> Result<u32, Error> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(Error::Timeout);
    }
    Ok(remaining.as_millis().clamp(1, u32::MAX as u128) as u32)
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let field = bytes
        .get(offset..offset.checked_add(4).ok_or(Error::Protocol)?)
        .ok_or(Error::Protocol)?;
    Ok(u32::from_le_bytes(
        field.try_into().map_err(|_| Error::Protocol)?,
    ))
}

fn parse_reply(bytes: &[u8], expected_offset: u32) -> Result<(Vec<FileHit>, usize), Error> {
    let head = std::mem::size_of::<ListHeader>();
    if bytes.len() < head || bytes.len() > MAX_REPLY_BYTES {
        return Err(Error::Protocol);
    }
    let total = u32_at(bytes, 0)? as usize;
    let count = u32_at(bytes, 4)? as usize;
    if count > file_search::PAGE_SIZE as usize
        || total > i32::MAX as usize
        || count > total.saturating_sub(expected_offset as usize)
        || u32_at(bytes, 8)? != expected_offset
        || u32_at(bytes, 12)? != QUERY_FULL_PATH
        || u32_at(bytes, 16)? != SORT_NAME_ASCENDING
    {
        return Err(Error::Protocol);
    }
    let items_end = head
        .checked_add(
            count
                .checked_mul(std::mem::size_of::<ItemHeader>())
                .ok_or(Error::Protocol)?,
        )
        .ok_or(Error::Protocol)?;
    if items_end > bytes.len() {
        return Err(Error::Protocol);
    }
    let mut hits = Vec::with_capacity(count);
    for index in 0..count {
        let item = head + index * std::mem::size_of::<ItemHeader>();
        let flags = u32_at(bytes, item)?;
        let data = u32_at(bytes, item + 4)? as usize;
        if data < items_end {
            return Err(Error::Protocol);
        }
        let units = u32_at(bytes, data)? as usize;
        let byte_len = units.checked_mul(2).ok_or(Error::Protocol)?;
        let start = data.checked_add(4).ok_or(Error::Protocol)?;
        let terminator = start.checked_add(byte_len).ok_or(Error::Protocol)?;
        let field = bytes
            .get(start..terminator.checked_add(2).ok_or(Error::Protocol)?)
            .ok_or(Error::Protocol)?;
        if field.get(byte_len..byte_len + 2) != Some(&[0, 0]) {
            return Err(Error::Protocol);
        }
        let utf16: Vec<u16> = field[..byte_len]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|u| u16::from_le_bytes([u[0], u[1]]))
            .collect();
        let path = String::from_utf16(&utf16).map_err(|_| Error::Protocol)?;
        if path.is_empty() || path.contains('\0') || !Path::new(&path).is_absolute() {
            return Err(Error::Protocol);
        }
        hits.push(FileHit {
            path,
            is_folder: flags & FOLDER != 0,
        });
    }
    Ok((hits, total))
}

#[test]
#[ignore = "requires the isolated 125-file Everything fixture"]
fn live_query2_exchange_is_opt_in_for_a_machine_with_everything() {
    let hwnd = create_receiver().expect("create isolated IPC reply window");
    let query = "Velo文件验证 ext:txt";
    let (first, total) = search(hwnd, query, 0, None).expect("live Unicode Query2");
    assert_eq!(total, 125, "run only against the isolated 125-file fixture");
    assert_eq!(first.len(), 50);
    let (second, second_total) = search(hwnd, query, 50, None).expect("second Query2 page");
    let (last, _) = search(hwnd, query, 100, None).expect("last Query2 page");
    assert_eq!(second_total, total);
    assert_eq!(second.len(), 50);
    assert_eq!(last.len(), 25);
    let distinct: std::collections::BTreeSet<_> = first
        .iter()
        .chain(&second)
        .chain(&last)
        .map(|hit| &hit.path)
        .collect();
    assert_eq!(distinct.len(), total);
    assert!(distinct.iter().all(|path| Path::new(path).is_absolute()));
    let (folders, folder_count) = search(hwnd, "folder:合同归档", 0, None).expect("folder syntax");
    assert_eq!(folder_count, 1);
    assert!(folders[0].is_folder);
    let (empty, total) = search(hwnd, "VeloSyntheticNoSuchFile_8c71", 0, None).expect("zero match");
    assert_eq!(total, 0);
    assert!(empty.is_empty());
    destroy_receiver(hwnd);
}

fn create_receiver() -> Result<HWND, Error> {
    // Each worker owns its class, including during an asynchronous stop/start.
    let class_name = receiver_class();
    // SAFETY: a null module name reads this process's live executable module.
    let instance = unsafe { GetModuleHandleW(null()) };
    let class = WNDCLASSW {
        lpfnWndProc: Some(reply_proc),
        hInstance: instance,
        lpszClassName: class_name.as_ptr(),
        style: CS_HREDRAW | CS_VREDRAW,
        ..Default::default()
    };
    // SAFETY: class strings and window procedure are live for registration.
    if unsafe { RegisterClassW(&class) } == 0 {
        return Err(Error::Window);
    }
    // SAFETY: no user pointers are retained; HWND_MESSAGE keeps the window hidden.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            null_mut(),
            instance,
            null(),
        )
    };
    if hwnd.is_null() {
        unsafe {
            UnregisterClassW(class_name.as_ptr(), instance);
        }
        return Err(Error::Window);
    }
    Ok(hwnd)
}

fn destroy_receiver(hwnd: HWND) {
    let class_name = receiver_class();
    // SAFETY: only the creating worker destroys its window, after queries finish.
    unsafe {
        DestroyWindow(hwnd);
        UnregisterClassW(class_name.as_ptr(), GetModuleHandleW(null()));
    }
}

fn receiver_class() -> Vec<u16> {
    // SAFETY: this only reads the current thread's identifier.
    let thread = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
    crate::platform::windows::instance::wide(&format!("VeloEverythingIpcReceiver-{thread}"))
}

extern "system" fn reply_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message != WM_COPYDATA {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    if lparam == 0 {
        return 0;
    }
    // SAFETY: Windows supplies a live COPYDATASTRUCT for WM_COPYDATA; copy
    // the borrowed payload before returning and never retain its pointer.
    let packet = unsafe { &*(lparam as *const COPYDATASTRUCT) };
    let expected = EXPECTED_REPLY.with(Cell::get);
    if expected == 0
        || packet.dwData != expected as usize
        || packet.lpData.is_null()
        || EXPECTED_SENDER.with(Cell::get) != wparam as isize
    {
        return 0;
    }
    let size = packet.cbData as usize;
    if !(std::mem::size_of::<ListHeader>()..=MAX_REPLY_BYTES).contains(&size) {
        return 0;
    }
    // SAFETY: COPYDATASTRUCT guarantees cbData readable bytes at lpData.
    let bytes = unsafe { std::slice::from_raw_parts(packet.lpData.cast::<u8>(), size) };
    REPLY.with(|reply| *reply.borrow_mut() = Some(bytes.to_vec()));
    1
}

pub fn launch(executable: &str) -> Result<(), Error> {
    let owned = if executable.trim().is_empty() {
        default_executable().ok_or(Error::Launch)?
    } else {
        Path::new(executable).to_owned()
    };
    let path = owned.as_path();
    if !path.is_file()
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Err(Error::Launch);
    }
    std::process::Command::new(path)
        .arg("-startup")
        .spawn()
        .map(|_| ())
        .map_err(|_| Error::Launch)
}

pub fn default_executable() -> Option<std::path::PathBuf> {
    for root in [
        env::var_os("ProgramW6432"),
        env::var_os("ProgramFiles"),
        env::var_os("ProgramFiles(x86)"),
        env::var_os("LOCALAPPDATA"),
    ]
    .into_iter()
    .flatten()
    .map(std::path::PathBuf::from)
    {
        let candidate = root.join("Everything").join("Everything.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    env::split_paths(&env::var_os("PATH")?)
        .map(|root| root.join("Everything.exe"))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(path: &[u16]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for value in [
            1u32,
            1,
            0,
            QUERY_FULL_PATH,
            SORT_NAME_ASCENDING,
            FOLDER,
            28,
            path.len() as u32,
        ] {
            bytes.extend(value.to_le_bytes());
        }
        for unit in path.iter().copied().chain([0]) {
            bytes.extend(unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn query2_reply_parses_unicode_full_paths_and_folder_flags() {
        let path: Vec<u16> = r"C:\数据\项目".encode_utf16().collect();
        let (hits, total) = parse_reply(&reply(&path), 0).expect("synthetic Query2 response");
        assert_eq!(total, 1);
        assert_eq!(
            hits,
            [FileHit {
                path: r"C:\数据\项目".into(),
                is_folder: true
            }]
        );
    }

    #[test]
    fn malformed_reply_offsets_and_terminators_are_rejected() {
        assert_eq!(parse_reply(&[], 0), Err(Error::Protocol));
        let path: Vec<u16> = "报告.pdf".encode_utf16().collect();
        assert_eq!(parse_reply(&reply(&path), 50), Err(Error::Protocol));
        let mut malformed = reply(&path);
        malformed.pop();
        assert_eq!(parse_reply(&malformed, 0), Err(Error::Protocol));
    }

    #[test]
    fn malformed_query2_counts_offsets_flags_utf16_and_relative_paths_are_rejected() {
        let path: Vec<u16> = r"C:\synthetic\报告.txt".encode_utf16().collect();
        for (offset, value) in [(4, u32::MAX), (12, 0), (16, 999), (24, 0), (28, u32::MAX)] {
            let mut malformed = reply(&path);
            malformed[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert_eq!(parse_reply(&malformed, 0), Err(Error::Protocol));
        }
        assert_eq!(parse_reply(&reply(&[0xd800]), 0), Err(Error::Protocol));
        assert_eq!(
            parse_reply(
                &reply(&"relative.txt".encode_utf16().collect::<Vec<_>>()),
                0
            ),
            Err(Error::Protocol)
        );
    }

    #[test]
    fn query_mailbox_replaces_pending_work_and_cancel_releases_it() {
        let mailbox = Arc::new(Mailbox {
            pending: Mutex::new(Pending {
                latest: None,
                closed: false,
            }),
            wake: Condvar::new(),
            epoch: AtomicU64::new(0),
        });
        let worker = Worker {
            mailbox: mailbox.clone(),
            thread: None,
        };
        assert!(worker.query("obsolete".into(), 0, 1));
        assert!(worker.query("newest".into(), 50, 2));
        let request = take_request(&mailbox, Duration::ZERO).unwrap();
        assert_eq!(
            (request.query.as_str(), request.offset, request.generation),
            ("newest", 50, 2)
        );
        assert!(worker.query("cancelled".into(), 0, 3));
        worker.cancel();
        assert!(mailbox.pending.lock().unwrap().latest.is_none());
        worker.request_stop();
        assert!(!worker.query("after stop".into(), 0, 4));
        assert!(take_request(&mailbox, Duration::from_secs(1)).is_none());
    }
}
