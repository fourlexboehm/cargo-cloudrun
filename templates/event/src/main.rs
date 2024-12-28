use axum::{async_trait, extract::FromRequest, response::IntoResponse, routing::{get, post}, Router};
use axum::body::Body;
use axum::extract::Request;
use axum::response::Response;
use bytes::Bytes;
use cloudevents::{Data, Event};
use google_cloudevents::google::events::cloud::pubsub::v1::MessagePublishedData;
use google_cloudevents::google::events::firebase::auth::v1::UserCreatedEvent;
use http::StatusCode;
use prost::Message;
use serde::de::DeserializeOwned;
use tower_http::trace::TraceLayer;
use tracing::{debug, error};
use tracing_subscriber::fmt::layer;

fn echo_app() -> Router {
    Router::new()
        .route("/", get(|| async { "Hello from CloudEvents server" }))
        .route("/", post(|event: ExtractGoogleEvent<MessagePublishedData>| async move {
                        debug!("Received user created event: {:?}", event.0);
                        (StatusCode::OK, "User event processed".to_string())
                    })

        )
        .layer(TraceLayer::new_for_http())
}

// Specific handler example
async fn handle_user_created(
    ExtractGoogleEvent(event): ExtractGoogleEvent<UserCreatedEvent>,
) -> impl IntoResponse {
    debug!("Processing user created event: {:?}", event);
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
pub struct ExtractGoogleEvent<T>(pub T);

#[derive(Debug)]
pub enum GoogleEventError {
    InvalidData(String),
    DecodingError(String),
}

impl IntoResponse for GoogleEventError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            GoogleEventError::InvalidData(msg) => (StatusCode::BAD_REQUEST, msg),
            GoogleEventError::DecodingError(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
        };
        (status, message).into_response()
    }
}

#[async_trait]
impl<S, T> FromRequest<S> for ExtractGoogleEvent<T>
where
    Event: FromRequest<S>,
    S: Send + Sync,
    T: Message + Default + DeserializeOwned + Send + 'static,
{
    type Rejection = GoogleEventError;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let event = Event::from_request(req, state)
            .await
            .map_err(|_| GoogleEventError::InvalidData("Invalid CloudEvent".to_string()))?;

        let google_event = event
            .data()
            .map(|d| d.to_owned())
            .and_then(|data| match data {
                Data::Binary(bytes) => Message::decode(bytes.as_slice())
                    .map_err(|e| error!("Failed to decode binary data: {}", e))
                    .ok(),
                Data::Json(value) => serde_json::from_value(value)
                    .map_err(|e| error!("Failed to decode JSON data: {}", e))
                    .ok(),
                Data::String(str) => {
                    error!("Unexpected string data: {}", str);
                    None
                }
            })
            .ok_or_else(|| {
                GoogleEventError::DecodingError("Failed to decode event data".to_string())
            })?;

        Ok(ExtractGoogleEvent(google_event))
    }
}
