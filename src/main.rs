use clap::{Args, Parser, Subcommand};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};
use std::{env, fs};
mod init;
#[derive(Parser, Debug)]
#[command(
    name = "cargo-cloudrun",
    about = "A custom cargo subcommand to manage Google Cloud Run deployments."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Deploy(DeployArgs),
    Init(InitArgs),
}

#[derive(Args, Debug)]
struct DeployArgs {
    /// Additional flags or arguments to pass through to `gcloud`.
    #[arg(trailing_var_arg = true)]
    extra_args: Vec<String>,
}
#[derive(Args, Debug)]
struct InitArgs {
    /// The name of the package to create (if not already in a package)
    package_name: String,

    /// If set, create an HTTP handler (default = true).
    /// This conflicts with --event and --event-type.
    #[arg(long, default_value = "true", conflicts_with_all = &["event", "event_type"])]
    http: bool,

    /// If set, create an event-based handler.
    /// Conflicts with --http and --event-type.
    #[arg(long, conflicts_with_all = &["http"])]
    event: bool,

    /// Specify the event type for the event-based handler.
    /// Conflicts with --http and --event.
    #[arg(long, conflicts_with_all = &["http"])]
    event_type: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Deploy(deploy_args) => deploy(deploy_args),
        Commands::Init(init_args) => {
            if let Err(err) = init::handle_init(init_args) {
                eprintln!("Failed to init new project: {err}");
                exit(1);
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
        r#"# Use the official Rust image.
# https://hub.docker.com/_/rust
FROM rust

# Copy local code to the container image.
WORKDIR /usr/src/app
COPY . .

# Install production dependencies and build a release artifact.
RUN cargo build --release

# Service must listen to $PORT environment variable.
# This default value facilitates local development.
ENV PORT 8080

# Run the web service on container startup.
ENTRYPOINT ["target/release/{}"]
"#,
        root_package_name
    );

    // if Rc::new(fs::File("Dockerfile")) {}
    let mut delete_dockerfile = false;
    if fs::File::open("Dockerfile").is_err() {
        // 4. Write the Dockerfile in the crate root
        let dockerfile_path = root_dir.join("Dockerfile");
        if let Err(err) = fs::write(&dockerfile_path, &dockerfile_content) {
            eprintln!("Failed to write Dockerfile: {err}");
            exit(1);
        }
        delete_dockerfile = true;
    }
    let mut cmd_args = vec![
        String::from("run"),
        String::from("deploy"),
        root_package_name.clone(),
        String::from("--source"),
        String::from("."),
    ];
    cmd_args.extend_from_slice(&args.extra_args); // Append e.g. ["--region", "us-central1"]

    // 5. Run `gcloud run deploy` with the user-provided extra args
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




