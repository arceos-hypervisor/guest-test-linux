use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::system::num_cpus;
use crate::rootfs::create_rootfs_for_config;

/// Download busybox if not exists
pub fn download_busybox() {
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

/// Build busybox and create rootfs for a specific configuration
pub fn build_busybox_and_rootfs_for_config(config_name: &str, arch: &str, kernel_arch: &str, cross_compile_prefix: &Option<String>) {
    println!(
        "Starting busybox build and rootfs creation for configuration: {}",
        config_name
    );

    // Download busybox if not exists
    download_busybox();

    // Build busybox
    build_busybox_for_config(config_name, cross_compile_prefix);

    // Create rootfs
    create_rootfs_for_config(config_name, arch, kernel_arch, cross_compile_prefix);
}

/// Build busybox for a specific configuration
fn build_busybox_for_config(config_name: &str, cross_compile_prefix: &Option<String>) {
    println!("Building busybox for configuration: {}", config_name);

    let busybox_dir = "busybox";
    let build_dir = PathBuf::from("build").join(config_name).join("busybox");

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
        eprintln!("Busybox configuration failed for configuration: {}", config_name);
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
            } else if line.starts_with("CONFIG_TC=y") {
                "# CONFIG_TC is not set".to_string()
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
        eprintln!("Busybox build failed for configuration: {}", config_name);
        return;
    }

    println!("Busybox build completed for {}", config_name);
}
