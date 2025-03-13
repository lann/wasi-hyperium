# Hyperium (`http`, `http-body`) for WASI Preview2 HTTP

```rust
struct Guest;

impl ::wasi::exports::http::incoming_handler::Guest for Guest {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        let poller = Poller::default();
        let svc: tower_service::Service</* TODO DOCUMENT */> = ...;
        wasi_hyperium::hyperium1::handle_service_call(svc, request, response_out, poller).unwrap()
    }
}
```

See [axum-server example](examples/axum-server).
