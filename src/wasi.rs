use std::{
    future::{Future, IntoFuture},
    task::{Context, Poll},
};

use wasi::http::types;

use crate::{
    poll::{PollableRegistry, WasiSubscribe},
    Error,
};

struct Subscribable<T, Registry: PollableRegistry> {
    // NOTE: order matters; handle must be dropped before inner
    handle: Option<Registry::RegisteredPollable>,
    inner: T,
    registry: Registry,
}

impl<T, Registry> Subscribable<T, Registry>
where
    T: WasiSubscribe,
    Registry: PollableRegistry,
{
    fn new(inner: T, registry: Registry) -> Self {
        Self {
            handle: None,
            inner,
            registry,
        }
    }

    fn register_subscribe(&mut self, cx: &mut Context) {
        let pollable = self.inner.subscribe();
        self.handle = Some(self.registry.register_pollable(cx, pollable));
    }

    fn maybe_subscribe(&mut self, cx: &mut Context) -> Poll<()> {
        let pollable = self.inner.subscribe();
        if pollable.ready() {
            Poll::Ready(())
        } else {
            self.handle = Some(self.registry.register_pollable(cx, pollable));
            Poll::Pending
        }
    }

    fn registry(&self) -> &Registry {
        &self.registry
    }
}

impl<T, Registry: PollableRegistry> std::ops::Deref for Subscribable<T, Registry> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct InputStream<Registry: PollableRegistry> {
    stream: Subscribable<types::InputStream, Registry>,
}

impl<Registry> InputStream<Registry>
where
    Registry: PollableRegistry,
{
    pub fn new(stream: types::InputStream, registry: Registry) -> Self {
        let stream = Subscribable::new(stream, registry);
        Self { stream }
    }

    pub fn poll_read(&mut self, cx: &mut Context, len: usize) -> Poll<Result<Vec<u8>, Error>> {
        let data = self
            .stream
            .read(len.try_into().unwrap())
            .map_err(Error::wasi_stream_error)?;
        if data.is_empty() {
            self.stream.register_subscribe(cx);
            Poll::Pending
        } else {
            Poll::Ready(Ok(data))
        }
    }

    fn registry(&self) -> &Registry {
        self.stream.registry()
    }
}

pub struct OutputStream<Registry: PollableRegistry> {
    stream: Subscribable<types::OutputStream, Registry>,
}

impl<Registry> OutputStream<Registry>
where
    Registry: PollableRegistry,
{
    pub fn new(stream: types::OutputStream, registry: Registry) -> Self {
        let stream = Subscribable::new(stream, registry);
        Self { stream }
    }

    pub fn poll_check_write(
        &mut self,
        cx: &mut Context,
    ) -> Poll<Result<OutputStreamPermit, Error>> {
        let size = self
            .stream
            .check_write()
            .map_err(Error::wasi_stream_error)?;
        if size == 0 {
            self.stream.register_subscribe(cx);
            Poll::Pending
        } else {
            Poll::Ready(Ok(OutputStreamPermit {
                stream: &self.stream.inner,
                size,
            }))
        }
    }

    pub fn poll_splice(
        &mut self,
        cx: &mut Context,
        src: &InputStream<Registry>,
        len: u64,
    ) -> Poll<Result<u64, Error>> {
        if len == 0 {
            return Poll::Ready(Ok(0));
        }
        let size = self
            .stream
            .splice(&src.stream.inner, len)
            .map_err(Error::wasi_stream_error)?;
        if size == 0 {
            self.stream.register_subscribe(cx);
            Poll::Pending
        } else {
            Poll::Ready(Ok(size))
        }
    }

    pub fn poll_flush(&mut self, cx: &mut Context) -> Poll<Result<(), Error>> {
        self.stream.flush().map_err(Error::wasi_stream_error)?;
        self.stream.maybe_subscribe(cx).map(|()| Ok(()))
    }

    fn registry(&self) -> &Registry {
        self.stream.registry()
    }
}

pub struct OutputStreamPermit<'a> {
    stream: &'a types::OutputStream,
    size: u64,
}

impl OutputStreamPermit<'_> {
    pub fn write(self, contents: &[u8]) -> Result<usize, Error> {
        let len = self
            .size
            .min(contents.len().try_into().unwrap())
            .try_into()
            .unwrap();
        self.stream
            .write(&contents[..len])
            .map_err(Error::wasi_stream_error)?;
        Ok(len)
    }

    pub fn size(&self) -> u64 {
        self.size
    }
}

