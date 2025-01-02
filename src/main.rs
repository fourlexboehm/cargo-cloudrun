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
                    let package_name = "".to_string();

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
    if File::open("Dockerfile").is_err() {
        // 4. Write the Dockerfile in the crate root
        let dockerfile_path = root_dir.join("Dockerfile");
        if let Err(err) = fs::write(&dockerfile_path, &dockerfile_content) {
            eprintln!("Failed to write Dockerfile: {err}");
            exit(1);
        }
        delete_dockerfile = true;
    }

    if !Path::new(".gcloudignore").exists() {
        if let Err(e) = create_gcloudignore() {
            eprintln!("Warning: Failed to create .gcloudignore: {}", e);
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
        "run",
        "deploy",
        &*root_package_name,
        "--source",
        ".",
        "--allow-unauthenticated",
        "--use-http2"
    ];

    // if !previous_image.is_empty() {
    //     cmd_args.push(previous_image);
    // }
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
        fs::remove_file("Dockerfile").unwrap();
    }
}

/// Find the Cargo workspace root and the *root package name* using `cargo metadata`.
    /// Returns a tuple: (workspace_root_path, root_package_name).
    ///
    /// Assumes there *is* a package in the workspace root (i.e., not just a virtual manifest).
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

        // Look through the "packages" array and see which package has
        // `manifest_path` = workspace_root + "Cargo.toml"
        let Some(packages) = v.get("packages").and_then(Value::as_array) else {
            return Err("'packages' not found or is not an array in cargo metadata".into());
        };

        let manifest_path = workspace_root
            .join("Cargo.toml")
            .to_string_lossy()
            .to_string();

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

        Err(format!(
            "Did not find a package with manifest_path = {manifest_path}"
        ))?
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
use std::io::{BufRead, BufReader, Write};

fn create_gcloudignore() -> std::io::Result<()> {
    let gcloudignore_content = r#"# Rust build artifacts
/target/
/debug/
/target/**/*
.git
.gitignore
.gcloudignore"#;

    let mut file = File::create(".gcloudignore")?;
    file.write_all(gcloudignore_content.as_bytes())?;
    Ok(())
}




