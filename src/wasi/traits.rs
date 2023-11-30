#![allow(clippy::result_unit_err, clippy::type_complexity)]

use super::Method;
use super::Scheme;
use super::StreamError;

pub trait WasiPollable: Unpin {
    fn handle(&self) -> u32;
    fn ready(&self) -> bool;
}

pub trait WasiPoll: WasiPollable {
    fn poll(pollables: &[&Self]) -> Vec<u32>;
}

pub trait WasiSubscribe: Unpin {
    type Pollable: WasiPollable;

    fn subscribe(&self) -> Self::Pollable;
}

pub trait WasiError: Unpin {
    fn to_debug_string(&self) -> String;
}

pub trait WasiStreamError: Unpin {
    type IoError: WasiError;

    fn into_stream_error(self) -> StreamError<Self::IoError>;
}

pub trait WasiInputStream: WasiSubscribe {
    type StreamError: WasiStreamError;

    fn read(&self, len: u64) -> Result<Vec<u8>, Self::StreamError>;
}

pub trait WasiOutputStream: WasiSubscribe {
    type InputStream: WasiInputStream;
    type StreamError: WasiStreamError;

    fn check_write(&self) -> Result<u64, Self::StreamError>;
    fn write(&self, contents: &[u8]) -> Result<(), Self::StreamError>;
    fn splice(&self, src: &Self::InputStream, len: u64) -> Result<u64, Self::StreamError>;
    fn flush(&self) -> Result<(), Self::StreamError>;
}

pub trait WasiErrorCode: std::error::Error + Unpin {}

pub trait WasiMethod: Unpin {
    fn from_method(method: Method) -> Self
    where
        Self: Sized;

    fn into_method(self) -> Method;
}

pub trait WasiScheme: Unpin {
    fn from_scheme(scheme: Scheme) -> Self
    where
        Self: Sized;

    fn into_scheme(self) -> Scheme;
}

pub trait WasiFields: Sized {
    type Error: std::error::Error + Unpin;

    fn from_list(entries: &[(String, Vec<u8>)]) -> Result<Self, Self::Error>;
    fn entries(&self) -> Vec<(String, Vec<u8>)>;
}

pub trait WasiIncomingBody: Unpin {
    type Pollable: WasiPollable;
    type InputStream: WasiInputStream<Pollable = Self::Pollable>;
    type FutureTrailers: WasiFutureTrailers<Pollable = Self::Pollable>;

    fn stream(&self) -> Result<Self::InputStream, ()>;
    fn finish(self) -> Self::FutureTrailers;
}

pub trait WasiFutureTrailers: WasiSubscribe {
    type Trailers: WasiFields;
    type ErrorCode: WasiErrorCode;

    fn get(&self) -> Option<Result<Option<Self::Trailers>, Self::ErrorCode>>;
}

pub trait WasiIncomingRequest: Unpin {
    type Method: WasiMethod;
    type Scheme: WasiScheme;
    type Headers: WasiFields;
    type IncomingBody: WasiIncomingBody;

    fn method(&self) -> Self::Method;
    fn path_with_query(&self) -> Option<String>;
    fn scheme(&self) -> Option<Self::Scheme>;
    fn authority(&self) -> Option<String>;
    fn headers(&self) -> Self::Headers;
    fn consume(&self) -> Result<Self::IncomingBody, ()>;
}

pub trait WasiIncomingResponse: Unpin {
    type Headers: WasiFields;
    type IncomingBody: WasiIncomingBody;

    fn status(&self) -> u16;
    fn headers(&self) -> Self::Headers;
    fn consume(&self) -> Result<Self::IncomingBody, ()>;
}

pub trait WasiFutureIncomingResponse: WasiSubscribe {
    type IncomingResponse: WasiIncomingResponse;
    type ErrorCode: WasiErrorCode;

    fn get(&self) -> Option<Result<Result<Self::IncomingResponse, Self::ErrorCode>, ()>>;
}

pub trait WasiOutgoingBody: Unpin {
    type OutputStream: WasiOutputStream;
    type Trailers: WasiFields;
    type ErrorCode: WasiErrorCode;

    fn write(&self) -> Result<Self::OutputStream, ()>;
    fn finish(self, trailers: Option<Self::Trailers>) -> Result<(), Self::ErrorCode>;
}

pub trait WasiOutgoingRequest: Unpin {
    type Method: WasiMethod;
    type Scheme: WasiScheme;
    type Headers: WasiFields;
    type OutgoingBody: WasiOutgoingBody;

    fn new(headers: Self::Headers) -> Self
    where
        Self: Sized;
    fn body(&self) -> Result<Self::OutgoingBody, ()>;
    fn set_method(&self, method: &Self::Method) -> Result<(), ()>;
    fn set_path_with_query(&self, path_with_query: Option<&str>) -> Result<(), ()>;
    fn set_scheme(&self, scheme: Option<&Self::Scheme>) -> Result<(), ()>;
    fn set_authority(&self, authority: Option<&str>) -> Result<(), ()>;
}

pub trait WasiOutgoingResponse: Unpin {
    type Headers: WasiFields;
    type OutgoingBody: WasiOutgoingBody;

    fn new(headers: Self::Headers) -> Self
    where
        Self: Sized;
    fn set_status_code(&self, status_code: u16) -> Result<(), ()>;
    fn body(&self) -> Result<Self::OutgoingBody, ()>;
}

pub trait WasiResponseOutparam {
    type OutgoingResponse: WasiOutgoingResponse;
    type ErrorCode: WasiErrorCode;

    fn set(self, response: Result<Self::OutgoingResponse, &Self::ErrorCode>);
}

pub trait WasiOutgoingHandler: WasiOutgoingRequest + Sized {
    type RequestOptions;
    type FutureIncomingResponse: WasiFutureIncomingResponse;
    type ErrorCode: WasiErrorCode;

    fn handle(
        self,
        options: Option<Self::RequestOptions>,
    ) -> Result<Self::FutureIncomingResponse, Self::ErrorCode>;
}
