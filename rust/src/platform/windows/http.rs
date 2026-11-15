//! Synchronous WinHTTP is used only on the bounded update worker. Windows owns
//! TLS and system/PAC proxy handling; no network runtime or resident process.
use super::instance::wide;
use crate::core::{
    DataError, Result,
    update::{self, Response, Transport},
};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::io::{self, Read};
use std::net::IpAddr;
use std::ptr::{null, null_mut};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use windows_sys::Win32::Networking::WinHttp::*;

struct Handle(*mut c_void);
impl Handle {
    fn new(value: *mut c_void) -> Result<Self> {
        if value.is_null() {
            Err(DataError::new(
                "open update connection",
                io::Error::last_os_error(),
            ))
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
pub struct Http;
pub struct Body {
    request: Handle,
    _connection: Handle,
    _session: Handle,
    deadline: Instant,
}
impl Read for Body {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if Instant::now() > self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "update response deadline",
            ));
        }
        let mut read = 0;
        // SAFETY: handles remain owned by Body, buffer is writable for this call.
        if unsafe {
            WinHttpReadData(
                self.request.0,
                bytes.as_mut_ptr().cast(),
                bytes.len().min(u32::MAX as usize) as u32,
                &mut read,
            )
        } == 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(read as usize)
        }
    }
}
impl Transport for Http {
    type Body = Body;
    fn get(&mut self, url: &str, cancel: &AtomicBool) -> Result<Response<Body>> {
        update::check_cancel(cancel)?;
        if url.contains('\0') || !url.starts_with("https://") {
            return Err(DataError::new("validate update URL", "HTTPS required"));
        }
        let encoded = wide(url);
        let mut parts = URL_COMPONENTS {
            dwStructSize: std::mem::size_of::<URL_COMPONENTS>() as u32,
            dwHostNameLength: u32::MAX,
            dwUrlPathLength: u32::MAX,
            dwExtraInfoLength: u32::MAX,
            dwUserNameLength: u32::MAX,
            dwPasswordLength: u32::MAX,
            ..Default::default()
        };
        // SAFETY: URL_COMPONENTS refers only into the live encoded URL; all
        // handles use RAII and request strings stay alive until SendRequest.
        unsafe {
            if WinHttpCrackUrl(encoded.as_ptr(), 0, 0, &mut parts) == 0
                || parts.dwUserNameLength > 0
                || parts.dwPasswordLength > 0
            {
                return Err(DataError::new(
                    "parse update URL",
                    io::Error::last_os_error(),
                ));
            }
            let host = String::from_utf16_lossy(std::slice::from_raw_parts(
                parts.lpszHostName,
                parts.dwHostNameLength as usize,
            ));
            let mut path = if parts.dwUrlPathLength == 0 {
                "/".into()
            } else {
                String::from_utf16_lossy(std::slice::from_raw_parts(
                    parts.lpszUrlPath,
                    parts.dwUrlPathLength as usize,
                ))
            };
            if parts.dwExtraInfoLength > 0 {
                path.push_str(&String::from_utf16_lossy(std::slice::from_raw_parts(
                    parts.lpszExtraInfo,
                    parts.dwExtraInfoLength as usize,
                )));
            }
            let proxy = ["HTTPS_PROXY", "https_proxy"]
                .iter()
                .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()));
            let proxy = proxy.map(|p| {
                wide(
                    p.strip_prefix("http://")
                        .or_else(|| p.strip_prefix("https://"))
                        .unwrap_or(&p)
                        .trim_end_matches('/'),
                )
            });
            let excluded = ["NO_PROXY", "no_proxy"]
                .iter()
                .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
                .unwrap_or_default();
            let direct = bypass(&host, parts.nPort, &excluded);
            let session = Handle::new(WinHttpOpen(
                wide(concat!("Velo-Slint/", env!("CARGO_PKG_VERSION"))).as_ptr(),
                if direct {
                    WINHTTP_ACCESS_TYPE_NO_PROXY
                } else if proxy.is_some() {
                    WINHTTP_ACCESS_TYPE_NAMED_PROXY
                } else {
                    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
                },
                if direct {
                    null()
                } else {
                    proxy.as_ref().map_or(null(), |p| p.as_ptr())
                },
                null(),
                0,
            ))?;
            if WinHttpSetTimeouts(session.0, 10000, 10000, 30000, 30000) == 0 {
                return Err(DataError::new(
                    "set update timeout",
                    io::Error::last_os_error(),
                ));
            }
            let connection = Handle::new(WinHttpConnect(
                session.0,
                wide(&host).as_ptr(),
                parts.nPort,
                0,
            ))?;
            let request = Handle::new(WinHttpOpenRequest(
                connection.0,
                wide("GET").as_ptr(),
                wide(&path).as_ptr(),
                null(),
                null(),
                null(),
                WINHTTP_FLAG_SECURE,
            ))?;
            // Never follow an HTTPS-to-HTTP redirect.
            let redirect = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;
            if WinHttpSetOption(
                request.0,
                WINHTTP_OPTION_REDIRECT_POLICY,
                (&redirect as *const u32).cast(),
                4,
            ) == 0
            {
                return Err(DataError::new(
                    "set update redirect policy",
                    io::Error::last_os_error(),
                ));
            }
            if WinHttpSendRequest(
                request.0,
                wide("Accept: application/vnd.github+json\r\n").as_ptr(),
                u32::MAX,
                null(),
                0,
                0,
                0,
            ) == 0
                || WinHttpReceiveResponse(request.0, null_mut()) == 0
            {
                return Err(DataError::new(
                    "request update server",
                    io::Error::last_os_error(),
                ));
            }
            let mut status = 0u32;
            let mut length = 4;
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut status as *mut u32).cast(),
                &mut length,
                null_mut(),
            ) == 0
            {
                return Err(DataError::new(
                    "read update status",
                    io::Error::last_os_error(),
                ));
            }
            let mut headers = BTreeMap::new();
            for name in [
                "content-length",
                "retry-after",
                "x-ratelimit-remaining",
                "x-ratelimit-reset",
            ] {
                let mut buffer = [0u16; 256];
                let mut size = (buffer.len() * 2) as u32;
                if WinHttpQueryHeaders(
                    request.0,
                    WINHTTP_QUERY_CUSTOM,
                    wide(name).as_ptr(),
                    buffer.as_mut_ptr().cast(),
                    &mut size,
                    null_mut(),
                ) != 0
                {
                    let end = buffer.iter().position(|&u| u == 0).unwrap_or(buffer.len());
                    headers.insert(name.into(), String::from_utf16_lossy(&buffer[..end]));
                }
            }
            update::check_cancel(cancel)?;
            Ok(Response {
                status,
                headers,
                body: Body {
                    request,
                    _connection: connection,
                    _session: session,
                    deadline: Instant::now()
                        + Duration::from_secs(if url.contains("/releases/download/") {
                            20 * 60
                        } else {
                            30
                        }),
                },
            })
        }
    }
}

