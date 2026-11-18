use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Storage::FileSystem::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

/// Both paths must be in the same directory, so replacement stays on one volume.
pub(crate) fn move_file(
    source: &Path,
    destination: &Path,
    replace_existing: bool,
) -> io::Result<()> {
    fn wide(path: &Path) -> io::Result<Vec<u16>> {
        let mut units: Vec<_> = path.as_os_str().encode_wide().collect();
        if units.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NUL in file name",
            ));
        }
        units.push(0);
        Ok(units)
    }
    let source = wide(source)?;
    let destination = wide(destination)?;
    // SAFETY: Both buffers are NUL-terminated and live through this synchronous
    // call. MoveFileEx owns no handles; the caller already flushed/closed the file.
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace_existing {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    if unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
