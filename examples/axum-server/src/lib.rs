use axum::{
    body::Body,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use exports::wasi::http::incoming_handler::ResponseOutparam;
use wasi::http::types::IncomingRequest;
use wasi_hyperium::hyperium1::handle_service_call;

wit_bindgen::generate!({
    path: "../../wit",
    world: "incoming",
    exports: {
        "wasi:http/incoming-handler": Guest,
    },
});

wasi_hyperium::impl_wasi_2023_11_10!(wasi);

struct Guest;

impl exports::wasi::http::incoming_handler::Guest for Guest {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let router = Router::new()
            .route("/", get("Hello, WASI"))
            .route("/echo", post(echo));
        handle_service_call(router, request, response_out).unwrap()
    }
}

#[axum::debug_handler]
async fn echo(body: Body) -> impl IntoResponse {
    body
}