pub struct IncomingBody<Registry: PollableRegistry> {
    // NOTE: order matters; stream must be dropped before body
    stream: InputStream<Registry>,
    body: types::IncomingBody,
}

impl<Registry> IncomingBody<Registry>
where
    Registry: PollableRegistry,
{
    pub fn new(body: types::IncomingBody, registry: Registry) -> Result<Self, Error> {
        let stream = InputStream::new(
            body.stream()
                .map_err(|()| Error::WasiInvalidState("incoming-body.stream already called"))?,
            registry,
        );
        Ok(Self { stream, body })
    }

    pub fn stream(&mut self) -> &mut InputStream<Registry> {
        &mut self.stream
    }

    pub fn finish(self) -> FutureTrailers<Registry> {
        let Self { stream, body } = self;
        let registry = stream.registry().clone();
        drop(stream);
        let wasi_trailers = types::IncomingBody::finish(body);
        let trailers = Subscribable::new(wasi_trailers, registry);
        FutureTrailers { trailers }
    }
}

pub struct FutureTrailers<Registry: PollableRegistry> {
    trailers: Subscribable<types::FutureTrailers, Registry>,
}

impl<Registry> Future for FutureTrailers<Registry>
where
    Registry: PollableRegistry,
{
    type Output = Result<Option<FieldEntries>, Error>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.trailers.get() {
            Some(Ok(Ok(Some(fields)))) => Poll::Ready(Ok(Some(fields.into()))),
            Some(Ok(Ok(None))) => Poll::Ready(Ok(None)),
            Some(Ok(Err(err))) => Poll::Ready(Err(Error::wasi_error_code(err))),
            Some(Err(())) => Poll::Ready(Err(Error::WasiInvalidState(
                "future-trailers.get already consumed",
            ))),
            None => {
                self.trailers.register_subscribe(cx);
                Poll::Pending
            }
        }
    }
}

#[derive(Debug)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch,
    Other(String),
}

impl From<Method> for types::Method {
    fn from(method: Method) -> Self {
        match method {
            Method::Get => Self::Get,
            Method::Head => Self::Head,
            Method::Post => Self::Post,
            Method::Put => Self::Put,
            Method::Delete => Self::Delete,
            Method::Connect => Self::Connect,
            Method::Options => Self::Options,
            Method::Trace => Self::Trace,
            Method::Patch => Self::Patch,
            Method::Other(other) => Self::Other(other),
        }
    }
}

impl From<types::Method> for Method {
    fn from(method: types::Method) -> Self {
        match method {
            types::Method::Get => Self::Get,
            types::Method::Head => Self::Head,
            types::Method::Post => Self::Post,
            types::Method::Put => Self::Put,
            types::Method::Delete => Self::Delete,
            types::Method::Connect => Self::Connect,
            types::Method::Options => Self::Options,
            types::Method::Trace => Self::Trace,
            types::Method::Patch => Self::Patch,
            types::Method::Other(other) => Self::Other(other),
        }
    }
}

#[derive(Debug)]
pub enum Scheme {
    Http,
    Https,
    Other(String),
}

impl From<Scheme> for types::Scheme {
    fn from(scheme: Scheme) -> Self {
        match scheme {
            Scheme::Http => Self::Http,
            Scheme::Https => Self::Https,
            Scheme::Other(other) => Self::Other(other),
        }
    }
}

impl From<types::Scheme> for Scheme {
    fn from(scheme: types::Scheme) -> Self {
        match scheme {
            types::Scheme::Http => Self::Http,
            types::Scheme::Https => Self::Https,
            types::Scheme::Other(other) => Self::Other(other),
        }
    }
}

pub struct IncomingRequest<Registry: PollableRegistry> {
    request: types::IncomingRequest,
    body: IncomingBody<Registry>,
}

impl<Registry> IncomingRequest<Registry>
where
    Registry: PollableRegistry,
{
    pub fn new(request: types::IncomingRequest, registry: Registry) -> Result<Self, Error> {
        let body = request
            .consume()
            .map_err(|()| Error::WasiInvalidState("incoming-request.consume already called"))?;
        let body = IncomingBody::new(body, registry)?;
        Ok(Self { request, body })
    }

    pub fn method(&self) -> Method {
        self.request.method().into()
    }

    pub fn path_with_query(&self) -> Option<String> {
        self.request.path_with_query()
    }

    pub fn scheme(&self) -> Option<Scheme> {
        self.request.scheme().map(Into::into)
    }

    pub fn authority(&self) -> Option<String> {
        self.request.authority()
    }

    pub fn headers(&self) -> FieldEntries {
        self.request.headers().into()
    }

    pub fn body(&mut self) -> &mut IncomingBody<Registry> {
        &mut self.body
    }

    pub fn into_body(self) -> IncomingBody<Registry> {
        self.body
    }
}

