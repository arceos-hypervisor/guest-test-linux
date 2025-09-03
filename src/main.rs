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
    let cross_compile_prefix_clone = cross_compile_prefix.clone();

    // Build make arguments - select appropriate target based on architecture
    let kernel_target = match arch {
        "arm64" => "Image",
        "x86" => "bzImage",
        _ => "bzImage", // default for other architectures
    };

    let mut make_args = vec![
        format!("O={}", build_dir.canonicalize().unwrap().display()),
        format!("ARCH={}", kernel_arch),
        kernel_target.to_string(),
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
    println!(
        "Running make for {} with target {} and args: {:?}",
        arch, kernel_target, make_args
    );
    let mut cmd = Command::new("make");
    cmd.current_dir(linux_dir).args(make_args);
    println!("{:?}", cmd);

    let status = cmd.status().expect("Failed to run make");

    if !status.success() {
        eprintln!("Make failed for arch: {}", arch);
        return;
    }

    println!(
        "Linux kernel build completed for {}: {}",
        arch,
        build_dir.display()
    );

    // Build busybox and create rootfs
    build_busybox_and_rootfs(arch, &kernel_arch, &cross_compile_prefix_clone);
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

fn build_busybox_and_rootfs(arch: &str, kernel_arch: &str, cross_compile_prefix: &Option<String>) {
    println!(
        "Starting busybox build and rootfs creation for architecture: {}",
        arch
    );

    // Download busybox if not exists
    download_busybox();

    // Build busybox
    build_busybox(arch, cross_compile_prefix);

    // Create rootfs
    create_rootfs(arch, kernel_arch, cross_compile_prefix);
}

fn download_busybox() {
    let busybox_dir = Path::new("busybox");
    if busybox_dir.exists() {
        println!("Busybox source already exists, skipping download.");
        return;
    }

    println!("Cloning busybox 1_36_1 source code from Gitee...");
    let status = Command::new("git")
        .args([
            "clone",
            "--depth=1",
            "-b",
            "1_36_1",
            "https://gitee.com/mirrors_addons/busybox.git",
            "busybox",
        ])
        .status()
        .expect("Failed to clone busybox");

    if !status.success() {
        eprintln!("Failed to clone busybox from Gitee");
        return;
    }

    println!("Busybox clone completed.");
}

fn build_busybox(arch: &str, cross_compile_prefix: &Option<String>) {
    println!("Building busybox for architecture: {}", arch);

    let busybox_dir = "busybox";
    let build_dir = PathBuf::from("build").join(arch).join("busybox");

    // Create build directory
    fs::create_dir_all(&build_dir).expect("Failed to create busybox build directory");

    // Configure busybox with default config
    let mut make_args = vec![
        format!("O={}", build_dir.canonicalize().unwrap().display()),
        "defconfig".to_string(),
    ];

    if let Some(prefix) = cross_compile_prefix {
        make_args.push(format!("CROSS_COMPILE={}", prefix));
    }

    println!("Configuring busybox...");
    let status = Command::new("make")
        .current_dir(busybox_dir)
        .args(&make_args)
        .status()
        .expect("Failed to configure busybox");

    if !status.success() {
        eprintln!("Busybox configuration failed for arch: {}", arch);
        return;
    }

    // Enable static linking in busybox config
    println!("Enabling static compilation for busybox...");
    let config_path = build_dir.join(".config");
    let config_content = fs::read_to_string(&config_path).expect("Failed to read busybox config");

    // Enable CONFIG_STATIC and disable CONFIG_FEATURE_SHARED_BUSYBOX
    let modified_config = config_content
        .lines()
        .map(|line| {
            if line.starts_with("# CONFIG_STATIC is not set") {
                "CONFIG_STATIC=y".to_string()
            } else if line.starts_with("CONFIG_FEATURE_SHARED_BUSYBOX=y") {
                "# CONFIG_FEATURE_SHARED_BUSYBOX is not set".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join("\n");

    // If CONFIG_STATIC wasn't found, add it
    let final_config = if !modified_config.contains("CONFIG_STATIC=y") {
        format!("{}\nCONFIG_STATIC=y\n", modified_config)
    } else {
        modified_config
    };

    fs::write(&config_path, final_config).expect("Failed to write modified busybox config");

    println!("Static compilation enabled for busybox");

    // Build busybox
    let mut make_args = vec![
        format!("O={}", build_dir.canonicalize().unwrap().display()),
        format!("-j{}", num_cpus()),
    ];

    if let Some(prefix) = cross_compile_prefix {
        make_args.push(format!("CROSS_COMPILE={}", prefix));
    }

    println!("Building busybox...");
    let status = Command::new("make")
        .current_dir(busybox_dir)
        .args(&make_args)
        .status()
        .expect("Failed to build busybox");

    if !status.success() {
        eprintln!("Busybox build failed for arch: {}", arch);
        return;
    }

    println!("Busybox build completed for {}", arch);
}

fn create_rootfs(arch: &str, kernel_arch: &str, cross_compile_prefix: &Option<String>) {
    println!("Creating rootfs for architecture: {}", arch);

    let rootfs_dir = PathBuf::from("build").join(arch).join("rootfs");
    let busybox_build_dir = PathBuf::from("build").join(arch).join("busybox");
    let linux_build_dir = PathBuf::from("build").join(arch).join("linux");
    let output_dir = PathBuf::from("build").join(arch);

    // Clean and create rootfs directory
    if rootfs_dir.exists() {
        fs::remove_dir_all(&rootfs_dir).expect("Failed to remove existing rootfs directory");
    }
    fs::create_dir_all(&rootfs_dir).expect("Failed to create rootfs directory");

    // Install busybox
    println!("Installing busybox to rootfs...");
    let mut make_args = vec![
        format!("O={}", busybox_build_dir.canonicalize().unwrap().display()),
        format!(
            "CONFIG_PREFIX={}",
            rootfs_dir.canonicalize().unwrap().display()
        ),
        "install".to_string(),
    ];

    if let Some(prefix) = cross_compile_prefix {
        make_args.push(format!("CROSS_COMPILE={}", prefix));
    }

    let status = Command::new("make")
        .current_dir("busybox")
        .args(&make_args)
        .status()
        .expect("Failed to install busybox");

    if !status.success() {
        eprintln!("Failed to install busybox for arch: {}", arch);
        return;
    }

    // Debug: List contents of rootfs after busybox install
    println!("Checking busybox installation...");
    let _ = Command::new("ls")
        .args(["-la", &rootfs_dir.to_string_lossy()])
        .status();

    // Create additional directories
    let dirs = [
        "dev", "proc", "sys", "tmp", "var", "etc", "root", "home", "mnt",
    ];
    for dir in &dirs {
        fs::create_dir_all(rootfs_dir.join(dir)).expect("Failed to create directory in rootfs");
    }

    // Debug: List rootfs contents after creating directories
    println!("Rootfs contents after setup:");
    let _ = Command::new("bash")
        .args([
            "-c",
            &format!("find {} -type f | head -20", rootfs_dir.display()),
        ])
        .status();

    // Install kernel modules
    println!("Installing kernel modules...");
    let modules_dir = rootfs_dir.join("lib").join("modules");
    fs::create_dir_all(&modules_dir).expect("Failed to create modules directory");

    let mut make_args = vec![
        format!("O={}", linux_build_dir.canonicalize().unwrap().display()),
        format!("ARCH={}", kernel_arch),
        format!(
            "INSTALL_MOD_PATH={}",
            rootfs_dir.canonicalize().unwrap().display()
        ),
        "modules_install".to_string(),
    ];

    if let Some(prefix) = cross_compile_prefix {
        make_args.push(format!("CROSS_COMPILE={}", prefix));
    }

    let status = Command::new("make")
        .current_dir("linux")
        .args(&make_args)
        .status()
        .expect("Failed to install kernel modules");

    if !status.success() {
        eprintln!("Failed to install kernel modules for arch: {}", arch);
        // Continue anyway, modules might not be essential
    }

    // Create init script
    create_init_script(&rootfs_dir);

    // Create rootfs image
    println!("Creating rootfs.img...");
    let rootfs_img = output_dir.join("rootfs.img");

    // Calculate size (add some extra space)
    let size_mb = calculate_rootfs_size(&rootfs_dir) + 50; // Add 50MB extra

    // Create empty image file
    let status = Command::new("dd")
        .args([
            "if=/dev/zero",
            &format!("of={}", rootfs_img.display()),
            "bs=1M",
            &format!("count={}", size_mb),
        ])
        .status()
        .expect("Failed to create rootfs image file");

    if !status.success() {
        eprintln!("Failed to create rootfs image file for arch: {}", arch);
        return;
    }

    // Format as ext4
    let status = Command::new("mkfs.ext4")
        .args(["-F", &rootfs_img.to_string_lossy()])
        .status()
        .expect("Failed to format rootfs image");

    if !status.success() {
        eprintln!("Failed to format rootfs image for arch: {}", arch);
        return;
    }

    // Mount and copy files
    let mount_point = PathBuf::from("/tmp").join(format!("rootfs_mount_{}", arch));
    fs::create_dir_all(&mount_point).expect("Failed to create mount point");

    let status = Command::new("sudo")
        .args([
            "mount",
            "-o",
            "loop",
            &rootfs_img.to_string_lossy(),
            &mount_point.to_string_lossy(),
        ])
        .status()
        .expect("Failed to mount rootfs image");

    if !status.success() {
        eprintln!("Failed to mount rootfs image for arch: {}", arch);
        return;
    }

    // Copy rootfs contents
    println!("Copying rootfs contents to image...");

    // Check if rootfs directory has content
    if !rootfs_dir.exists()
        || fs::read_dir(&rootfs_dir)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true)
    {
        eprintln!("Warning: rootfs directory is empty or doesn't exist");
    }

    let copy_cmd = format!(
        "cd {} && sudo find . -mindepth 1 -maxdepth 1 -exec cp -a {{}} {} \\;",
        rootfs_dir.display(),
        mount_point.display()
    );

    let status = Command::new("bash").args(["-c", &copy_cmd]).status();

    // Change ownership of all files to root:root
    if status.is_ok() && status.as_ref().unwrap().success() {
        println!("Changing ownership of all files to root:root...");
        let chown_status = Command::new("sudo")
            .args(["chown", "-R", "root:root", &mount_point.to_string_lossy()])
            .status()
            .expect("Failed to change ownership");

        if !chown_status.success() {
            eprintln!(
                "Warning: Failed to change ownership to root:root for arch: {}",
                arch
            );
        } else {
            println!("Successfully changed ownership to root:root");
        }
    }

    // Unmount
    let _ = Command::new("sudo")
        .args(["umount", &mount_point.to_string_lossy()])
        .status();

    // Clean up mount point
    let _ = fs::remove_dir(&mount_point);

    if status.is_ok() && status.unwrap().success() {
        println!(
            "Rootfs image created successfully: {}",
            rootfs_img.display()
        );
    } else {
        eprintln!("Failed to copy files to rootfs image for arch: {}", arch);
    }
}

fn create_init_script(rootfs_dir: &Path) {
    let init_script = r#"#!/bin/sh

# Mount essential filesystems
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev

# Create device nodes if they don't exist
[ ! -c /dev/console ] && mknod /dev/console c 5 1
[ ! -c /dev/null ] && mknod /dev/null c 1 3

echo "Welcome to the guest Linux system!"
echo "BusyBox init system started."

# Start a shell
exec /bin/sh
"#;

    let init_path = rootfs_dir.join("init");
    fs::write(&init_path, init_script).expect("Failed to create init script");

    // Make it executable
    let status = Command::new("chmod")
        .args(["+x", &init_path.to_string_lossy()])
        .status()
        .expect("Failed to make init script executable");

    if !status.success() {
        eprintln!("Failed to make init script executable");
    }
}

fn calculate_rootfs_size(rootfs_dir: &Path) -> u64 {
    // Use du command to calculate directory size in MB
    let output = Command::new("du")
        .args(["-sm", &rootfs_dir.to_string_lossy()])
        .output()
        .expect("Failed to calculate rootfs size");

    if output.status.success() {
        let size_str = String::from_utf8_lossy(&output.stdout);
        if let Some(size_part) = size_str.split_whitespace().next()
            && let Ok(size) = size_part.parse::<u64>()
        {
            return size;
        }
    }

    // Default to 100MB if calculation fails
    100
}
