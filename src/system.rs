use std::process::Command;

/// Get the number of available CPU cores
pub fn num_cpus() -> usize {
    std::thread::available_parallelism().unwrap().get()
}

/// Get the host architecture
pub fn get_host_arch() -> String {
    let output = Command::new("uname")
        .arg("-m")
        .output()
        .expect("Failed to get host architecture");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Get kernel architecture name and cross-compile prefix based on target arch and host arch
pub fn get_arch_config(target_arch: &str, host_arch: &str) -> (String, Option<String>) {
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
