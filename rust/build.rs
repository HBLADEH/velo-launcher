fn main() {
    let target = std::env::var("TARGET").expect("Cargo must provide TARGET");
    assert_eq!(
        target, "x86_64-pc-windows-msvc",
        "Only Windows x64 is supported"
    );
    println!("cargo:rustc-env=VELO_BUILD_TARGET={target}");
    println!("cargo:rerun-if-changed=ui");
    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new()
            .with_style("fluent".into())
            .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles),
    )
    .expect("Slint UI compilation failed");
}
