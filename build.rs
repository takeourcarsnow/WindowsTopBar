// Build script for TopBar
// Embeds Windows resources (manifest, icon)

use std::fs;
use std::path::Path;

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

    // Copy resources directory to target directory
    copy_resources_to_target();

    // Rebuild if manifest changes
    println!("cargo:rerun-if-changed=resources/");
    println!("cargo:rerun-if-changed=build.rs");
}

fn copy_resources_to_target() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let target_dir = Path::new(&out_dir)
        .parent().unwrap()  // build directory
        .parent().unwrap()  // target/debug or target/release
        .parent().unwrap(); // target

    let resources_src = Path::new("resources");
    let resources_dst = target_dir.join("resources");

    if resources_src.exists() {
        // Remove old resources directory if it exists
        if resources_dst.exists() {
            let _ = fs::remove_dir_all(&resources_dst);
        }

        // Copy the entire resources directory
        if let Err(e) = copy_dir_recursive(resources_src, &resources_dst) {
            println!("cargo:warning=Failed to copy resources: {}", e);
        }
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
