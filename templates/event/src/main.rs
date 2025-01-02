use axum::{debug_handler, extract::FromRequest, response::IntoResponse, routing::{get, post}, Router};
use google_cloudevents::google::events::cloud::pubsub::v1::{MessagePublishedData, PubsubMessage};
use http::StatusCode;
use google_cloudevents::GoogleCloudEvent;
use tower_http::trace::TraceLayer;

fn echo_app() -> Router {
    Router::new()
        .route("/", get(|| async { "Hello from CloudEvents server" }))
        .route("/",  post(handle_event))
        .layer(TraceLayer::new_for_http())
}


async fn handle_event(
        GoogleCloudEvent { event, data }: GoogleCloudEvent<MessagePublishedData>,
) -> impl IntoResponse {
    // debug!("Processing user created event: {:?}", event);
    dbg!(event);
    dbg!(data);
    (StatusCode::OK, "User created event processed".to_string())
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


