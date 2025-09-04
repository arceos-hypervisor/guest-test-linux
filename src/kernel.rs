use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use crate::config::parse_config_name;
use crate::system::{get_host_arch, get_arch_config, num_cpus};

/// Build Linux for a specific configuration
pub fn build_linux_for_config(config_name: &str) {
    let (arch, name) = parse_config_name(config_name);
    
    println!("Starting build for configuration: {}", config_name);
    let config_path = PathBuf::from("config").join(&arch).join(&name);
    let linux_dir = "linux";
    let build_dir = PathBuf::from("build").join(config_name).join("linux");

    // Copy config
    if !Path::new(&config_path).exists() {
        eprintln!("Config file not found for configuration: {}", config_name);
        return;
    }
    fs::create_dir_all(&build_dir).expect("Failed to create build directory");
    fs::copy(&config_path, build_dir.join(".config")).expect("Failed to copy config");

    // Detect host architecture and set cross-compile prefix if needed
    let host_arch = get_host_arch();
    let (kernel_arch, cross_compile_prefix) = get_arch_config(&arch, &host_arch);
    let cross_compile_prefix_clone = cross_compile_prefix.clone();

    // Build make arguments - select appropriate target based on architecture
    let kernel_target = match arch.as_str() {
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
        println!("Cross-compiling for {} using {}", config_name, prefix);
        make_args.push(format!("CROSS_COMPILE={}", prefix));
    } else {
        println!("Native compilation for {}", config_name);
    }

    // Run make
    println!(
        "Running make for {} with target {} and args: {:?}",
        config_name, kernel_target, make_args
    );
    let mut cmd = Command::new("make");
    cmd.current_dir(linux_dir).args(make_args);
    println!("{:?}", cmd);

    let status = cmd.status().expect("Failed to run make");

    if !status.success() {
        eprintln!("Make failed for configuration: {}", config_name);
        return;
    }

    println!(
        "Linux kernel build completed for {}: {}",
        config_name,
        build_dir.display()
    );

    // Copy kernel image to build/config_name directory
    copy_kernel_image(config_name, &arch, kernel_target, &build_dir);

    // Build busybox and create rootfs
    crate::busybox::build_busybox_and_rootfs_for_config(config_name, &arch, &kernel_arch, &cross_compile_prefix_clone);
}

/// Copy kernel image to the output directory
fn copy_kernel_image(config_name: &str, arch: &str, kernel_target: &str, build_dir: &Path) {
    println!("Copying kernel image for configuration: {}", config_name);

    let output_dir = PathBuf::from("build").join(config_name);
    fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Determine source kernel image path based on architecture and target
    let source_kernel_path = match arch {
        "arm64" => {
            // For arm64, the Image is in arch/arm64/boot/Image
            build_dir
                .join("arch")
                .join("arm64")
                .join("boot")
                .join("Image")
        }
        "x86" => {
            // For x86, the bzImage is in arch/x86/boot/bzImage
            build_dir
                .join("arch")
                .join("x86")
                .join("boot")
                .join("bzImage")
        }
        _ => {
            eprintln!("Unsupported architecture for kernel copy: {}", arch);
            return;
        }
    };

    if !source_kernel_path.exists() {
        eprintln!(
            "Kernel image not found at: {}",
            source_kernel_path.display()
        );
        return;
    }

    // Copy kernel image to build/config_name directory
    let dest_kernel_path = output_dir.join(kernel_target);

    match fs::copy(&source_kernel_path, &dest_kernel_path) {
        Ok(_) => {
            println!(
                "Kernel image copied successfully: {} -> {}",
                source_kernel_path.display(),
                dest_kernel_path.display()
            );
        }
        Err(e) => {
            eprintln!("Failed to copy kernel image: {}", e);
        }
    }
}
