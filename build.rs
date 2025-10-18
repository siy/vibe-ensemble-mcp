use std::process::Command;

fn main() {
    // Tell cargo to rerun if dashboard source files change
    println!("cargo:rerun-if-changed=dashboard/src");
    println!("cargo:rerun-if-changed=dashboard/package.json");
    println!("cargo:rerun-if-changed=dashboard/vite.config.ts");
    println!("cargo:rerun-if-changed=dashboard/tsconfig.json");
    println!("cargo:rerun-if-changed=dashboard/index.html");

    // Check if npm is available
    let npm_check = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "npm", "--version"])
            .output()
    } else {
        Command::new("npm").arg("--version").output()
    };

    if npm_check.is_err() {
        eprintln!("Warning: npm not found. Skipping dashboard build.");
        eprintln!("The dashboard will not be available unless you manually run:");
        eprintln!("  cd dashboard && npm install && npm run build");
        return;
    }

    println!("Building dashboard...");

    // Install dependencies
    let install_status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "npm", "install", "--prefix", "dashboard"])
            .status()
    } else {
        Command::new("npm")
            .args(["install", "--prefix", "dashboard"])
            .status()
    };

    if let Err(e) = install_status {
        eprintln!("Warning: Failed to install dashboard dependencies: {}", e);
        return;
    }

    // Build dashboard
    let build_status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "npm", "run", "build", "--prefix", "dashboard"])
            .status()
    } else {
        Command::new("npm")
            .args(["run", "build", "--prefix", "dashboard"])
            .status()
    };

    match build_status {
        Ok(status) if status.success() => {
            println!("Dashboard build completed successfully");
        }
        Ok(status) => {
            eprintln!("Warning: Dashboard build failed with status: {}", status);
        }
        Err(e) => {
            eprintln!("Warning: Failed to build dashboard: {}", e);
        }
    }
}
