use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};
use std::{env, fs, thread};

mod init;
#[derive(Parser)] // requires `derive` feature
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
#[command(styles = CLAP_STYLING)]
enum CargoCli {
    #[command(name = "cloudrun")]
    CloudRun(Cli),
}

#[derive(Args, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

// about = "A custom cargo subcommand to manage Google Cloud Run deployments."
#[derive(Subcommand, Debug)]
enum Commands {
    Deploy(DeployArgs),
    Init, // No additional args needed for Init
    New(NewArgs), // Assuming NewArgs might differ from InitArgs
}

#[derive(Args, Debug)]
struct DeployArgs {
    /// Additional flags or arguments to pass through to `gcloud`.
    #[arg(trailing_var_arg = true)]
    extra_args: Vec<String>,
}
#[derive(Args, Debug)]
struct NewArgs {
    /// The name of the package to create
    package_name: String,

    /// If set, create an HTTP handler (default = true).
    #[arg(long, default_value = "true", conflicts_with_all = &["event", "event_type"])]
    http: bool,

    /// If set, create an event-based handler.
    #[arg(long, conflicts_with_all = &["http"])]
    event: bool,

    /// Specify the event type for the event-based handler.
    #[arg(long, conflicts_with_all = &["http"])]
    event_type: Option<String>,
}

pub const CLAP_STYLING: clap::builder::styling::Styles = clap::builder::styling::Styles::styled()
    .header(clap_cargo::style::HEADER)
    .usage(clap_cargo::style::USAGE)
    .literal(clap_cargo::style::LITERAL)
    .placeholder(clap_cargo::style::PLACEHOLDER)
    .error(clap_cargo::style::ERROR)
    .valid(clap_cargo::style::VALID)
    .invalid(clap_cargo::style::INVALID);

fn main() {
    let cli = CargoCli::parse();

    match &cli {
        CargoCli::CloudRun(cli) => {
            match &cli.command {
                Commands::Deploy(deploy_args) => deploy(deploy_args),

                Commands::New(new_args) => {
                    if let Err(err) = init::handle_new(new_args) {
                        eprintln!("Failed to create new project: {err}");
                        exit(1);
                    }
                },

                Commands::Init => {
                    let mut package_name = "".to_string();
                    
                    // Try to get the current directory name as the package name
                    if let Ok(current_dir) = env::current_dir() {
                        if let Some(dir_name) = current_dir.file_name() {
                            if let Some(name) = dir_name.to_str() {
                                package_name = name.to_string();
                            }
                        }
                    }
                    // Create NewArgs with the current directory's name
                    let new_args = NewArgs {
                        package_name,
                        http: true, // Set default values or derive from InitArgs if needed
                        event: false,
                        event_type: None,
                    };

                    // Delegate to handle_new function
                    if let Err(err) = init::handle_new(&new_args) {
                        eprintln!("Failed to initialize project in current directory: {err}");
                        exit(1);
                    }
                }
            }
        }
    }
}

fn deploy(args: &DeployArgs) {
    // 1. Find the workspace root and the root package name
    let (root_dir, root_package_name) = match find_root_package() {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("Failed to determine root package: {err}");
            exit(1);
        }
    };

    // 2. Change directory to the root package directory
    if let Err(err) = env::set_current_dir(&root_dir) {
        eprintln!(
            "Failed to change directory to {}: {err}",
            root_dir.display()
        );
        exit(1);
    }

    // 3. Build the Dockerfile content, referencing the found package name
    let dockerfile_content = format!(
        r#"
# https://hub.docker.com/_/rust
FROM rust:1 as build-env
WORKDIR /app
COPY . /app
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
ENV PORT 8080
COPY --from=build-env /app/target/release/{} /
ENTRYPOINT ["/{}"]
"#,
        root_package_name,
        root_package_name
    );

    // if Rc::new(fs::File("Dockerfile")) {}
    let mut delete_dockerfile = false;
    if File::open(root_dir.join("Dockerfile")).is_err() {
        // 4. Write the Dockerfile in the crate root
        let dockerfile_path = root_dir.join("Dockerfile");
        if let Err(err) = fs::write(&dockerfile_path, &dockerfile_content) {
            eprintln!("Failed to write Dockerfile: {err}");
            exit(1);
        }
        delete_dockerfile = true;
    }

    if !root_dir.join(".gcloudignore").exists() {
        if let Err(e) = create_gcloudignore() {
            let gcloudignore_path = root_dir.join(".gcloudignore");
            eprintln!("Warning: Failed to create {}: {}", gcloudignore_path.display(), e);
        }
    }

    // let previous_image = Command::new("gcloud")
    //     .args([
    //         "run",
    //         "services",
    //         "describe",
    //         &root_package_name,
    //         "--format=value(image)"
    //     ])
    //     .output()
    //     .ok()
    //     .and_then(|output| {
    //         if output.status.success() {
    //             String::from_utf8_lossy(&output.stdout)
    //                 .trim()
    //                 .to_string()
    //                 .into()
    //         } else {
    //             None
    //         }
    //     })
    //     .filter(|s| !s.is_empty())
    //     .map(|s| format!("--cache-from={}", s))
    //     .unwrap_or_default();

    let mut cmd_args = vec![
        "run".to_string(),
        "deploy".to_string(),
        root_package_name.clone(),
        "--source".to_string(),
        ".".to_string(),
        "--allow-unauthenticated".to_string(),
        "--use-http2".to_string()
    ];

    // if !previous_image.is_empty() {
    //     cmd_args.push(previous_image);
    // }
    
    // Add any additional arguments from DeployArgs
    if !args.extra_args.is_empty() {
        if !cmd_args.is_empty() {
            cmd_args.push("--".to_string());
        }
        cmd_args.extend(args.extra_args.iter().cloned());
    }
    
    let status = Command::new("gcloud")
        .args(&cmd_args)
        .status()
        .expect("Failed to spawn gcloud process");

    if !status.success() {
        eprintln!("gcloud run deploy failed with status: {:?}", status.code());
        maybe_delete_dockerfile(&mut delete_dockerfile);
        exit(1);
    }
    maybe_delete_dockerfile(&mut delete_dockerfile);
}

