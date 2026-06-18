use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    slint_build::compile("ui/appwindow.slint").unwrap();

    // Copy assets/map.mbtiles to the target profile directory
    if let Ok(out_dir) = env::var("OUT_DIR") {
        let out_path = PathBuf::from(out_dir);
        let mut profile_dir = out_path.clone();
        for _ in 0..3 {
            if let Some(parent) = profile_dir.parent() {
                profile_dir = parent.to_path_buf();
            }
        }

        let src_map = Path::new("assets/map.mbtiles");
        if src_map.exists() {
            let dest_dir = profile_dir.join("assets");
            let _ = fs::create_dir_all(&dest_dir);
            let dest_map = dest_dir.join("map.mbtiles");
            let _ = fs::copy(src_map, dest_map);
            println!("cargo:rerun-if-changed=assets/map.mbtiles");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