// Mirror the old Go NO_PROXY matching for host suffixes, optional ports, IPs
// and CIDR blocks. These exclusions also apply when Windows supplies a proxy.
fn bypass(host: &str, port: u16, excluded: &str) -> bool {
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    let address = host.parse::<IpAddr>().ok();
    if host == "localhost" || address.is_some_and(|ip| ip.is_loopback()) {
        return true;
    }
    excluded.split(',').any(|entry| {
        let entry = entry.trim().to_ascii_lowercase();
        if entry == "*" {
            return true;
        }
        if let Some((network, bits)) = entry.split_once('/') {
            let (Ok(network), Ok(bits), Some(address)) =
                (network.parse::<IpAddr>(), bits.parse::<u32>(), address)
            else {
                return false;
            };
            return match (network, address) {
                (IpAddr::V4(network), IpAddr::V4(address)) if bits <= 32 => {
                    bits == 0
                        || u32::from(network) >> (32 - bits) == u32::from(address) >> (32 - bits)
                }
                (IpAddr::V6(network), IpAddr::V6(address)) if bits <= 128 => {
                    bits == 0
                        || u128::from(network) >> (128 - bits)
                            == u128::from(address) >> (128 - bits)
                }
                _ => false,
            };
        }
        let (name, required_port) = if let Some((name, port)) = entry
            .rsplit_once(':')
            .filter(|(name, _)| !name.contains(':') || name.ends_with(']'))
        {
            let Ok(port) = port.parse::<u16>() else {
                return false;
            };
            (name, Some(port))
        } else {
            (entry.as_str(), None)
        };
        if required_port.is_some_and(|p| p != port) {
            return false;
        }
        let name = name.trim_matches(['[', ']']);
        if let Ok(ip) = name.parse::<IpAddr>() {
            return Some(ip) == address;
        }
        let subdomains_only = name.starts_with('.') || name.starts_with("*.");
        let name = name.trim_start_matches('*').trim_start_matches('.');
        !name.is_empty()
            && ((!subdomains_only && name == host)
                || host
                    .strip_suffix(name)
                    .is_some_and(|prefix| prefix.ends_with('.')))
    })
}

#[cfg(test)]
mod tests {
    use super::bypass;
    #[test]
    fn no_proxy_keeps_go_domain_port_ip_cidr_and_loopback_rules() {
        for (host, port, list, expected) in [
            ("api.github.com", 443, "github.com", true),
            ("github.com", 443, ".github.com", false),
            ("api.github.com", 443, "*.github.com", true),
            ("evilgithub.com", 443, "github.com", false),
            ("api.github.com", 443, "github.com:80", false),
            ("api.github.com", 443, "github.com:443", true),
            ("10.8.0.7", 443, "10.0.0.0/8", true),
            ("11.0.0.7", 443, "10.0.0.0/8", false),
            ("[2001:db8::1]", 443, "2001:db8::/32", true),
            ("[2001:db8::1]", 443, "[2001:db8::1]:443", true),
            ("localhost", 443, "", true),
            ("127.0.0.1", 443, "", true),
            ("::1", 443, "", true),
            ("api.github.com", 443, "*", true),
            ("api.github.com", 443, "", false),
        ] {
            assert_eq!(bypass(host, port, list), expected, "{host} {list}");
        }
    }
}
