use axum::{
    body::Body,
    extract::State,
    http::Request,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use exports::wasi::http::incoming_handler::ResponseOutparam;
use wasi::{http::types::IncomingRequest, io::poll::Pollable};
use wasi_hyperium::{
    hyperium1::{handle_service_call, send_request},
    poll::Poller,
};

wit_bindgen::generate!({
    path: "../../wit",
    world: "incoming",
    exports: {
        "wasi:http/incoming-handler": Guest,
    },
});

wasi_hyperium::impl_wasi_preview2!(wasi);

struct Guest;

impl exports::wasi::http::incoming_handler::Guest for Guest {
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
async fn proxy_example_com(State(poller): State<Poller<Pollable>>) -> impl IntoResponse {
    let req = Request::get("https://example.com")
        .body(Body::empty())
        .unwrap();
    let resp = send_request::<wasi::http::types::OutgoingRequest, _, _>(req, poller).unwrap();
    Body::new(resp.into_body())
}
