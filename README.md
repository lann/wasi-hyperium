# Hyperium (`http`, `http-body`) for WASI Preview2 HTTP

```rust
wit_bindgen::generate!({
    // World must include wasi:http/outgoing-handler@0.2.0
});

// Implement wrapper traits
wasi_hyperium::impl_wasi_preview2!(wasi);
```

See [axum-server example](examples/axum-server).
