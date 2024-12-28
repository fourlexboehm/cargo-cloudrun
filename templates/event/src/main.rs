use axum::{
    extract::FromRequest,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use cloudevents::{Data, Event};
use google_cloudevents::google::events::firebase::auth::v1::UserCreatedEvent;
use http::StatusCode;
use prost::Message;
use tower_http::trace::TraceLayer;
use tracing::{debug, error};

fn echo_app() -> Router {
    Router::new()
        .route("/", get(|| async { "Hello from CloudEvents server" }))
        .route(
            "/",
            post(|event: Event| async move {
                let data = event
                    .data()
                    .map(|d| d.to_owned())
                    .and_then(|data| match data {
                        Data::Binary(bytes) => Some(UserCreatedEvent::decode(bytes.as_slice()).unwrap()),
                        Data::Json(value) => Some(serde_json::from_value(value).unwrap()),
                        Data::String(str) => {
                            error!("Unexpected string data: {}", str);
                            None
                            // return (StatusCode::BAD_REQUEST, "Unexpected string data".to_string());
                        },
                    });

                // Use (StatusCode, Event) return type since Event implements IntoResponse
                (StatusCode::OK, "Event received".to_string())
            }),
        )
        .layer(TraceLayer::new_for_http())
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "axum_example=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    // Build and run the application
    let app = echo_app();
    // run it
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}