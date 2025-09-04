use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;
use std::process::Command;

mod busybox;
mod config;
mod kernel;
mod rootfs;
mod system;

use config::{is_valid_config, list_configs};
use kernel::build_linux_for_config;

#[derive(Parser)]
#[command(author, version, about = "Manage Linux 6.12 source code and builds")]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build Linux for a specific configuration
    Build {
        /// Configuration name in format arch-name (e.g., arm64-qemu, x86-qemu)
        config: String,
    },
    /// Clean the build directory
    Clean,
    /// List all available configurations
    List,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    match args.command {
        Commands::Build { config } => {
            // Validate the config format and existence
            if !is_valid_config(&config) {
                eprintln!("Invalid configuration: {}", config);
                eprintln!("Use 'list' command to see available configurations.");
                return;
            }

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

            // Build for the specific configuration
            println!("Building for configuration: {}", config);
            build_linux_for_config(&config);
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
        Commands::List => {
            list_configs();
        }
    }
}
