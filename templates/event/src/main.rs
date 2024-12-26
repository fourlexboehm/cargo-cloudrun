use axum::{routing::{get, post}, Json, Router};
use axum::response::IntoResponse;
use axum_extra::protobuf::Protobuf;
use google_cloudevents::google::events::cloud::firestore::v1::DocumentCreatedEvent;
use http::StatusCode;
#[tokio::main]
async fn main() {
    // build our application with a route
    let app = Router::new()
        .route("/", get(|| async { "hello from cloudevents server" }))
        .route(
            "/", post(handle_event)
        );
    // run it
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

// Example handler for ApiCreatedEvent
pub async fn handle_event(
    Protobuf(event): Protobuf<DocumentCreatedEvent>,
) -> impl IntoResponse {
    // Process the event as needed
    // let event = event.data.unwrap().value.unwrap().name;
    // Respond with a simple acknowledgment
    // Optionally, you can send a Protobuf-encoded response
    (StatusCode::OK, "Event received")
}
