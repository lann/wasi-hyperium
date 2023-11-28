# Hyperium (`http`, `http-body`) for WASI Preview2 HTTP

```rust
wit_bindgen::generate!({
    // World must include wasi:http/outgoing-handler@0.2.0-rc-2023-11-10
});

// Implement wrapper traits
wasi_hyperium::impl_wasi_2023_11_10!(wasi);
```

See [axum-server example](examples/axum-server).
