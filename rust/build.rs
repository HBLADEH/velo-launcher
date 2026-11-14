fn main() {
    let target = std::env::var("TARGET").expect("Cargo must provide TARGET");
    assert_eq!(
        target, "x86_64-pc-windows-msvc",
        "Only Windows x64 is supported"
    );
    println!("cargo:rustc-env=VELO_BUILD_TARGET={target}");
    windows_resources();
    println!("cargo:rerun-if-changed=ui");
    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new()
            .with_style("fluent".into())
            .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles),
    )
    .expect("Slint UI compilation failed");
}

fn windows_resources() {
    use std::{env, fs, path::PathBuf, process::Command};

    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo project path"));
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output path"));
    let version = env::var("CARGO_PKG_VERSION").expect("Cargo package version");
    let parts: Vec<u16> = version
        .split('.')
        .map(|part| {
            part.parse()
                .expect("Windows version requires major.minor.patch numbers")
        })
        .collect();
    assert_eq!(parts.len(), 3, "Windows version requires three components");
    let renderer = if env::var_os("CARGO_FEATURE_RENDERER_SOFTWARE").is_some() {
        "Software"
    } else {
        "FemtoVG"
    };
    let manifest = output.join("app.manifest");
    fs::write(
        &manifest,
        fs::read_to_string(root.join("windows/app.manifest"))
            .expect("read Windows manifest")
            .replace("@VERSION@", &format!("{version}.0")),
    )
    .expect("write versioned Windows manifest");
    let quote = |path: PathBuf| path.to_string_lossy().replace('\\', "\\\\");
    let source = format!(
        "#pragma code_page(65001)\n#include <windows.h>\n1 ICON \"{}\"\n1 RT_MANIFEST \"{}\"\n1 VERSIONINFO\nFILEVERSION {},{},{},0\nPRODUCTVERSION {},{},{},0\nFILEOS VOS_NT_WINDOWS32\nFILETYPE VFT_APP\nBEGIN\n BLOCK \"StringFileInfo\" BEGIN\n BLOCK \"040904b0\" BEGIN\n VALUE \"CompanyName\", \"HBLADEH\\0\"\n VALUE \"FileDescription\", \"Velo Rust + Slint ({renderer})\\0\"\n VALUE \"FileVersion\", \"{version}\\0\"\n VALUE \"ProductName\", \"Velo Rust + Slint\\0\"\n VALUE \"ProductVersion\", \"{version}\\0\"\n VALUE \"InternalName\", \"velo-slint\\0\"\n VALUE \"OriginalFilename\", \"velo-slint.exe\\0\"\n VALUE \"LegalCopyright\", \"Copyright (c) HBLADEH\\0\"\n END\n END\n BLOCK \"VarFileInfo\" BEGIN VALUE \"Translation\", 0x0409, 1200 END\nEND\n",
        quote(root.join("ui/assets/velo.ico")),
        quote(manifest),
        parts[0],
        parts[1],
        parts[2],
        parts[0],
        parts[1],
        parts[2],
    );
    let script = output.join("velo.rc");
    let resource = output.join("velo.res");
    fs::write(&script, source).expect("write Windows resources");
    let status = Command::new("rc.exe")
        .arg("/nologo")
        .arg(format!("/fo{}", resource.display()))
        .arg(&script)
        .status()
        .expect(
            "run pinned Windows SDK resource compiler; initialize scripts/lib/rust-environment.ps1",
        );
    assert!(status.success(), "Windows resource compilation failed");
    println!("cargo:rustc-link-arg-bin=velo-slint={}", resource.display());
    println!("cargo:rerun-if-changed=windows/app.manifest");
    println!("cargo:rerun-if-env-changed=CARGO_PKG_VERSION");
}
