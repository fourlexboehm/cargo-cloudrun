use std::error::Error;
use std::{env, fs, io};
use dialoguer::{Select, Sort};
use dialoguer::theme::ColorfulTheme;
use google_cloudevents::ALL_EVENT_PATHS;
use std::path::Path;
use crate::{NewArgs};

pub const EVENT_CARGO_TOML: &str = include_str!("event/Cargo.toml");
pub const EVENT_MAIN_RS: &str = include_str!("../templates/event/src/main.rs");
pub const HTTP_CARGO_TOML: &str = include_str!("../templates/http/NotCargo.toml");
pub const HTTP_MAIN_RS: &str = include_str!("../templates/http/src/main.rs");
pub fn handle_new(args: &NewArgs) -> Result<(), Box<dyn Error>> {
    dbg!(&args);
    let current_dir = env::current_dir()?;
    let new_project_dir = current_dir.join(&args.package_name);

    // Determine if the package is 'event'
    let is_event_package = args.event || args.event_type.is_some();

    let selected_event_type = if is_event_package {
        match &args.event_type {
            Some(event) => {
                Some(map_event_type(event)?)
            },
            None => {
                // Prompt the user to select an event type
                println!("Please select a Cloud Event type:");
                let selection = Select::with_theme(&ColorfulTheme::default())
                    .items(&ALL_EVENT_PATHS.iter().map(|s| s[36..].to_string()).collect::<Vec<String>>())
                    .interact()?;

                Some(ALL_EVENT_PATHS[selection].to_string())
            }
        }
    } else {
        None
    };

    if new_project_dir.exists() && args.package_name != "" {
        return Err(format!("Directory '{}' already exists", args.package_name).into());
    }
    if new_project_dir.join("Cargo.toml").exists() {
        return Err(format!("Cargo.toml already exists in '{}'", new_project_dir.display()).into());
    }
    fs::create_dir_all(new_project_dir.join("src"))?;
    let pkg_name = if  args.package_name == "" {
        "axum_serverless"
    } else {
        &*args.package_name
    };

    dbg!(&pkg_name);

    if is_event_package {
        if let Some(event_type) = &selected_event_type {
            write_event_files(
                &new_project_dir,
                EVENT_CARGO_TOML,
                EVENT_MAIN_RS,
                pkg_name,
                event_type,
            )?;
        }
    } else {
        // Existing logic for non-event packages
        if let Some(_ev) = &args.event_type {
            write_files(
                &new_project_dir,
                EVENT_CARGO_TOML,
                EVENT_MAIN_RS,
                pkg_name,
            )?;
        } else {
            write_files(
                &new_project_dir,
                HTTP_CARGO_TOML,
                HTTP_MAIN_RS,
                pkg_name,
            )?;
        }
    }

    println!(
        "Successfully created new Axum project '{}' at '{}'",
        args.package_name,
        new_project_dir.display()
    );
    Ok(())
}
fn map_event_type(event_suffix: &str) -> Result<String, Box<dyn Error>> {
    // Find all events that end with the provided suffix
    let matches: Vec<&str> = ALL_EVENT_PATHS.iter()
        .filter(|event| event.ends_with(event_suffix))
        .cloned()
        .collect();

    match matches.len() {
        0 => Err(format!(
            "No event found with the suffix '{}'. Please provide a valid event type.",
            event_suffix
        ).into()),
        1 => Ok(matches[0].to_string()),
        _ => Err(format!(
            "Multiple events found with the suffix '{}'. Please specify the full event type.",
            event_suffix
        ).into()),
    }
}

/// Writes the embedded Cargo.toml & main.rs to disk for event packages,
/// updating the `[package] name` in Cargo.toml to `package_name` and
/// inserting the selected `event_type` into main.rs.
fn write_event_files(
    project_dir: &Path,
    cargo_toml_str: &str,
    main_rs_str: &str,
    package_name: &str,
    event_type: &str,
) -> io::Result<()> {
    // 1. Write Cargo.toml
    let cargo_toml_path = project_dir.join("Cargo.toml");
    let updated_cargo_toml = rewrite_package_name(cargo_toml_str, package_name);
    fs::write(cargo_toml_path, updated_cargo_toml)?;

    // 2. Write src/main.rs with the event type
    let main_rs_path = project_dir.join("src").join("main.rs");
    let updated_main_rs = main_rs_str.replace("google_cloudevents::google::events::cloud::firestore::v1::DocumentCreatedEvent", event_type)
        .replace("DocumentCreatedEvent", &*event_type[(event_type.rfind("::").unwrap() + 2)..].to_string());
    fs::write(main_rs_path, updated_main_rs)?;

    Ok(())
}

/// Writes the embedded Cargo.toml & main.rs to disk,
/// updating the `[package] name` in Cargo.toml to `package_name`.
fn write_files(
    project_dir: &Path,
    cargo_toml_str: &str,
    main_rs_str: &str,
    package_name: &str,
) -> io::Result<()> {
    // 1. Write Cargo.toml
    // EVENT_PATHS

    let cargo_toml_path = project_dir.join("Cargo.toml");
    let updated_cargo_toml = rewrite_package_name(cargo_toml_str, package_name);
    fs::write(cargo_toml_path, updated_cargo_toml)?;

    // 2. Write src/main.rs
    let main_rs_path = project_dir.join("src").join("main.rs");
    fs::write(main_rs_path, main_rs_str)?;

    Ok(())
}

/// Replaces the line `name = "whatever"` inside `[package]` with the user-provided `package_name`.
fn rewrite_package_name(toml_input: &str, package_name: &str) -> String {
    let mut in_package = false;
    let mut output = String::new();

    for line in toml_input.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("[package]") {
            in_package = true;
            output.push_str(line);
            output.push('\n');
            continue;
        }

        if in_package && trimmed.starts_with("name =") {
            output.push_str(&format!("name = \"{}\"\n", package_name));
            in_package = false; // Only replace once
            continue;
        }

        output.push_str(line);
        output.push('\n');
    }

    output
}