#[macro_export]
macro_rules! impl_wasi_2023_11_10 {
    ($wasi_module_path:tt) => {
        mod impl_wasi_traits_2023_11_10 {
            use super::$wasi_module_path as wasi;
            use $crate::wasi::{traits, StreamError};

            impl traits::WasiPollable for wasi::io::poll::Pollable {
                fn handle(&self) -> u32 {
                    self.handle()
                }

                fn ready(&self) -> bool {
                    self.ready()
                }
            }

            impl traits::WasiPoll for wasi::io::poll::Pollable {
                fn poll(pollables: &[&Self]) -> Vec<u32> {
                    wasi::io::poll::poll(pollables)
                }
            }

            impl traits::WasiError for wasi::io::error::Error {
                fn to_debug_string(&self) -> String {
                    self.to_debug_string()
                }
            }

            impl traits::WasiStreamError for wasi::io::streams::StreamError {
                type IoError = wasi::io::error::Error;

                fn into_stream_error(self) -> StreamError<Self::IoError> {
                    match self {
                        Self::LastOperationFailed(err) => StreamError::LastOperationFailed(err),
                        Self::Closed => StreamError::Closed,
                    }
                }
            }

            impl traits::WasiInputStream for wasi::io::streams::InputStream {
                type StreamError = wasi::io::streams::StreamError;

                fn read(&self, len: u64) -> Result<Vec<u8>, Self::StreamError> {
                    let data = self.read(len).map_err(Into::into)?;
                    Ok(data)
                }
            }
            impl traits::WasiSubscribe for wasi::io::streams::InputStream {
                type Pollable = wasi::io::poll::Pollable;

                fn subscribe(&self) -> Self::Pollable {
                    self.subscribe()
                }
            }

            impl traits::WasiOutputStream for wasi::io::streams::OutputStream {
                type InputStream = wasi::io::streams::InputStream;
                type StreamError = wasi::io::streams::StreamError;

                fn check_write(&self) -> Result<u64, Self::StreamError> {
                    self.check_write().map_err(Into::into)
                }

                fn write(&self, contents: &[u8]) -> Result<(), Self::StreamError> {
                    self.write(contents).map_err(Into::into)
                }

                fn splice(
                    &self,
                    src: &Self::InputStream,
                    len: u64,
                ) -> Result<u64, Self::StreamError> {
                    self.splice(src, len)
                }

                fn flush(&self) -> Result<(), Self::StreamError> {
                    self.flush()
                }
            }
            impl traits::WasiSubscribe for wasi::io::streams::OutputStream {
                type Pollable = wasi::io::poll::Pollable;

                fn subscribe(&self) -> Self::Pollable {
                    self.subscribe()
                }
            }

            impl traits::WasiErrorCode for wasi::http::types::ErrorCode {}

            impl traits::WasiMethod for wasi::http::types::Method {
                fn from_method(method: $crate::wasi::Method) -> Self
                where
                    Self: Sized,
                {
                    match method {
                        $crate::wasi::Method::Get => Self::Get,
                        $crate::wasi::Method::Head => Self::Head,
                        $crate::wasi::Method::Post => Self::Post,
                        $crate::wasi::Method::Put => Self::Put,
                        $crate::wasi::Method::Delete => Self::Delete,
                        $crate::wasi::Method::Connect => Self::Connect,
                        $crate::wasi::Method::Options => Self::Options,
                        $crate::wasi::Method::Trace => Self::Trace,
                        $crate::wasi::Method::Patch => Self::Patch,
                        $crate::wasi::Method::Other(other) => Self::Other(other),
                    }
                }

                fn into_method(self) -> $crate::wasi::Method {
                    match self {
                        Self::Get => $crate::wasi::Method::Get,
                        Self::Head => $crate::wasi::Method::Head,
                        Self::Post => $crate::wasi::Method::Post,
                        Self::Put => $crate::wasi::Method::Put,
                        Self::Delete => $crate::wasi::Method::Delete,
                        Self::Connect => $crate::wasi::Method::Connect,
                        Self::Options => $crate::wasi::Method::Options,
                        Self::Trace => $crate::wasi::Method::Trace,
                        Self::Patch => $crate::wasi::Method::Patch,
                        Self::Other(other) => $crate::wasi::Method::Other(other),
                    }
                }
            }

            impl traits::WasiScheme for wasi::http::types::Scheme {
                fn from_scheme(scheme: $crate::wasi::Scheme) -> Self
                where
                    Self: Sized,
                {
                    match scheme {
                        $crate::wasi::Scheme::Http => Self::Http,
                        $crate::wasi::Scheme::Https => Self::Https,
                        $crate::wasi::Scheme::Other(other) => Self::Other(other),
                    }
                }

                fn into_scheme(self) -> $crate::wasi::Scheme {
                    match self {
                        Self::Http => $crate::wasi::Scheme::Http,
                        Self::Https => $crate::wasi::Scheme::Https,
                        Self::Other(other) => $crate::wasi::Scheme::Other(other),
                    }
                }
            }

            impl traits::WasiFields for wasi::http::types::Fields {
                type Error = wasi::http::types::HeaderError;

                fn from_list(entries: &[(String, Vec<u8>)]) -> Result<Self, Self::Error>
                where
                    Self: Sized,
                {
                    Self::from_list(entries)
                }

                fn entries(&self) -> Vec<(String, Vec<u8>)> {
                    self.entries()
                }
            }

            impl traits::WasiIncomingBody for wasi::http::types::IncomingBody {
                type Pollable = wasi::io::poll::Pollable;
                type InputStream = wasi::io::streams::InputStream;
                type FutureTrailers = wasi::http::types::FutureTrailers;

                fn stream(&self) -> Result<Self::InputStream, ()> {
                    self.stream()
                }

                fn finish(self) -> Self::FutureTrailers {
                    Self::finish(self)
                }
            }

            impl traits::WasiFutureTrailers for wasi::http::types::FutureTrailers {
                type Trailers = wasi::http::types::Trailers;
                type ErrorCode = wasi::http::types::ErrorCode;

                fn get(&self) -> Option<Result<Option<Self::Trailers>, Self::ErrorCode>> {
                    self.get()
                }
            }
            impl traits::WasiSubscribe for wasi::http::types::FutureTrailers {
                type Pollable = wasi::io::poll::Pollable;

                fn subscribe(&self) -> Self::Pollable {
                    self.subscribe()
                }
            }

            impl traits::WasiIncomingRequest for wasi::http::types::IncomingRequest {
                type Method = wasi::http::types::Method;
                type Scheme = wasi::http::types::Scheme;
                type Headers = wasi::http::types::Headers;
                type IncomingBody = wasi::http::types::IncomingBody;

                fn method(&self) -> Self::Method {
                    self.method()
                }

                fn path_with_query(&self) -> Option<String> {
                    self.path_with_query()
                }

                fn scheme(&self) -> Option<Self::Scheme> {
                    self.scheme()
                }

                fn authority(&self) -> Option<String> {
                    self.authority()
                }

                fn headers(&self) -> Self::Headers {
                    self.headers()
                }

                fn consume(&self) -> Result<Self::IncomingBody, ()> {
                    self.consume()
                }
            }

            impl traits::WasiIncomingResponse for wasi::http::types::IncomingResponse {
                type Headers = wasi::http::types::Headers;
                type IncomingBody = wasi::http::types::IncomingBody;

                fn status(&self) -> u16 {
                    self.status()
                }

                fn headers(&self) -> Self::Headers {
                    self.headers()
                }

                fn consume(&self) -> Result<Self::IncomingBody, ()> {
                    self.consume()
                }
            }

            impl traits::WasiFutureIncomingResponse for wasi::http::types::FutureIncomingResponse {
                type IncomingResponse = wasi::http::types::IncomingResponse;
                type ErrorCode = wasi::http::types::ErrorCode;

                fn get(
                    &self,
                ) -> Option<Result<Result<Self::IncomingResponse, Self::ErrorCode>, ()>> {
                    self.get()
                }
            }
            impl traits::WasiSubscribe for wasi::http::types::FutureIncomingResponse {
                type Pollable = wasi::io::poll::Pollable;

                fn subscribe(&self) -> Self::Pollable {
                    self.subscribe()
                }
            }

            impl traits::WasiOutgoingBody for wasi::http::types::OutgoingBody {
                type OutputStream = wasi::io::streams::OutputStream;
                type Trailers = wasi::http::types::Trailers;
                type ErrorCode = wasi::http::types::ErrorCode;

                fn write(&self) -> Result<Self::OutputStream, ()> {
                    self.write()
                }

                fn finish(self, trailers: Option<Self::Trailers>) -> Result<(), Self::ErrorCode> {
                    Self::finish(self, trailers)
                }
            }

            impl traits::WasiOutgoingRequest for wasi::http::types::OutgoingRequest {
                type Method = wasi::http::types::Method;
                type Scheme = wasi::http::types::Scheme;
                type Headers = wasi::http::types::Headers;
                type OutgoingBody = wasi::http::types::OutgoingBody;

                fn new(headers: Self::Headers) -> Self
                where
                    Self: Sized,
                {
                    Self::new(headers)
                }

                fn body(&self) -> Result<Self::OutgoingBody, ()> {
                    self.body()
                }

                fn set_method(&self, method: &Self::Method) -> Result<(), ()> {
                    self.set_method(method)
                }

                fn set_path_with_query(&self, path_with_query: Option<&str>) -> Result<(), ()> {
                    self.set_path_with_query(path_with_query)
                }

                fn set_scheme(&self, scheme: Option<&Self::Scheme>) -> Result<(), ()> {
                    self.set_scheme(scheme)
                }

                fn set_authority(&self, authority: Option<&str>) -> Result<(), ()> {
                    self.set_authority(authority)
                }
            }

            impl traits::WasiOutgoingHandler for wasi::http::types::OutgoingRequest {
                type RequestOptions = wasi::http::types::RequestOptions;
                type FutureIncomingResponse = wasi::http::types::FutureIncomingResponse;
                type ErrorCode = wasi::http::types::ErrorCode;

                fn handle(
                    self,
                    options: Option<Self::RequestOptions>,
                ) -> Result<Self::FutureIncomingResponse, Self::ErrorCode> {
                    wasi::http::outgoing_handler::handle(self, options)
                }
            }

            impl traits::WasiOutgoingResponse for wasi::http::types::OutgoingResponse {
                type Headers = wasi::http::types::Headers;
                type OutgoingBody = wasi::http::types::OutgoingBody;

                fn new(headers: Self::Headers) -> Self
                where
                    Self: Sized,
                {
                    Self::new(headers)
                }

                fn set_status_code(&self, status_code: u16) -> Result<(), ()> {
                    self.set_status_code(status_code)
                }

                fn body(&self) -> Result<Self::OutgoingBody, ()> {
                    self.body()
                }
            }

            impl traits::WasiResponseOutparam for wasi::http::types::ResponseOutparam {
                type OutgoingResponse = wasi::http::types::OutgoingResponse;
                type ErrorCode = wasi::http::types::ErrorCode;

                fn set(self, response: Result<Self::OutgoingResponse, &Self::ErrorCode>) {
                    Self::set(self, response)
                }
            }
        }
    };
}

#[cfg(test)]
mod type_check_macro {
    wit_bindgen::generate!({
        world: "test",
    });
    impl_wasi_2023_11_10!(wasi);

    #[allow(unused_imports)]
    use crate::wasi::traits;
}