fn maybe_delete_dockerfile(delete_dockerfile: &mut bool) {
    if *delete_dockerfile {
        if let Err(e) = fs::remove_file("Dockerfile") {
            eprintln!("Warning: Failed to delete temporary Dockerfile: {}", e);
        }
    }
}

/// Find the Cargo workspace root and the *root package name* using `cargo metadata`.
    /// Returns a tuple: (workspace_root_path, root_package_name).
    ///
/// If the workspace root has a virtual manifest (no package in root), falls back to using
/// the current package but still deploys from the workspace root to maintain dependencies.
fn find_root_package() -> Result<(PathBuf, String), Box<dyn std::error::Error>> {
    // Run `cargo metadata --format-version=1`
    let output = Command::new("cargo")
        .args(["metadata", "--format-version=1"])
        .output()?;

    if !output.status.success() {
        return Err("`cargo metadata` failed".into());
    }

    // Parse JSON
    let v: Value = serde_json::from_slice(&output.stdout)?;

    // Extract workspace_root
    let Some(workspace_root_str) = v.get("workspace_root").and_then(Value::as_str) else {
        return Err("No 'workspace_root' found in cargo metadata".into());
    };
    let workspace_root = PathBuf::from(workspace_root_str);

    // Look through the "packages" array
    let Some(packages) = v.get("packages").and_then(Value::as_array) else {
        return Err("'packages' not found or is not an array in cargo metadata".into());
    };

    let manifest_path = workspace_root
        .join("Cargo.toml")
        .to_string_lossy()
        .to_string();

    // First try to find a package at the workspace root
    for pkg in packages {
        let pkg_manifest_path = pkg
            .get("manifest_path")
            .and_then(Value::as_str)
            .unwrap_or_default();

        // Compare them in a platform-agnostic way
        if same_file_path(&pkg_manifest_path, &manifest_path) {
            // Found the root package
            let Some(pkg_name) = pkg.get("name").and_then(Value::as_str) else {
                return Err("Package in root has no 'name' in cargo metadata".into());
            };
            return Ok((workspace_root, pkg_name.to_owned()));
        }
    }

    // No package at workspace root (virtual manifest) - find the current package instead,
    // but still return the workspace root as the directory to build from
    let current_dir = env::current_dir()?;
    let mut current_package_name = None;
    
    // Try to find a package that contains the current directory
    for pkg in packages {
        let Some(pkg_manifest_path) = pkg.get("manifest_path").and_then(Value::as_str) else {
            continue;
        };
        
        // Get the directory of the package manifest
        let pkg_dir = Path::new(pkg_manifest_path).parent().unwrap_or(Path::new(""));
        
        // Debug info
        // eprintln!("Checking package at {}: current_dir={}", pkg_dir.display(), current_dir.display());
        
        // Check if the current directory starts with this package directory
        // This is a simplified check - we might need a more robust method
        if let Ok(rel_path) = current_dir.strip_prefix(pkg_dir) {
            if !rel_path.as_os_str().is_empty() && rel_path.components().count() > 0 {
                // We're not in the package directory, skip
                continue;
            }
            
            let Some(pkg_name) = pkg.get("name").and_then(Value::as_str) else {
                continue;
            };
            
            current_package_name = Some(pkg_name.to_owned());
            break;
        };
    }

    if let Some(package_name) = current_package_name {
        return Ok((workspace_root, package_name));
    }

    Err("Could not find a suitable package to deploy. Neither a root package nor a package at the current directory was found.".into())
}

/// Compare two file paths in a slightly more robust way.
/// (On Windows, e.g., backslash vs forward slash).
fn same_file_path(a: &str, b: &str) -> bool {
    // Convert both to a canonical PathBuf
    let path_a = Path::new(a).components().collect::<Vec<_>>();
    let path_b = Path::new(b).components().collect::<Vec<_>>();
    path_a == path_b
}

use std::fs::File;
use std::io::Write;

fn create_gcloudignore() -> std::io::Result<()> {
    let root_dir = match find_root_package() {
        Ok((dir, _)) => dir,
        Err(_) => PathBuf::from("."), // Fallback to current directory if can't determine workspace root
    };
    let gcloudignore_content = r#"# Rust build artifacts
/target/
/debug/
/target/**/*
.git
.gitignore
.gcloudignore"#;

    let mut file = File::create(root_dir.join(".gcloudignore"))?;
    file.write_all(gcloudignore_content.as_bytes())?;
    Ok(())
}




