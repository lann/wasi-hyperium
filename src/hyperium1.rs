mod incoming;
mod outgoing;
mod send;
mod service;

pub use incoming::{incoming_request, incoming_response};
pub use outgoing::{outgoing_request, outgoing_response, Hyperium1OutgoingBodyCopier};
pub use send::send_request;
pub use service::handle_service_call;

use crate::wasi::{FieldEntries, Method, Scheme};

impl TryFrom<Method> for http1::Method {
    type Error = http1::Error;

    fn try_from(method: Method) -> Result<Self, Self::Error> {
        Ok(match method {
            Method::Get => Self::GET,
            Method::Head => Self::HEAD,
            Method::Post => Self::POST,
            Method::Put => Self::PUT,
            Method::Delete => Self::DELETE,
            Method::Connect => Self::CONNECT,
            Method::Options => Self::OPTIONS,
            Method::Trace => Self::TRACE,
            Method::Patch => Self::PATCH,
            Method::Other(other) => other.parse()?,
        })
    }
}

impl From<&http1::Method> for Method {
    fn from(method: &http1::Method) -> Self {
        match method {
            &http1::Method::GET => Self::Get,
            &http1::Method::HEAD => Self::Head,
            &http1::Method::POST => Self::Post,
            &http1::Method::PUT => Self::Put,
            &http1::Method::DELETE => Self::Delete,
            &http1::Method::CONNECT => Self::Connect,
            &http1::Method::OPTIONS => Self::Options,
            &http1::Method::TRACE => Self::Trace,
            &http1::Method::PATCH => Self::Patch,
            other => Self::Other(other.to_string()),
        }
    }
}

impl TryFrom<Scheme> for http1::uri::Scheme {
    type Error = http1::Error;

    fn try_from(scheme: Scheme) -> Result<Self, Self::Error> {
        Ok(match scheme {
            Scheme::Http => Self::HTTP,
            Scheme::Https => Self::HTTPS,
            Scheme::Other(other) => other.parse()?,
        })
    }
}

impl From<&http1::uri::Scheme> for Scheme {
    fn from(scheme: &http1::uri::Scheme) -> Self {
        if scheme == &http1::uri::Scheme::HTTP {
            Self::Http
        } else if scheme == &http1::uri::Scheme::HTTPS {
            Self::Https
        } else {
            Self::Other(scheme.to_string())
        }
    }
}

impl TryFrom<FieldEntries> for http1::HeaderMap {
    type Error = http1::Error;

    fn try_from(entries: FieldEntries) -> Result<Self, Self::Error> {
        entries
            .into_iter()
            .map(|(name, val)| Ok((name.try_into()?, val.try_into()?)))
            .collect()
    }
}

impl From<http1::HeaderMap> for FieldEntries {
    fn from(map: http1::HeaderMap) -> Self {
        (&map).into()
    }
}

impl From<&http1::HeaderMap> for FieldEntries {
    fn from(map: &http1::HeaderMap) -> Self {
        map.iter()
            .map(|(name, val)| (name.to_string(), val.as_bytes().to_vec()))
            .collect::<Vec<_>>()
            .into()
    }
}