pub struct IncomingResponse<Registry: PollableRegistry> {
    response: types::IncomingResponse,
    body: IncomingBody<Registry>,
}

impl<Registry> IncomingResponse<Registry>
where
    Registry: PollableRegistry,
{
    pub fn new(response: types::IncomingResponse, registry: Registry) -> Result<Self, Error> {
        let body = response
            .consume()
            .map_err(|()| Error::WasiInvalidState("incoming-response.consume already called"))?;
        let body = IncomingBody::new(body, registry)?;
        Ok(Self { response, body })
    }

    pub fn status(&self) -> u16 {
        self.response.status()
    }

    pub fn headers(&self) -> FieldEntries {
        self.response.headers().into()
    }

    pub fn body(&mut self) -> &mut IncomingBody<Registry> {
        &mut self.body
    }

    pub fn into_body(self) -> IncomingBody<Registry> {
        self.body
    }
}

pub struct OutgoingBody<Registry: PollableRegistry> {
    // NOTE: order matters; stream must be dropped before body
    stream: OutputStream<Registry>,
    body: types::OutgoingBody,
}

impl<Registry> OutgoingBody<Registry>
where
    Registry: PollableRegistry,
{
    pub fn new(body: types::OutgoingBody, registry: Registry) -> Result<Self, Error> {
        let stream = OutputStream::new(
            body.write()
                .map_err(|()| Error::WasiInvalidState("outgoing-body.write already called"))?,
            registry,
        );
        Ok(Self { stream, body })
    }

    pub fn stream(&mut self) -> &mut OutputStream<Registry> {
        &mut self.stream
    }

    pub fn finish(self, trailers: Option<FieldEntries>) -> Result<(), Error> {
        let trailers = match trailers {
            Some(trailers) => Some(trailers.try_into_fields()?),
            None => None,
        };
        drop(self.stream);
        types::OutgoingBody::finish(self.body, trailers).map_err(Error::wasi_error_code)
    }

    fn registry(&self) -> &Registry {
        self.stream.registry()
    }
}

pub struct OutgoingRequest<Registry: PollableRegistry> {
    request: types::OutgoingRequest,
    body: OutgoingBody<Registry>,
}

impl<Registry> OutgoingRequest<Registry>
where
    Registry: PollableRegistry,
{
    pub fn new(request: types::OutgoingRequest, registry: Registry) -> Result<Self, Error> {
        let body = request
            .body()
            .map_err(|()| Error::WasiInvalidState("outgoing-request.body already called"))?;
        let body = OutgoingBody::new(body, registry)?;
        Ok(Self { request, body })
    }

    pub fn from_headers(headers: &FieldEntries, registry: Registry) -> Result<Self, Error> {
        let fields = headers.try_into_fields()?;
        let response = types::OutgoingRequest::new(fields);
        Self::new(response, registry)
    }

    pub fn set_method(&mut self, method: Method) -> Result<(), Error> {
        self.request
            .set_method(&method.into())
            .map_err(|()| Error::WasiInvalidValue("invalid method"))
    }

    pub fn set_path_with_query(&mut self, path_with_query: Option<&str>) -> Result<(), Error> {
        self.request
            .set_path_with_query(path_with_query)
            .map_err(|()| Error::WasiInvalidValue("invalid path_with_query"))
    }

    pub fn set_scheme(&mut self, scheme: Option<Scheme>) -> Result<(), Error> {
        self.request
            .set_scheme(scheme.map(|scheme| scheme.into()).as_ref())
            .map_err(|()| Error::WasiInvalidValue("invalid scheme"))
    }

    pub fn set_authority(&mut self, authority: Option<&str>) -> Result<(), Error> {
        self.request
            .set_authority(authority)
            .map_err(|()| Error::WasiInvalidValue("invalid authority"))
    }
}

impl<Registry> OutgoingRequest<Registry>
where
    Registry: PollableRegistry,
{
    pub fn send(
        self,
        options: Option<types::RequestOptions>,
    ) -> Result<ActiveOutgoingRequest<Registry>, Error> {
        let Self { request, body } = self;
        let response = wasi::http::outgoing_handler::handle(request, options)
            .map_err(Error::wasi_error_code)?;
        let inner = Subscribable::new(response, body.registry().clone());
        let future_response = FutureIncomingResponse { inner };
        Ok(ActiveOutgoingRequest {
            body,
            future_response,
        })
    }
}

