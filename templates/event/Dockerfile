
# https://hub.docker.com/_/rust
FROM rust:1 as build-env
WORKDIR /app
COPY . /app
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
ENV PORT 8080
COPY --from=build-env /app/target/release/axum-example /
ENTRYPOINT ["/axum-example"]
