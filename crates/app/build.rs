// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Build script: embeds the app icon into the Windows executable via winresource.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to embed icon: {e}");
        }
    }
}
