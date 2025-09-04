use std::fs;
use std::path::{Path, PathBuf};

/// Check if the given config name is valid and exists
pub fn is_valid_config(config_name: &str) -> bool {
    let parts: Vec<&str> = config_name.split('-').collect();
    if parts.len() < 2 {
        return false;
    }

    let arch = parts[0];
    let name = parts[1..].join("-");

    let config_path = PathBuf::from("config").join(arch).join(&name);
    config_path.exists()
}

/// List all available configurations
pub fn list_configs() {
    println!("Available configurations:");
    let config_dir = Path::new("config");

    if !config_dir.exists() {
        eprintln!("Config directory does not exist.");
        return;
    }

    let mut configs = Vec::new();

    // Traverse architecture directories
    for arch_entry in fs::read_dir(config_dir).expect("Failed to read config directory") {
        let arch_entry = arch_entry.expect("Failed to read arch entry");
        let arch_path = arch_entry.path();

        if arch_path.is_dir() {
            let arch_name = arch_path.file_name().unwrap().to_str().unwrap();

            // Traverse config files within each architecture directory
            for config_entry in fs::read_dir(&arch_path).expect("Failed to read arch directory") {
                let config_entry = config_entry.expect("Failed to read config entry");
                let config_path = config_entry.path();

                if config_path.is_file() {
                    let config_name = config_path.file_name().unwrap().to_str().unwrap();
                    let full_config_name = format!("{}-{}", arch_name, config_name);
                    configs.push(full_config_name);
                }
            }
        }
    }

    // Sort configurations for consistent output
    configs.sort();

    if configs.is_empty() {
        println!("No configurations found.");
    } else {
        for config in configs {
            println!("  {}", config);
        }
    }
}

/// Parse config name into arch and name components
pub fn parse_config_name(config_name: &str) -> (String, String) {
    let parts: Vec<&str> = config_name.split('-').collect();
    let arch = parts[0].to_string();
    let name = parts[1..].join("-");
    (arch, name)
}
