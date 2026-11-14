//! Release selection, bounded downloads and update scripts can be exercised
//! offline. Windows transport and process creation live in platform/windows.
use super::{DataError, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::{Duration, Instant};

pub const REPOSITORY: &str = "HBLADEH/velo-launcher";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    numbers: [i64; 3],
    pre: Vec<String>,
}
impl Version {
    pub fn parse(text: &str) -> Result<Self> {
        let text = text
            .trim()
            .strip_prefix('v')
            .or_else(|| text.trim().strip_prefix('V'))
            .unwrap_or(text.trim());
        let text = text.split('+').next().unwrap_or("");
        let (core, pre) = text
            .split_once('-')
            .map_or((text, None), |(a, b)| (a, Some(b)));
        let parts: Vec<_> = core.split('.').collect();
        if parts.is_empty() || parts.len() > 3 || pre.is_some_and(str::is_empty) {
            return Err(fail("parse update version"));
        }
        let mut numbers = [0; 3];
        for (i, part) in parts.iter().enumerate() {
            numbers[i] = part
                .parse::<i64>()
                .ok()
                .filter(|v| *v >= 0)
                .ok_or_else(|| fail("parse update version"))?;
        }
        let pre: Vec<_> = pre.map_or_else(Vec::new, |p| p.split('.').map(str::to_owned).collect());
        if pre.iter().any(String::is_empty) {
            return Err(fail("parse update version"));
        }
        Ok(Self { numbers, pre })
    }
    pub fn compare(&self, other: &Self) -> Ordering {
        let core = self.numbers.cmp(&other.numbers);
        if core != Ordering::Equal {
            return core;
        }
        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, false) => return Ordering::Greater,
            (false, true) => return Ordering::Less,
            _ => {}
        }
        for (a, b) in self.pre.iter().zip(&other.pre) {
            let order = match (a.parse::<i64>(), b.parse::<i64>()) {
                (Ok(a), Ok(b)) => a.cmp(&b),
                (Ok(_), Err(_)) => Ordering::Less,
                (Err(_), Ok(_)) => Ordering::Greater,
                _ => a.cmp(b),
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        self.pre.len().cmp(&other.pre.len())
    }
}
#[derive(Default, Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub url: String,
    pub size: u64,
}
#[derive(Default, Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Release {
    pub tag_name: String,
    pub body: String,
    pub draft: bool,
    pub published_at: String,
    pub assets: Vec<Asset>,
}
#[derive(Default, Clone)]
pub struct Info {
    pub release: Option<Release>,
    pub portable: Option<Asset>,
    pub installer: Option<Asset>,
    pub checksum: Option<Asset>,
}
pub struct Response<R> {
    pub status: u32,
    pub headers: BTreeMap<String, String>,
    pub body: R,
}
pub trait Transport {
    type Body: Read;
    fn get(&mut self, url: &str, cancelled: &AtomicBool) -> Result<Response<Self::Body>>;
}
pub struct Client<T> {
    pub transport: T,
    pub api: String,
    pub web: String,
    pub portable: String,
    pub installer: String,
    retry: Option<Instant>,
}
impl<T: Transport> Client<T> {
    pub fn prototype(transport: T) -> Self {
        Self {
            transport,
            api: "https://api.github.com".into(),
            web: "https://github.com".into(),
            portable: "velo-slint.exe".into(),
            installer: "velo-slint-x64-installer.exe".into(),
            retry: None,
        }
    }
    pub fn for_build(transport: T) -> Self {
        let mut client = Self::prototype(transport);
        if cfg!(feature = "product") {
            let suffix = if cfg!(feature = "renderer-femtovg") {
                "-femtovg"
            } else {
                ""
            };
            client.portable = format!("velo-rust{suffix}.exe");
            client.installer = format!("velo-rust{suffix}-x64-installer.exe");
        }
        client
    }
    fn bytes(&mut self, url: &str, limit: usize, cancel: &AtomicBool) -> Result<Vec<u8>> {
        let response = self.transport.get(url, cancel)?;
        if response.status != 200 {
            return Err(fail("read update server response"));
        }
        bounded(response.body, limit, cancel)
    }
    pub fn check(&mut self, current: &str, cancel: &AtomicBool) -> Result<Info> {
        let mut feed = self.retry.is_some_and(|at| Instant::now() < at);
        let mut releases = Vec::new();
        if !feed {
            let response = self.transport.get(
                &format!("{}/repos/{REPOSITORY}/releases?per_page=20", self.api),
                cancel,
            )?;
            if response.status == 429 || response.status == 403 {
                let body = bounded(response.body, 65536, cancel)?;
                if response.status == 429
                    || response
                        .headers
                        .get("x-ratelimit-remaining")
                        .is_some_and(|v| v == "0")
                    || String::from_utf8_lossy(&body)
                        .to_lowercase()
                        .contains("rate limit")
                {
                    let delay = response
                        .headers
                        .get("retry-after")
                        .and_then(|v| v.parse::<u64>().ok())
                        .filter(|d| *d <= 86400)
                        .unwrap_or(300);
                    self.retry = Some(Instant::now() + Duration::from_secs(delay));
                    feed = true;
                } else {
                    return Err(fail("check update permissions"));
                }
            } else if response.status != 200 {
                return Err(fail("check update server"));
            } else {
                releases = serde_json::from_slice(&bounded(response.body, 4 << 20, cancel)?)
                    .map_err(|e| DataError::new("parse update releases", e))?;
            }
        }
        if feed {
            releases = public_releases(
                &self.bytes(
                    &format!("{}/{REPOSITORY}/releases.atom", self.web),
                    4 << 20,
                    cancel,
                )?,
                &self.web,
            )?;
        }
        let current = Version::parse(current)?;
        let mut candidates = Vec::new();
        for release in releases {
            let Ok(version) = Version::parse(&release.tag_name) else {
                continue;
            };
            if !release.draft && version.compare(&current) == Ordering::Greater {
                candidates.push((version, release));
            }
        }
        candidates.sort_by(|(a, _), (b, _)| b.compare(a));
        for (_, mut release) in candidates {
            if feed {
                let tag = &release.tag_name;
                if !safe_segment(tag) {
                    return Err(fail("read release tag"));
                }
                release.assets = public_assets(
                    &self.bytes(
                        &format!("{}/{REPOSITORY}/releases/expanded_assets/{tag}", self.web),
                        4 << 20,
                        cancel,
                    )?,
                    &self.web,
                    tag,
                );
            }
            let find = |name: &str| release.assets.iter().find(|a| a.name == name).cloned();
            let portable = find(&self.portable);
            let installer = find(&self.installer);
            // Rust restarts at 0.1.0. Older Go releases have larger versions;
            // artifact identity, including renderer, must precede selection.
            if portable.is_none() && installer.is_none() {
                continue;
            }
            return Ok(Info {
                portable,
                installer,
                checksum: find("SHA256SUMS.txt"),
                release: Some(release),
            });
        }
        Ok(Info::default())
    }
    pub fn download(
        &mut self,
        info: &Info,
        directory: &Path,
        portable: bool,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u8),
    ) -> Result<(PathBuf, String)> {
        let checksum = info
            .checksum
            .as_ref()
            .ok_or_else(|| fail("require update checksum"))?;
        let asset = (if portable {
            &info.portable
        } else {
            &info.installer
        })
        .as_ref()
        .ok_or_else(|| fail("require matching Rust update artifact"))?;
        if !safe_segment(&asset.name) {
            return Err(fail("validate update filename"));
        }
        let sums = checksums(&self.bytes(&checksum.url, 1 << 20, cancel)?);
        let expected = sums
            .get(&asset.name)
            .ok_or_else(|| fail("find update checksum"))?;
        std::fs::create_dir_all(directory)
            .map_err(|e| DataError::new("create update directory", e))?;
        let mut file = tempfile::Builder::new()
            .prefix(".update-")
            .tempfile_in(directory)
            .map_err(|e| DataError::new("create update download", e))?;
        let response = self.transport.get(&asset.url, cancel)?;
        if response.status != 200 {
            return Err(fail("download update"));
        }
        let total = response
            .headers
            .get("content-length")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(asset.size);
        let mut reader = response.body;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut done = 0u64;
        loop {
            check_cancel(cancel)?;
            let count = reader
                .read(&mut buffer)
                .map_err(|e| DataError::new("read update download", e))?;
            if count == 0 {
                break;
            }
            done += count as u64;
            if done > 256 << 20 {
                return Err(fail("limit update download"));
            }
            file.write_all(&buffer[..count])
                .map_err(|e| DataError::new("write update download", e))?;
            hash.update(&buffer[..count]);
            if let Some(percent) = done.saturating_mul(100).checked_div(total) {
                progress(percent.min(99) as u8);
            }
        }
        let actual: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if &actual != expected || (total > 0 && done != total) {
            return Err(fail("verify update SHA-256"));
        }
        check_cancel(cancel)?;
        file.as_file()
            .sync_all()
            .map_err(|e| DataError::new("flush update download", e))?;
        let temporary = file.into_temp_path();
        let destination = directory.join(&asset.name);
        crate::atomic_file::move_file(&temporary, &destination, true)
            .map_err(|e| DataError::new("commit verified update", e))?;
        progress(100);
        Ok((destination, actual))
    }
}
fn fail(operation: &'static str) -> DataError {
    DataError::new(operation, "update unavailable or invalid")
}
pub fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(AtomicOrdering::Acquire) {
        Err(fail("cancel update"))
    } else {
        Ok(())
    }
}
fn bounded(reader: impl Read, limit: usize, cancel: &AtomicBool) -> Result<Vec<u8>> {
    check_cancel(cancel)?;
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| DataError::new("read update response", e))?;
    check_cancel(cancel)?;
    if bytes.len() > limit {
        return Err(fail("limit update response"));
    }
    Ok(bytes)
}
fn safe_segment(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', '\0', '?', '#', ':', '"', '<', '>', '|'])
}
pub fn checksums(bytes: &[u8]) -> BTreeMap<String, String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let name = parts
                .next()?
                .trim_start_matches('*')
                .trim_start_matches("./");
            let name = name.rsplit(['/', '\\']).next()?;
            safe_segment(name).then(|| (name.to_owned(), hash.to_lowercase()))
        })
        .collect()
}
fn public_releases(bytes: &[u8], web: &str) -> Result<Vec<Release>> {
    #[derive(Deserialize)]
    struct Feed {
        #[serde(default)]
        entry: Vec<Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        #[serde(default)]
        link: Vec<Link>,
        #[serde(default)]
        updated: String,
        #[serde(default)]
        content: String,
    }
    #[derive(Deserialize)]
    struct Link {
        #[serde(rename = "@href")]
        href: String,
    }
    let feed: Feed = quick_xml::de::from_reader(bytes)
        .map_err(|e| DataError::new("parse public release feed", e))?;
    let prefix = format!("{web}/{REPOSITORY}/releases/tag/");
    Ok(feed
        .entry
        .into_iter()
        .filter_map(|entry| {
            let tag = entry
                .link
                .iter()
                .find_map(|link| link.href.strip_prefix(&prefix))?;
            if !safe_segment(tag) {
                return None;
            }
            Some(Release {
                tag_name: tag.into(),
                body: plain_text(&entry.content),
                published_at: entry.updated,
                ..Release::default()
            })
        })
        .collect())
}
fn plain_text(html: &str) -> String {
    let mut reader = quick_xml::Reader::from_str(html);
    reader.config_mut().check_end_names = false;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Text(value)) => {
                if let Ok(value) = value.xml_content(quick_xml::XmlVersion::Implicit1_0) {
                    match quick_xml::escape::unescape(&value) {
                        Ok(decoded) => text.push_str(&decoded),
                        Err(_) => text.push_str(&value),
                    }
                }
            }
            Ok(
                quick_xml::events::Event::Start(_)
                | quick_xml::events::Event::End(_)
                | quick_xml::events::Event::Empty(_),
            ) => text.push('\n'),
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text.trim().into()
}
fn public_assets(bytes: &[u8], web: &str, tag: &str) -> Vec<Asset> {
    let mut reader = quick_xml::Reader::from_reader(bytes);
    reader.config_mut().check_end_names = false;
    let prefix = format!("/{REPOSITORY}/releases/download/{tag}/");
    let mut assets = Vec::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e) | quick_xml::events::Event::Empty(e))
                if e.name().as_ref() == b"a" =>
            {
                for attr in e
                    .attributes()
                    .flatten()
                    .filter(|a| a.key.as_ref() == b"href")
                {
                    if let Ok(url) = attr.decoded_and_normalized_value(
                        quick_xml::XmlVersion::Implicit1_0,
                        reader.decoder(),
                    ) {
                        let relative = url.strip_prefix(web).unwrap_or(&url);
                        if let Some(name) =
                            relative.strip_prefix(&prefix).filter(|n| safe_segment(n))
                        {
                            assets.push(Asset {
                                name: name.into(),
                                url: format!("{web}{relative}"),
                                size: 0,
                            });
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    assets
}

fn literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}
pub fn script(
    target: &Path,
    source: &Path,
    portable: bool,
    sha256: &str,
    pid: u32,
    arguments: &[String],
) -> String {
    let args = arguments
        .iter()
        .map(|s| literal(s))
        .collect::<Vec<_>>()
        .join(", ");
    let mut lines = vec!["$ErrorActionPreference = 'Stop'".into(), format!("$veloTarget = {}",literal(&target.to_string_lossy())), format!("$veloSource = {}",literal(&source.to_string_lossy())), format!("$veloArguments = @({args})"), "try {".into(), format!("  $veloParent = Get-Process -Id {pid} -ErrorAction SilentlyContinue"), "  if ($veloParent -and -not $veloParent.WaitForExit(180000)) { throw 'Velo did not exit' }".into(), "  $veloHasher = [System.Security.Cryptography.SHA256]::Create()".into(), "  $veloStream = [System.IO.File]::OpenRead($veloSource)".into(), "  try { $veloHash = [BitConverter]::ToString($veloHasher.ComputeHash($veloStream)).Replace('-', '') } finally { $veloStream.Dispose(); $veloHasher.Dispose() }".into(), format!("  if ($veloHash -ne {}) {{ throw 'SHA-256 mismatch' }}",literal(sha256))];
    if portable {
        lines.extend(["  $veloReplaced = $false".into(), "  for ($veloAttempt = 0; $veloAttempt -lt 180; $veloAttempt++) {".into(), "    try { Copy-Item -LiteralPath $veloSource -Destination $veloTarget -Force -ErrorAction Stop; $veloReplaced = $true; break } catch { Start-Sleep -Seconds 1 }".into(), "  }".into(), "  if (-not $veloReplaced) { throw 'Could not replace Velo' }".into()]);
    } else {
        lines.extend([format!("  $veloInstallArgs = {}",literal(&format!("/S /D={}",target.parent().unwrap_or(Path::new("")).display()))), "  $veloInstaller = Start-Process -FilePath $veloSource -ArgumentList $veloInstallArgs -Verb RunAs -Wait -PassThru".into(), "  if ($veloInstaller.ExitCode -ne 0) { throw 'Installer failed' }".into()]);
    }
    lines.extend(["  if ($veloArguments.Count) { Start-Process -FilePath $veloTarget -ArgumentList $veloArguments } else { Start-Process -FilePath $veloTarget }".into(), "  Remove-Item -LiteralPath $veloSource -Force".into(), "  Remove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force".into(), "} catch {".into(), "  $_ | Out-String | Out-File -LiteralPath ($MyInvocation.MyCommand.Path + '.log') -Encoding UTF8 -Append".into(), "  if ($veloArguments.Count) { Start-Process -FilePath $veloTarget -ArgumentList $veloArguments -ErrorAction SilentlyContinue } else { Start-Process -FilePath $veloTarget -ErrorAction SilentlyContinue }".into(), "  exit 1".into(), "}".into()]);
    lines.join("\r\n") + "\r\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::Cursor;
    struct Fake {
        responses: VecDeque<(u32, Vec<u8>, BTreeMap<String, String>)>,
        urls: Vec<String>,
    }
    impl Transport for Fake {
        type Body = Cursor<Vec<u8>>;
        fn get(&mut self, url: &str, cancel: &AtomicBool) -> Result<Response<Self::Body>> {
            check_cancel(cancel)?;
            self.urls.push(url.into());
            let (status, body, headers) = self
                .responses
                .pop_front()
                .ok_or_else(|| fail("unexpected HTTP request"))?;
            Ok(Response {
                status,
                body: Cursor::new(body),
                headers,
            })
        }
    }
    fn client(responses: Vec<(u32, Vec<u8>, BTreeMap<String, String>)>) -> Client<Fake> {
        Client::prototype(Fake {
            responses: responses.into(),
            urls: Vec::new(),
        })
    }
    fn response(status: u32, body: &[u8]) -> (u32, Vec<u8>, BTreeMap<String, String>) {
        (status, body.to_vec(), BTreeMap::new())
    }
    #[test]
    fn release_selection_includes_prereleases_and_rate_limit_feed_stays_on_its_repo() {
        let cancel = AtomicBool::new(false);
        let data = br#"[{"tag_name":"v9.0.0","draft":true},{"tag_name":"v0.10.0-beta.11","assets":[{"name":"velo-slint.exe","browser_download_url":"https://example.test/new"}]},{"tag_name":"0.9.99"}]"#;
        let info = client(vec![response(200, data)])
            .check("0.9.11", &cancel)
            .unwrap();
        assert_eq!(info.release.unwrap().tag_name, "v0.10.0-beta.11");
        assert!(info.portable.is_some());
        let feed = br#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><link href="https://github.com/HBLADEH/velo-launcher/releases/tag/v0.10.0"/><updated>2026-11-03</updated><content type="html">&lt;p&gt;Changes&lt;/p&gt;</content></entry><entry><link href="https://evil.test/HBLADEH/velo-launcher/releases/tag/v99"/></entry></feed>"#;
        let assets = br#"<div><a href="/HBLADEH/velo-launcher/releases/download/v0.10.0/velo-slint.exe">binary</a><a href="https://evil.test/HBLADEH/velo-launcher/releases/download/v0.10.0/SHA256SUMS.txt">bad</a><a href="/other/repo/releases/download/v0.10.0/SHA256SUMS.txt">bad</a></div>"#;
        let mut c = client(vec![
            response(429, b"rate limit"),
            response(200, feed),
            response(200, assets),
            response(200, feed),
            response(200, assets),
        ]);
        for _ in 0..2 {
            let info = c.check("0.9.11", &cancel).unwrap();
            assert_eq!(info.release.unwrap().tag_name, "v0.10.0");
            assert!(info.portable.is_some());
            assert!(info.checksum.is_none());
        }
        assert_eq!(
            c.transport
                .urls
                .iter()
                .filter(|url| url.contains("api.github"))
                .count(),
            1
        );
    }
    #[test]
    fn rust_versions_ignore_legacy_go_and_keep_the_matching_renderer() {
        let cancel = AtomicBool::new(false);
        let data = br#"[
            {"tag_name":"v0.9.11","assets":[{"name":"velo-launcher.exe"},{"name":"velo-launcher-amd64-installer.exe"}]},
            {"tag_name":"0.3.0","assets":[{"name":"velo-rust-femtovg-x64-installer.exe"}]},
            {"tag_name":"0.2.0","assets":[{"name":"velo-rust-x64-installer.exe"},{"name":"SHA256SUMS.txt"}]},
            {"tag_name":"0.1.0","assets":[{"name":"velo-rust-x64-installer.exe"}]}
        ]"#;
        let mut software = client(vec![response(200, data), response(200, data)]);
        software.portable = "velo-rust.exe".into();
        software.installer = "velo-rust-x64-installer.exe".into();
        let info = software.check("0.1.0", &cancel).unwrap();
        assert_eq!(info.release.unwrap().tag_name, "0.2.0");
        assert!(info.checksum.is_some());
        assert!(software.check("0.2.0", &cancel).unwrap().release.is_none());

        let mut alternate = client(vec![response(200, data)]);
        alternate.portable = "velo-rust-femtovg.exe".into();
        alternate.installer = "velo-rust-femtovg-x64-installer.exe".into();
        assert_eq!(
            alternate
                .check("0.1.0", &cancel)
                .unwrap()
                .release
                .unwrap()
                .tag_name,
            "0.3.0"
        );
        if cfg!(feature = "product") {
            let actual = Client::for_build(alternate.transport);
            assert_eq!(
                actual.installer,
                if cfg!(feature = "renderer-femtovg") {
                    "velo-rust-femtovg-x64-installer.exe"
                } else {
                    "velo-rust-x64-installer.exe"
                }
            );
        }
    }
    #[test]
    fn rust_feed_fallback_continues_past_higher_legacy_versions() {
        let cancel = AtomicBool::new(false);
        let feed = br#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><link href="https://github.com/HBLADEH/velo-launcher/releases/tag/v0.9.11"/></entry><entry><link href="https://github.com/HBLADEH/velo-launcher/releases/tag/0.2.0"/></entry></feed>"#;
        let legacy = br#"<a href="/HBLADEH/velo-launcher/releases/download/v0.9.11/velo-launcher.exe">Go</a>"#;
        let rust = br#"<a href="/HBLADEH/velo-launcher/releases/download/0.2.0/velo-rust-x64-installer.exe">Rust</a><a href="/HBLADEH/velo-launcher/releases/download/0.2.0/SHA256SUMS.txt">SHA256</a>"#;
        let mut client = client(vec![
            response(429, b"rate limit"),
            response(200, feed),
            response(200, legacy),
            response(200, rust),
        ]);
        client.portable = "velo-rust.exe".into();
        client.installer = "velo-rust-x64-installer.exe".into();
        let info = client.check("0.1.0", &cancel).unwrap();
        assert_eq!(info.release.unwrap().tag_name, "0.2.0");
        assert!(info.installer.is_some());
        assert!(info.checksum.is_some());
        assert_eq!(client.transport.urls.len(), 4);
    }
    fn info() -> Info {
        Info {
            portable: Some(Asset {
                name: "velo-slint.exe".into(),
                url: "https://example.test/exe".into(),
                size: 3,
            }),
            checksum: Some(Asset {
                name: "SHA256SUMS.txt".into(),
                url: "https://example.test/sums".into(),
                ..Asset::default()
            }),
            ..Info::default()
        }
    }
    #[test]
    fn downloads_require_checksums_discard_partial_and_wrong_hash_and_preserve_existing_executable()
    {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("velo-slint.exe");
        std::fs::write(&path, b"old valid executable").unwrap();
        let cancel = AtomicBool::new(false);
        let hash: String = Sha256::digest(b"new")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        for payload in [b"bad".as_slice(), b"n", b"new"] {
            let sums = format!("{hash}  velo-slint.exe\n");
            let mut c = client(vec![response(200, sums.as_bytes()), response(200, payload)]);
            let result = c.download(&info(), directory.path(), true, &cancel, &mut |_| {});
            if payload == b"new" {
                assert!(result.is_ok());
                assert_eq!(std::fs::read(&path).unwrap(), b"new");
            } else {
                assert!(result.is_err());
                assert_eq!(std::fs::read(&path).unwrap(), b"old valid executable");
            }
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        }
        let mut no_checksum = info();
        no_checksum.checksum = None;
        assert!(
            client(vec![])
                .download(&no_checksum, directory.path(), true, &cancel, &mut |_| {})
                .is_err()
        );
        let mut no_asset = info();
        no_asset.portable = None;
        assert!(
            client(vec![])
                .download(&no_asset, directory.path(), true, &cancel, &mut |_| {})
                .is_err()
        );
        cancel.store(true, AtomicOrdering::Release);
        assert!(
            client(vec![])
                .download(&info(), directory.path(), true, &cancel, &mut |_| {})
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
    }
    #[test]
    fn scripts_quote_literal_paths_wait_for_exit_verify_again_and_keep_failure_recovery() {
        for portable in [true, false] {
            let script = script(
                Path::new(r"D:\中文 O'Brien\velo-slint.exe"),
                Path::new(r"D:\下载 $(invalid)\new.exe"),
                portable,
                &"a".repeat(64),
                12345,
                &["--data-dir=\"D:\\中文\"".into()],
            );
            assert!(script.contains("O''Brien"));
            assert!(script.contains("'D:\\下载 $(invalid)\\new.exe'"));
            assert!(script.contains("WaitForExit(180000)"));
            assert!(script.contains("SHA256]::Create()"));
            assert!(script.contains("catch {\r\n  $_"));
            if !portable {
                assert!(script.contains("-Verb RunAs -Wait"));
                assert!(script.contains("/S /D=D:\\中文 O''Brien"));
            }
        }
    }
}