pub struct ActiveOutgoingRequest<Registry>
where
    Registry: PollableRegistry,
{
    body: OutgoingBody<Registry>,
    future_response: FutureIncomingResponse<Registry>,
}

impl<Registry> ActiveOutgoingRequest<Registry>
where
    Registry: PollableRegistry,
{
    pub fn body(&mut self) -> &mut OutgoingBody<Registry> {
        &mut self.body
    }

    pub fn into_parts(self) -> (OutgoingBody<Registry>, FutureIncomingResponse<Registry>) {
        (self.body, self.future_response)
    }
}

impl<Registry> IntoFuture for ActiveOutgoingRequest<Registry>
where
    Registry: PollableRegistry,
{
    type Output = Result<IncomingResponse<Registry>, Error>;
    type IntoFuture = FutureIncomingResponse<Registry>;

    fn into_future(self) -> Self::IntoFuture {
        self.future_response
    }
}

pub struct FutureIncomingResponse<Registry: PollableRegistry> {
    inner: Subscribable<types::FutureIncomingResponse, Registry>,
}

impl<Registry> Future for FutureIncomingResponse<Registry>
where
    Registry: PollableRegistry,
{
    type Output = Result<IncomingResponse<Registry>, Error>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner.get() {
            Some(Ok(res)) => {
                let response = res.map_err(Error::wasi_error_code)?;
                // FIXME: figure out proper type contraints to avoid this
                let registry = self.inner.registry().clone();
                Poll::Ready(Ok(IncomingResponse::new(response, registry.clone())?))
            }
            Some(Err(())) => Poll::Ready(Err(Error::WasiInvalidState(
                "FutureIncomingResponse polled after completion",
            ))),
            None => {
                self.inner.register_subscribe(cx);
                Poll::Pending
            }
        }
    }
}

pub struct OutgoingResponse<Registry: PollableRegistry> {
    response: types::OutgoingResponse,
    body: OutgoingBody<Registry>,
}

impl<Registry> OutgoingResponse<Registry>
where
    Registry: PollableRegistry,
{
    pub fn new(response: types::OutgoingResponse, registry: Registry) -> Result<Self, Error> {
        let body = response
            .body()
            .map_err(|()| Error::WasiInvalidState("outgoing-response.body already called"))?;
        let body = OutgoingBody::new(body, registry)?;
        Ok(Self { response, body })
    }

    pub fn from_headers(headers: &FieldEntries, registry: Registry) -> Result<Self, Error> {
        let fields = headers.try_into_fields()?;
        let response = types::OutgoingResponse::new(fields);
        Self::new(response, registry)
    }

    pub fn set_status_code(&mut self, status_code: u16) -> Result<(), Error> {
        self.response
            .set_status_code(status_code)
            .map_err(|()| Error::WasiInvalidValue("invalid status code"))
    }

    pub fn body(&mut self) -> &mut OutgoingBody<Registry> {
        &mut self.body
    }

    pub fn into_body(self) -> OutgoingBody<Registry> {
        self.body
    }

    fn into_parts(self) -> (types::OutgoingResponse, OutgoingBody<Registry>) {
        (self.response, self.body)
    }
}

pub struct ResponseOutparam {
    outparam: types::ResponseOutparam,
}

impl ResponseOutparam {
    pub fn new(outparam: types::ResponseOutparam) -> Self {
        Self { outparam }
    }

    pub fn set_response<Registry>(
        self,
        response: OutgoingResponse<Registry>,
    ) -> OutgoingBody<Registry>
    where
        Registry: PollableRegistry,
    {
        let (wasi_response, body) = response.into_parts();
        types::ResponseOutparam::set(self.outparam, Ok(wasi_response));
        body
    }

    pub fn set_error(self, err: types::ErrorCode) {
        types::ResponseOutparam::set(self.outparam, Err(err));
    }
}

#[derive(Debug)]
pub struct FieldEntries(Vec<(String, Vec<u8>)>);

impl FieldEntries {
    pub fn try_into_fields(&self) -> Result<types::Fields, Error> {
        types::Fields::from_list(&self.0).map_err(|err| Error::WasiFieldsError(err.to_string()))
    }
}

impl From<Vec<(String, Vec<u8>)>> for FieldEntries {
    fn from(value: Vec<(String, Vec<u8>)>) -> Self {
        Self(value)
    }
}

impl IntoIterator for FieldEntries {
    type Item = (String, Vec<u8>);

    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<types::Fields> for FieldEntries {
    fn from(fields: types::Fields) -> Self {
        Self(fields.entries())
    }
}
