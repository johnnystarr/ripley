use std::process::Command;
use std::path::Path;

fn main() {
    // Rerun if web-ui source files change
    println!("cargo:rerun-if-changed=web-ui/src");
    println!("cargo:rerun-if-changed=web-ui/package.json");
    println!("cargo:rerun-if-changed=web-ui/index.html");
    
    // Check if web-ui directory exists
    if !Path::new("web-ui").exists() {
        eprintln!("Warning: web-ui directory not found, skipping UI build");
        return;
    }
    
    // Check if dist directory exists
    let dist_path = Path::new("web-ui/dist");
    if dist_path.exists() {
        println!("cargo:warning=Using existing web-ui/dist build");
        return;
    }
    
    // Check if npm is available
    let npm_check = Command::new("npm")
        .arg("--version")
        .output();
    
    if npm_check.is_err() {
        eprintln!("Warning: npm not found, skipping UI build");
        eprintln!("Run 'cd web-ui && npm install && npm run build' manually");
        return;
    }
    
    println!("cargo:warning=Building web UI...");
    
    // Install dependencies if needed
    if !Path::new("web-ui/node_modules").exists() {
        println!("cargo:warning=Installing npm dependencies...");
        let install_status = Command::new("npm")
            .args(&["install"])
            .current_dir("web-ui")
            .status()
            .expect("Failed to run npm install");
        
        if !install_status.success() {
            panic!("npm install failed");
        }
    }
    
    // Build the UI
    println!("cargo:warning=Running npm run build...");
    let build_status = Command::new("npm")
        .args(&["run", "build"])
        .current_dir("web-ui")
        .status()
        .expect("Failed to run npm build");
    
    if !build_status.success() {
        panic!("npm build failed");
    }
    
    println!("cargo:warning=Web UI built successfully");
}
