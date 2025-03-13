mod incoming;
pub mod outgoing;
pub mod poll;
pub mod wasi;

use ::wasi::{http::types::ErrorCode, io::streams::StreamError};
pub use incoming::IncomingHttpBody;

#[cfg(feature = "hyperium0")]
pub mod hyperium0;
#[cfg(feature = "hyperium1")]
pub mod hyperium1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    BodyError(anyhow::Error),

    #[error("{0}")]
    WasiError(String),
    #[error("{0}")]
    WasiErrorCode(String),
    #[error("{0}")]
    WasiFieldsError(String),
    #[error("{0}")]
    WasiInvalidState(&'static str),
    #[error("{0}")]
    WasiInvalidValue(&'static str),
    #[error("stream error: {0}")]
    WasiStreamOperationFailed(String),
    #[error("stream closed")]
    WasiStreamClosed,

    #[cfg(feature = "hyperium0")]
    #[error(transparent)]
    Hyperium0Error(#[from] http0::Error),
    #[cfg(feature = "hyperium1")]
    #[error(transparent)]
    Hyperium1Error(#[from] http1::Error),
}

impl Error {
    fn wasi_error_code(err: ErrorCode) -> Self {
        Self::WasiErrorCode(err.to_string())
    }

    fn wasi_stream_error(err: StreamError) -> Self {
        match err {
            StreamError::LastOperationFailed(err) => {
                Self::WasiStreamOperationFailed(err.to_debug_string())
            }
            StreamError::Closed => Self::WasiStreamClosed,
        }
    }
}
