use axum::{
    body::HttpBody,
    extract::RawBody,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use exports::wasi::http::incoming_handler::ResponseOutparam;
use wasi::{
    http::types::{IncomingBody, IncomingRequest},
    io::poll::Pollable,
};
use wasi_hyperium::{hyperium0::handle_service_call, poll::Poller};

wit_bindgen::generate!({
    path: "../../wit",
    world: "incoming",
    exports: {
        "wasi:http/incoming-handler": Guest,
    },
    ownership: Borrowing{ duplicate_if_necessary: false }
});

wasi_hyperium::impl_wasi_2023_11_10!(wasi);

type IncomingHttpBody = wasi_hyperium::IncomingHttpBody<IncomingBody, Poller<Pollable>>;

struct Guest;

impl exports::wasi::http::incoming_handler::Guest for Guest {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let router = Router::new()
            .route("/", get("Hello, WASI"))
            .route("/echo", post(echo));
        handle_service_call(router, request, response_out).unwrap()
    }
}

#[axum::debug_handler(body = IncomingHttpBody)]
async fn echo(RawBody(body): RawBody<IncomingHttpBody>) -> impl IntoResponse {
    body.map_data(|frame| frame.into_inner().into()).boxed()
}
