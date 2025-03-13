use axum::{
    body::Body,
    extract::State,
    http::Request,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use wasi::http::types::{IncomingRequest, ResponseOutparam};
use wasi_hyperium::{
    hyperium1::{handle_service_call, send_outbound_request},
    poll::Poller,
};

struct Guest;

wasi::http::proxy::export!(Guest);

impl wasi::exports::http::incoming_handler::Guest for Guest {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let poller = Poller::default();
        let router = Router::new()
            .route("/", get("Hello, WASI"))
            .route("/echo", post(echo))
            .route("/proxy", get(proxy_example_com))
            .with_state(poller.clone());
        handle_service_call(router, request, response_out, poller).unwrap()
    }
}

#[axum::debug_handler]
async fn echo(body: Body) -> impl IntoResponse {
    body
}

#[axum::debug_handler]
async fn proxy_example_com(State(poller): State<Poller>) -> impl IntoResponse {
    let req = Request::get("https://example.com")
        .body(Body::empty())
        .unwrap();
    let resp = send_outbound_request(req, poller).await.unwrap();
    Body::new(resp.into_body())
}
