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
        .output();

    if let Ok(output) = output
        && output.status.success()
    {
        let size_str = String::from_utf8_lossy(&output.stdout);
        if let Some(size_part) = size_str.split_whitespace().next()
            && let Ok(size) = size_part.parse::<u64>()
        {
            return size;
        }
    }

    // Fallback: estimate by walking files if du fails (small, conservative number)
    let mut total_bytes: u64 = 0;
    if let Ok(entries) = rootfs_dir.read_dir() {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(meta) = fs::metadata(&path) {
                total_bytes = total_bytes.saturating_add(meta.len());
            }
        }
    }

    // Convert bytes to MB, add minimal default
    let mut size_mb = if total_bytes > 0 {
        total_bytes / (1024 * 1024)
    } else {
        50
    };
    if size_mb == 0 {
        size_mb = 50;
    }
    size_mb
}

/// Create rootfs for a specific configuration
pub fn create_rootfs_for_config(
    config_name: &str,
    _arch: &str,
    kernel_arch: &str,
    cross_compile_prefix: &Option<String>,
) {
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
        eprintln!(
            "Failed to install busybox for configuration: {}",
            config_name
        );
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
        eprintln!(
            "Failed to install kernel modules for configuration: {}",
            config_name
        );
        // Continue anyway, modules might not be essential
    }

    // Create init script
    create_init_script(&rootfs_dir);

    // Locate kernel image and copy into rootfs boot directory
    println!("Locating kernel image and copying into rootfs boot directory...");

    // Determine possible kernel image paths
    let possible_images = vec![
        linux_build_dir
            .join("arch")
            .join("arm64")
            .join("boot")
            .join("Image"),
        linux_build_dir
            .join("arch")
            .join("x86")
            .join("boot")
            .join("bzImage"),
        linux_build_dir.join("Image"),
        linux_build_dir.join("bzImage"),
    ];

    let mut found_image: Option<PathBuf> = None;
    for img in possible_images {
        if img.exists() {
            found_image = Some(img);
            break;
        }
    }

    if let Some(img_path) = found_image {
        // Copy the kernel image to rootfs/boot for common layout
        let boot_dir = rootfs_dir.join("boot");
        if let Err(e) = fs::create_dir_all(&boot_dir) {
            eprintln!("Failed to create boot dir {:?}: {}", boot_dir, e);
        } else {
            let boot_dest = boot_dir.join(img_path.file_name().unwrap());
            match fs::copy(&img_path, &boot_dest) {
                Ok(_) => println!(
                    "Copied kernel image to boot: {} -> {}",
                    img_path.display(),
                    boot_dest.display()
                ),
                Err(e) => eprintln!("Failed to copy kernel image to boot: {}", e),
            }
        }
    } else {
        println!("No kernel image found in build directory to copy into rootfs");
    }

    // Create rootfs image
    create_rootfs_image(config_name, &rootfs_dir, &output_dir);
}

/// Create rootfs image file
fn create_rootfs_image(config_name: &str, rootfs_dir: &Path, output_dir: &Path) {
    println!("Creating rootfs.img...");
    let rootfs_img = output_dir.join("rootfs.img");

    // Calculate size (add larger safety margin: 30% + 200MB minimum)
    let base_size = calculate_rootfs_size(rootfs_dir);
    let mut size_mb = (base_size as f64 * 1.3).ceil() as u64;
    // Ensure at least base + 200MB extra
    if size_mb < base_size + 200 {
        size_mb = base_size + 200;
    }

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
        eprintln!(
            "Failed to create rootfs image file for configuration: {}",
            config_name
        );
        return;
    }

    // Format as ext4
    let status = Command::new("mkfs.ext4")
        .args(["-F", "-O", "^metadata_csum_seed", &rootfs_img.to_string_lossy()])
        .status()
        .expect("Failed to format rootfs image");

    if !status.success() {
        eprintln!(
            "Failed to format rootfs image for configuration: {}",
            config_name
        );
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
        eprintln!(
            "Failed to mount rootfs image for configuration: {}",
            config_name
        );
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
        eprintln!(
            "Failed to copy files to rootfs image for configuration: {}",
            config_name
        );
    }
}
