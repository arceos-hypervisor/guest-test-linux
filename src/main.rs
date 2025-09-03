use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(author, version, about = "Manage Linux 6.12 source code and builds")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Clone and build Linux for all architectures
    Build,
    /// Clean the build directory
    Clean,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    match args.command {
        Commands::Build => {
            // Clone Linux source if not exists
            if Path::new("linux").exists() {
                println!("Linux source already exists, skipping clone.");
            } else {
                println!("Cloning Linux 6.12 source code...");
                let status = Command::new("git")
                    .args([
                        "clone",
                        "--depth=1",
                        "-b",
                        "v6.12",
                        "https://github.com/torvalds/linux.git",
                        "linux",
                    ])
                    .status()
                    .expect("Failed to execute git clone");
                if status.success() {
                    println!("Clone completed successfully.");
                } else {
                    eprintln!("Clone failed.");
                    return;
                }
            }

            // Traverse config directory and build for each arch
            let config_dir = Path::new("config");
            if !config_dir.exists() {
                eprintln!("Config directory does not exist.");
                return;
            }

            for entry in fs::read_dir(config_dir).expect("Failed to read config directory") {
                let entry = entry.expect("Failed to read entry");
                let path = entry.path();
                if path.is_dir() {
                    let arch = path.file_name().unwrap().to_str().unwrap();
                    println!("Building for architecture: {}", arch);
                    build_linux(arch);
                }
            }
        }
        Commands::Clean => {
            let build_dir = Path::new("build");
            if build_dir.exists() {
                fs::remove_dir_all(build_dir).expect("Failed to remove build directory");
                println!("Build directory cleaned.");
            } else {
                println!("Build directory does not exist.");
            }
        }
    }
}

fn build_linux(arch: &str) {
    println!("Starting build for architecture: {}", arch);
    let config_path = PathBuf::from("config").join(arch).join("config");
    let linux_dir = "linux";
    let build_dir = PathBuf::from("build").join(arch).join("linux");

    // Copy config
    if !Path::new(&config_path).exists() {
        eprintln!("Config file not found for arch: {}", arch);
        return;
    }
    fs::create_dir_all(&build_dir).expect("Failed to create build directory");
    fs::copy(&config_path, build_dir.join(".config")).expect("Failed to copy config");

    // Detect host architecture and set cross-compile prefix if needed
    let host_arch = get_host_arch();
    let (kernel_arch, cross_compile_prefix) = get_arch_config(arch, &host_arch);

    // Build make arguments
    let mut make_args = vec![
        format!("O={}", build_dir.canonicalize().unwrap().display()),
        format!("ARCH={}", kernel_arch),
        format!("-j{}", num_cpus()),
    ];

    // Add CROSS_COMPILE if cross-compiling
    if let Some(prefix) = cross_compile_prefix {
        println!("Cross-compiling for {} using {}", arch, prefix);
        make_args.push(format!("CROSS_COMPILE={}", prefix));
    } else {
        println!("Native compilation for {}", arch);
    }

    // Run make
    println!("Running make for {} with args: {:?}", arch, make_args);
    let mut cmd = Command::new("make");
    cmd.current_dir(linux_dir).args(make_args);
    println!("{:?}", cmd);

    let status = cmd.status().expect("Failed to run make");

    if !status.success() {
        eprintln!("Make failed for arch: {}", arch);
        return;
    }

    println!("Build completed for {}: {}", arch, build_dir.display());
}

fn num_cpus() -> usize {
    std::thread::available_parallelism().unwrap().get()
}

/// Get the host architecture
fn get_host_arch() -> String {
    let output = Command::new("uname")
        .arg("-m")
        .output()
        .expect("Failed to get host architecture");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Get kernel architecture name and cross-compile prefix based on target arch and host arch
fn get_arch_config(target_arch: &str, host_arch: &str) -> (String, Option<String>) {
    // Map config directory names to kernel ARCH values
    let kernel_arch = match target_arch {
        "x86" => "x86_64",
        "arm64" => "arm64",
        // Add more mappings as needed
        _ => target_arch,
    };

    // Determine if cross-compilation is needed and set appropriate prefix
    let cross_compile_prefix = match (target_arch, host_arch) {
        // Native compilation cases
        ("x86", arch) if arch.starts_with("x86_64") || arch == "i686" => None,
        ("arm64", "aarch64") => None,

        // Cross-compilation cases
        ("arm64", _) => {
            // Check if aarch64-linux-gnu-gcc exists
            if Command::new("which")
                .arg("aarch64-linux-gnu-gcc")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                Some("aarch64-linux-gnu-".to_string())
            } else {
                eprintln!("Warning: aarch64-linux-gnu-gcc not found, cross-compilation may fail");
                Some("aarch64-linux-gnu-".to_string())
            }
        }
        ("x86", _) => {
            // For x86 on non-x86 hosts
            if Command::new("which")
                .arg("x86_64-linux-gnu-gcc")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                Some("x86_64-linux-gnu-".to_string())
            } else {
                eprintln!("Warning: x86_64-linux-gnu-gcc not found, trying native compilation");
                None
            }
        }
        // Default case - try native compilation
        _ => None,
    };

    (kernel_arch.to_string(), cross_compile_prefix)
}
