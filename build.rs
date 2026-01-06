// Build script for TopBar
// Embeds Windows resources (manifest, icon)

fn main() {
    // Only run on Windows
    #[cfg(target_os = "windows")]
    {
        // Embed Windows resources if the .rc file exists
        let rc_path = std::path::Path::new("resources/topbar.rc");
        if rc_path.exists() {
            embed_resource::compile("resources/topbar.rc", embed_resource::NONE);
        }

        // Link with required Windows libraries
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rustc-link-lib=dwmapi");
        println!("cargo:rustc-link-lib=shell32");
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=comctl32");
    }

    // Rebuild if manifest changes
    println!("cargo:rerun-if-changed=resources/");
    println!("cargo:rerun-if-changed=build.rs");
}
