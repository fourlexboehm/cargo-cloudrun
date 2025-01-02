# cargo-cloudrun

**cargo-cloudrun** is a command-line tool for deploying Rust applications to [Google Cloud Run](https://cloud.google.com/run). It wraps the `gcloud` CLI to simplify the deployment process—**no manual Docker builds** or direct interaction with the **Google Cloud Console** required.

Inspired by the ergonomics of [`cargo-lambda`](https://github.com/cargo-lambda/cargo-lambda), **cargo-cloudrun** can be used to create Cloud Run services that handle **HTTP requests** or **event triggers** in a function-like style.

It also supports **monolithic** Rust applications.
It uses [google-cloudevents](https://github.com/fourlexboehm/google-cloudevents-rs) to handle cloud events from the axum runtime.
This is nessesary for cloud events but cargo cloudrun can be used with any runtime. as long as it binds to port `8080`.

## Features

- **Easy Deployments**  
  Deploy Rust applications to Cloud Run with a single command (`cargo cloudrun deploy`)—no Dockerfiles or manual steps needed.

- **HTTP & Event Support**  
  Similar to Cloud Run Functions, you can build services that handle HTTP requests or respond to events.

- **Monolithic or Microservice**  
  Works equally well for single-crate monoliths or multi-crate, microservice-style architectures.

- **No Docker Skills Needed**  
  Under the hood, cargo-cloudrun uses the `gcloud` CLI and automatically manages Docker images for you.

## Quick Start

**Install** `cargo-cloudrun`:

   ```
    cargo install cargo-cloudrun
   ```

## Usage
    
```bash
cargo cloudrun new my-service
cd my-service
cargo cloudrun deploy
```
