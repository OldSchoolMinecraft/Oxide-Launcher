fn main() {
    println!("cargo:rerun-if-changed=assets/icons/minecraft.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icons/minecraft.ico")
            .compile()
            .expect("compile the Windows executable icon resource");
    }
}
