use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use velo_slint::core::data::IndexCache;
use velo_slint::core::indexer::{Generation, Resolver, Root};
use velo_slint::core::model::{AppItem, cached_icon_path, identity};
use velo_slint::core::{icons, indexer, launch};

struct Links {
    calls: usize,
    target: PathBuf,
    cancel: Option<Generation>,
}
impl Resolver for Links {
    fn resolve(&mut self, _path: &Path) -> velo_slint::core::Result<AppItem> {
        self.calls += 1;
        if let Some(cancel) = &self.cancel {
            cancel.advance();
        }
        Ok(AppItem {
            exec_path: self.target.to_string_lossy().into_owned(),
            arguments: "--profile 中文".into(),
            keywords: Some(vec![]),
            ..AppItem::default()
        })
    }
}
fn touch(path: &Path) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"synthetic non-executable").unwrap();
}
fn root(path: &Path, source: &str, max_depth: usize) -> Root {
    Root {
        path: path.into(),
        source: source.into(),
        max_depth,
    }
}
fn pixels() -> icons::Pixels {
    icons::Pixels {
        width: 64,
        height: 64,
        rgba: vec![255; 64 * 64 * 4],
    }
}

#[test]
fn unchanged_links_are_reused_changed_and_deleted_files_update_without_losing_identity() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    let link = path.join("中文 空格.lnk");
    let executable = path.join("app.exe");
    touch(&link);
    touch(&executable);
    touch(&path.join("ignore.txt"));
    let generation = Generation::default();
    let ticket = generation.ticket();
    let mut resolver = Links {
        calls: 0,
        target: executable,
        cancel: None,
    };
    let roots = [
        root(path, "Start Menu", 0),
        root(&path.join("."), "Custom", 0),
    ];
    let (first, _) = indexer::scan(&roots, &IndexCache::default(), &mut resolver, &ticket).unwrap();
    assert_eq!(resolver.calls, 1);
    assert_eq!(first.apps.as_ref().unwrap().len(), 2); // profile is a separate ID
    let (second, stats) = indexer::scan(&roots, &first, &mut resolver, &ticket).unwrap();
    assert_eq!(resolver.calls, 1);
    assert_eq!(stats.reused, 2);
    assert_eq!(first, second);
    fs::write(
        &link,
        b"changed shortcut metadata, same application identity",
    )
    .unwrap();
    let (third, _) = indexer::scan(&roots, &second, &mut resolver, &ticket).unwrap();
    assert_eq!(resolver.calls, 2);
    assert_eq!(third.apps.as_ref().unwrap(), first.apps.as_ref().unwrap());
    fs::remove_file(&link).unwrap();
    let (fourth, _) = indexer::scan(&roots, &third, &mut resolver, &ticket).unwrap();
    assert_eq!(fourth.apps.as_ref().unwrap().len(), 1);
}

