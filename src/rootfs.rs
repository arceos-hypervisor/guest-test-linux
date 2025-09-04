use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Create init script in the rootfs directory
pub fn create_init_script(rootfs_dir: &Path) {
    let init_script = include_str!("../init/init");

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

/// Calculate the size of rootfs directory in MB
pub fn calculate_rootfs_size(rootfs_dir: &Path) -> u64 {
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

/// Create rootfs for a specific configuration
pub fn create_rootfs_for_config(config_name: &str, _arch: &str, kernel_arch: &str, cross_compile_prefix: &Option<String>) {
    println!("Creating rootfs for configuration: {}", config_name);

    let rootfs_dir = PathBuf::from("build").join(config_name).join("rootfs");
    let busybox_build_dir = PathBuf::from("build").join(config_name).join("busybox");
    let linux_build_dir = PathBuf::from("build").join(config_name).join("linux");
    let output_dir = PathBuf::from("build").join(config_name);

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
        eprintln!("Failed to install busybox for configuration: {}", config_name);
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
        eprintln!("Failed to install kernel modules for configuration: {}", config_name);
        // Continue anyway, modules might not be essential
    }

    // Create init script
    create_init_script(&rootfs_dir);

    // Create rootfs image
    create_rootfs_image(config_name, &rootfs_dir, &output_dir);
}

/// Create rootfs image file
fn create_rootfs_image(config_name: &str, rootfs_dir: &Path, output_dir: &Path) {
    println!("Creating rootfs.img...");
    let rootfs_img = output_dir.join("rootfs.img");

    // Calculate size (add some extra space)
    let size_mb = calculate_rootfs_size(rootfs_dir) + 50; // Add 50MB extra

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
        eprintln!("Failed to create rootfs image file for configuration: {}", config_name);
        return;
    }

    // Format as ext4
    let status = Command::new("mkfs.ext4")
        .args(["-F", &rootfs_img.to_string_lossy()])
        .status()
        .expect("Failed to format rootfs image");

    if !status.success() {
        eprintln!("Failed to format rootfs image for configuration: {}", config_name);
        return;
    }

    // Mount and copy files
    let mount_point = PathBuf::from("/tmp").join(format!("rootfs_mount_{}", config_name));
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
        eprintln!("Failed to mount rootfs image for configuration: {}", config_name);
        return;
    }

    // Copy rootfs contents
    println!("Copying rootfs contents to image...");

    // Check if rootfs directory has content
    if !rootfs_dir.exists()
        || fs::read_dir(rootfs_dir)
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
                "Warning: Failed to change ownership to root:root for configuration: {}",
                config_name
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
        eprintln!("Failed to copy files to rootfs image for configuration: {}", config_name);
    }
}
