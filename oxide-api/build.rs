//! Build script for oxide-api
//!
//! This script ensures the UI is built before the Rust binary is compiled.
//! It automatically runs `npm run build` in the ui directory if the UI files
//! are missing or outdated.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../ui/src");
    println!("cargo:rerun-if-changed=../ui/package.json");
    println!("cargo:rerun-if-changed=../ui/package-lock.json");
    
    let ui_dist_path = Path::new("../ui/dist");
    
    // Check if UI needs to be built
    let needs_build = !ui_dist_path.exists() || 
        !ui_dist_path.join("index.html").exists() ||
        is_ui_outdated();
    
    if needs_build {
        println!("cargo:warning=Building UI...");
        
        // Change to UI directory and run npm install if needed
        if !Path::new("../ui/node_modules").exists() {
            println!("cargo:warning=Installing UI dependencies...");
            let output = Command::new("npm")
                .args(["install"])
                .current_dir("../ui")
                .output()
                .expect("Failed to run npm install");
            
            if !output.status.success() {
                panic!("npm install failed: {}", String::from_utf8_lossy(&output.stderr));
            }
        }
        
        // Build the UI
        let output = Command::new("npm")
            .args(["run", "build"])
            .current_dir("../ui")
            .output()
            .expect("Failed to run npm run build");
        
        if !output.status.success() {
            panic!("UI build failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        
        println!("cargo:warning=UI build completed successfully");
    }
}

fn is_ui_outdated() -> bool {
    // Simple check: if any source file is newer than the dist directory
    let ui_dist_path = Path::new("../ui/dist");
    let ui_src_path = Path::new("../ui/src");
    
    if let (Ok(dist_meta), Ok(src_meta)) = (ui_dist_path.metadata(), ui_src_path.metadata()) {
        if let (Ok(dist_time), Ok(src_time)) = (dist_meta.modified(), src_meta.modified()) {
            return src_time > dist_time;
        }
    }
    
    // If we can't determine, assume it needs rebuilding
    true
}