#[test]
fn internet_shortcuts_on_desktop_and_start_menu_are_searchable_and_pinnable() {
    use velo_slint::core::{catalog::Catalog, config::Config, data::History, library};

    let directory = tempfile::tempdir().unwrap();
    let desktop = directory.path().join("Desktop");
    let menu = directory.path().join("Programs");
    let nested = menu.join("Games");
    fs::create_dir(&desktop).unwrap();
    fs::create_dir_all(&nested).unwrap();
    let game = desktop.join("星际游戏.URL");
    let web = nested.join("星际游戏.url");
    fs::write(&game, "[InternetShortcut]\r\nURL=steam://rungameid/42\r\n").unwrap();
    fs::write(
        &web,
        "[InternetShortcut]\r\nURL=https://example.invalid/\r\n",
    )
    .unwrap();
    let roots = [root(&desktop, "Desktop", 0), root(&menu, "Start Menu", 0)];
    let mut resolver = Links {
        calls: 0,
        target: PathBuf::new(),
        cancel: None,
    };
    let ticket = Generation::default().ticket();
    let (cache, _) = indexer::scan(&roots, &IndexCache::default(), &mut resolver, &ticket).unwrap();
    let (warm, stats) = indexer::scan(&roots, &cache, &mut resolver, &ticket).unwrap();
    assert_eq!(warm, cache);
    assert_eq!(stats.reused, 2);

    let imported = library::import(
        &Default::default(),
        std::slice::from_ref(&game),
        &mut resolver,
    )
    .unwrap();
    assert_eq!((imported.added, imported.rejected), (1, 0));
    assert_eq!(resolver.calls, 0); // Internet shortcuts do not use the .lnk COM resolver.
    let apps = cache.apps.unwrap();
    let catalog = Catalog::new(&apps, &imported.library, &[]);
    let results = catalog
        .search(
            "星际游戏",
            &Config::default(),
            &History::default(),
            chrono::Utc::now(),
        )
        .unwrap();
    assert_eq!(results.len(), 2);
    let id = identity(&game.to_string_lossy(), "");
    let item = catalog.item(&id).unwrap();
    assert_eq!(item.source, "Manual");
    assert_eq!(item.name, "星际游戏");
    assert!(
        catalog
            .with_pin(&imported.library, &id, true)
            .unwrap()
            .has_pin(&id)
    );
    assert_eq!(launch::capabilities(item), (true, false));
    let open = || {
        launch::command(item, launch::Action::Launch, directory.path(), |p| {
            fs::metadata(p).map(|m| m.is_dir())
        })
    };
    let command = open().unwrap();
    assert_eq!(Path::new(&command.path), game);
    assert!(command.arguments.is_empty() && command.directory.is_empty());
    fs::remove_file(&game).unwrap();
    assert_eq!(open(), Err(launch::CommandError::MissingTarget));
    assert_eq!(
        indexer::scan(&roots, &warm, &mut resolver, &ticket)
            .unwrap()
            .0
            .apps
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn cached_shortcut_gets_shell_name_once_and_keeps_its_launch_metadata() {
    struct LocalizedName(usize);
    impl Resolver for LocalizedName {
        fn shortcut_name(&mut self, _: &Path) -> Option<String> {
            self.0 += 1;
            Some("远程桌面连接".into())
        }
        fn resolve(&mut self, _: &Path) -> velo_slint::core::Result<AppItem> {
            panic!("unchanged shortcut should keep its cached launch metadata")
        }
    }

    let directory = tempfile::tempdir().unwrap();
    let link = directory.path().join("Remote Desktop Connection.lnk");
    touch(&link);
    let roots = [root(directory.path(), "Start Menu", 0)];
    let ticket = Generation::default().ticket();
    let mut resolver = Links {
        calls: 0,
        target: directory.path().join("mstsc.exe"),
        cancel: None,
    };
    let (mut legacy, _) =
        indexer::scan(&roots, &IndexCache::default(), &mut resolver, &ticket).unwrap();
    let key = indexer::path_key(&link);
    let file = legacy.files.as_mut().unwrap().get_mut(&key).unwrap();
    file.shortcut_name_checked = false;
    let expected = file.item.clone();
    let mut resolver = LocalizedName(0);
    let (updated, stats) = indexer::scan(&roots, &legacy, &mut resolver, &ticket).unwrap();
    let item = &updated.apps.as_ref().unwrap()[0];
    assert_eq!(item.name, "远程桌面连接");
    assert_eq!(
        item.keywords.as_deref().unwrap(),
        ["Remote Desktop Connection", "RemoteDesktopConnection"]
    );
    assert_eq!(item.id, expected.id);
    assert_eq!(item.path, expected.path);
    assert_eq!(item.exec_path, expected.exec_path);
    assert_eq!(item.arguments, expected.arguments);
    assert_eq!(stats.resolved, 0);
    let (unchanged, _) = indexer::scan(&roots, &updated, &mut resolver, &ticket).unwrap();
    assert_eq!(unchanged, updated);
    assert_eq!(resolver.0, 1);
}

#[test]
fn desktop_targets_are_rechecked_even_when_the_shortcut_did_not_change() {
    let directory = tempfile::tempdir().unwrap();
    let desktop = directory.path().join("Desktop");
    let target = directory.path().join("application.exe");
    touch(&desktop.join("应用.lnk"));
    touch(&target);
    let mut resolver = Links {
        calls: 0,
        target: target.clone(),
        cancel: None,
    };
    let ticket = Generation::default().ticket();
    let roots = [root(&desktop, "Desktop", 0)];
    let (first, _) = indexer::scan(&roots, &IndexCache::default(), &mut resolver, &ticket).unwrap();
    assert_eq!(first.apps.as_ref().unwrap().len(), 1);
    fs::remove_file(&target).unwrap();
    let (next, _) = indexer::scan(&roots, &first, &mut resolver, &ticket).unwrap();
    assert!(next.apps.as_ref().unwrap().is_empty());
    touch(&target);
    let (restored, _) = indexer::scan(&roots, &next, &mut resolver, &ticket).unwrap();
    assert_eq!(first.apps, restored.apps);
}

#[test]
fn depth_windowsapps_and_cancellation_follow_the_baseline() {
    let directory = tempfile::tempdir().unwrap();
    for relative in [
        "App/app.exe",
        "Vendor/Product/main.exe",
        "Vendor/Product/bin/helper.exe",
        "WindowsApps/package.exe",
    ] {
        touch(&directory.path().join(relative));
    }
    let mut resolver = Links {
        calls: 0,
        target: PathBuf::new(),
        cancel: None,
    };
    let generation = Generation::default();
    let ticket = generation.ticket();
    let (shallow, _) = indexer::scan(
        &[root(directory.path(), "Program Files", 2)],
        &IndexCache::default(),
        &mut resolver,
        &ticket,
    )
    .unwrap();
    assert_eq!(shallow.apps.unwrap().len(), 2);
    let (deep, _) = indexer::scan(
        &[root(directory.path(), "Program Files", 0)],
        &IndexCache::default(),
        &mut resolver,
        &ticket,
    )
    .unwrap();
    assert_eq!(deep.apps.unwrap().len(), 3);
    touch(&directory.path().join("cancel.lnk"));
    resolver.cancel = Some(generation.clone());
    assert!(
        indexer::scan(
            &[root(directory.path(), "Custom", 0)],
            &IndexCache::default(),
            &mut resolver,
            &ticket
        )
        .is_err()
    );
    assert!(!ticket.current());
}

#[test]
fn inaccessible_subtree_retains_old_entries_but_missing_subtree_removes_them() {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_LIST_DIRECTORY, OPEN_EXISTING,
    };
    let directory = tempfile::tempdir().unwrap();
    let child = directory.path().join("container/protected");
    touch(&child.join("application.exe"));
    let roots = [root(directory.path(), "Custom", 0)];
    let ticket = Generation::default().ticket();
    let mut resolver = Links {
        calls: 0,
        target: PathBuf::new(),
        cancel: None,
    };
    let (old, _) = indexer::scan(&roots, &IndexCache::default(), &mut resolver, &ticket).unwrap();
    let path: Vec<u16> = child
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain([0])
        .collect();
    // SAFETY: test-owned directory, valid string, exclusively held for one scan.
    let lock = unsafe {
        CreateFileW(
            path.as_ptr(),
            FILE_LIST_DIRECTORY,
            0,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(lock, INVALID_HANDLE_VALUE);
    let result = indexer::scan(&roots, &old, &mut resolver, &ticket);
    let shallow = indexer::scan(
        &[root(directory.path(), "Custom", 1)],
        &old,
        &mut resolver,
        &ticket,
    );
    unsafe {
        CloseHandle(lock);
    }
    let (retained, stats) = result.unwrap();
    assert_eq!(old.apps, retained.apps);
    assert_eq!(stats.warnings, 1);
    let (shallow, stats) = shallow.unwrap();
    assert!(shallow.files.as_ref().unwrap().is_empty());
    assert_eq!(stats.warnings, 0);
    fs::remove_file(child.join("application.exe")).unwrap();
    fs::remove_dir(&child).unwrap();
    let (missing, _) = indexer::scan(&roots, &old, &mut resolver, &ticket).unwrap();
    assert!(missing.apps.as_ref().unwrap().is_empty());
}

#[test]
fn junctions_are_not_followed_during_discovery() {
    let directory = tempfile::tempdir().unwrap();
    let scanned = directory.path().join("scanned");
    let external = directory.path().join("external");
    touch(&scanned.join("application.exe"));
    touch(&external.join("other.exe"));
    let junction = scanned.join("junction");
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:VELO_RS10_JUNCTION -Target $env:VELO_RS10_TARGET | Out-Null"])
        .env("VELO_RS10_JUNCTION", &junction).env("VELO_RS10_TARGET", &external).status().unwrap();
    assert!(status.success());
    assert!(
        fs::symlink_metadata(&junction)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let mut resolver = Links {
        calls: 0,
        target: PathBuf::new(),
        cancel: None,
    };
    let result = indexer::scan(
        &[root(&scanned, "Custom", 0)],
        &IndexCache::default(),
        &mut resolver,
        &Generation::default().ticket(),
    );
    fs::remove_dir(&junction).unwrap();
    assert_eq!(result.unwrap().0.files.unwrap().len(), 1);
    assert!(external.join("other.exe").exists());
}

#[test]
fn disk_icons_reuse_invalidate_repair_and_prune_only_generated_unreferenced_names() {
    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("application.exe");
    touch(&executable);
    let mut item = AppItem {
        path: executable.to_string_lossy().into_owned(),
        ..AppItem::default()
    };
    item.icon_url = icons::signature(&item);
    let ticket = Generation::default().ticket();
    icons::load(directory.path(), &item, &ticket, |_| Ok(pixels())).unwrap();
    let old_url = item.icon_url.clone();
    let old = cached_icon_path(directory.path(), &old_url).unwrap();
    icons::load(directory.path(), &item, &ticket, |_| {
        panic!("PNG should be reused")
    })
    .unwrap();
    fs::write(&executable, b"changed binary signature").unwrap();
    item.icon_url = icons::signature(&item);
    assert_ne!(old_url, item.icon_url);
    icons::load(directory.path(), &item, &ticket, |_| Ok(pixels())).unwrap();
    assert!(old.exists(), "unaccepted scan must not prune old icons");
    let current = cached_icon_path(directory.path(), &item.icon_url).unwrap();
    fs::write(&current, b"corrupt PNG").unwrap();
    icons::load(directory.path(), &item, &ticket, |_| Ok(pixels())).unwrap();
    let unknown = current.parent().unwrap().join("user-picture.png");
    fs::write(&unknown, b"preserve unknown files").unwrap();
    icons::prune(
        directory.path(),
        [item.icon_url.clone(), old_url].into_iter(),
    )
    .unwrap();
    assert!(old.exists());
    icons::prune(directory.path(), [item.icon_url].into_iter()).unwrap();
    assert!(!old.exists());
    assert!(current.exists());
    assert!(unknown.exists());
}

#[test]
fn cancelled_extraction_does_not_write_or_prune_and_lru_honors_live_references() {
    let directory = tempfile::tempdir().unwrap();
    let item = AppItem {
        path: "synthetic.exe".into(),
        icon_url: format!("/icons/{}.png", identity("synthetic.exe", "")),
        ..AppItem::default()
    };
    let generation = Generation::default();
    let ticket = generation.ticket();
    assert!(
        icons::load(directory.path(), &item, &ticket, |_| {
            generation.advance();
            Ok(pixels())
        })
        .is_err()
    );
    assert!(!directory.path().join("cache").exists());
    let mut cache = icons::Lru::new(8);
    assert!(cache.insert("first".into(), 1, 4));
    assert!(cache.insert("second".into(), 2, 4));
    cache.protect(BTreeSet::from(["first".into()]));
    assert!(cache.insert("third".into(), 3, 4));
    assert_eq!(cache.get("first"), Some(1));
    assert!(cache.get("second").is_none());
    cache.protect(BTreeSet::from(["first".into(), "third".into()]));
    assert!(!cache.insert("fourth".into(), 4, 4));
    assert_eq!(cache.bytes(), 8);
    cache.protect(BTreeSet::new());
    assert!(cache.insert("large".into(), 9, 8));
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.bytes(), 8);
}

#[test]
fn launch_shortcuts_keep_shell_semantics_admin_retains_profile_and_invalid_targets_fail() {
    let item = AppItem {
        path: r"C:\Desktop\中文.lnk".into(),
        exec_path: r"D:\Apps\应用.exe".into(),
        arguments: "--profile 中文".into(),
        working_directory: r"D:\Workspace".into(),
        ..AppItem::default()
    };
    let windows = Path::new(r"C:\Windows");
    let command = launch::command(&item, launch::Action::Launch, windows, |_| Ok(false)).unwrap();
    assert_eq!(command.path, item.path);
    assert!(command.arguments.is_empty());
    assert!(command.directory.is_empty());
    let admin = launch::command(&item, launch::Action::RunAsAdmin, windows, |_| Ok(false)).unwrap();
    assert_eq!(admin.verb, "runas");
    assert_eq!(admin.path, item.exec_path);
    assert_eq!(admin.arguments, item.arguments);
    assert_eq!(admin.directory, item.working_directory);
    assert_eq!(
        launch::command(&item, launch::Action::Launch, windows, |_| Err(
            std::io::ErrorKind::NotFound.into()
        )),
        Err(launch::CommandError::MissingTarget)
    );
    assert!(launch::command(&item, launch::Action::RunAsAdmin, windows, |_| Ok(true)).is_err());
    for path in [
        "ms-settings:appsfeatures",
        "velo-action:shutdown",
        "unresolved.lnk",
    ] {
        let item = AppItem {
            path: path.into(),
            ..AppItem::default()
        };
        assert!(
            launch::command(&item, launch::Action::RunAsAdmin, windows, |_| Ok(false)).is_err()
        );
    }
}

#[test]
fn launch_preparation_only_flags_missing_application_targets() {
    use launch::{Action, CommandError};
    use std::io::ErrorKind;

    let item = AppItem {
        path: r"C:\Apps\Missing.exe".into(),
        exec_path: r"C:\Apps\Missing.exe".into(),
        ..AppItem::default()
    };
    for (action, error, expected) in [
        (
            Action::Launch,
            ErrorKind::NotFound,
            CommandError::MissingTarget,
        ),
        (
            Action::RunAsAdmin,
            ErrorKind::NotFound,
            CommandError::MissingTarget,
        ),
        (
            Action::Launch,
            ErrorKind::PermissionDenied,
            CommandError::Unavailable,
        ),
        (
            Action::OpenDirectory,
            ErrorKind::NotFound,
            CommandError::Unavailable,
        ),
    ] {
        assert_eq!(
            launch::command(&item, action, Path::new(r"C:\Windows"), |_| Err(
                error.into()
            )),
            Err(expected),
        );
    }
}

#[test]
fn windows_launch_reports_missing_targets_behind_shortcuts() {
    use velo_slint::platform::windows::{launch as native, shell};
    use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{Interface, PCWSTR};

    let directory = tempfile::tempdir().unwrap();
    let executable = directory.path().join("Missing.exe");
    let shortcut = directory.path().join("Pinned.lnk");
    fs::write(&executable, b"synthetic target").unwrap();
    let apartment = shell::Apartment::new().unwrap();
    let target_path = shell::wide(executable.to_str().unwrap()).unwrap();
    let shortcut_path = shell::wide(shortcut.to_str().unwrap()).unwrap();
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
        link.SetPath(PCWSTR(target_path.as_ptr())).unwrap();
        let file: IPersistFile = link.cast().unwrap();
        file.Save(PCWSTR(shortcut_path.as_ptr()), true).unwrap();
    }
    fs::remove_file(executable).unwrap();
    let item = AppItem {
        path: shortcut.to_string_lossy().into_owned(),
        ..AppItem::default()
    };
    assert_eq!(
        native::execute(&apartment, 0, &item, launch::Action::Launch),
        Err(native::Failure::MissingTarget),
    );
}

#[test]
fn windows_launch_reports_missing_apps_folder_entries() {
    use velo_slint::platform::windows::{launch as native, shell};

    let apartment = shell::Apartment::new().unwrap();
    let item = AppItem {
        path: format!(
            "{}Velo.Test.Missing_abcdefghij012!MissingApp",
            launch::APPS_FOLDER_PREFIX
        ),
        ..AppItem::default()
    };
    assert_eq!(
        native::execute(&apartment, 0, &item, launch::Action::Launch),
        Err(native::Failure::MissingTarget),
    );
}
