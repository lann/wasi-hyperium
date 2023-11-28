use std::{
    future::{Future, IntoFuture},
    task::{Context, Poll},
};

use crate::{poll::PollableRegistry, Error};

use self::traits::{
    WasiFields, WasiFutureIncomingResponse, WasiFutureTrailers, WasiIncomingBody,
    WasiIncomingRequest, WasiIncomingResponse, WasiInputStream, WasiMethod, WasiOutgoingBody,
    WasiOutgoingHandler, WasiOutgoingRequest, WasiOutgoingResponse, WasiOutputStream,
    WasiResponseOutparam, WasiScheme, WasiSubscribe,
};

mod impl_2023_11_10;
pub mod traits;

pub type PollableOf<Subscribe> = <Subscribe as WasiSubscribe>::Pollable;

struct Subscribable<T, Registry: PollableRegistry> {
    // NOTE: order matters; handle must be dropped before inner
    handle: Option<Registry::RegisteredPollable>,
    inner: T,
    registry: Registry,
}

impl<T, Registry> Subscribable<T, Registry>
where
    T: WasiSubscribe,
    Registry: PollableRegistry<Pollable = T::Pollable>,
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

pub struct InputStream<Stream, Registry: PollableRegistry> {
    stream: Subscribable<Stream, Registry>,
}

impl<Stream, Registry> InputStream<Stream, Registry>
where
    Stream: WasiInputStream,
    Registry: PollableRegistry<Pollable = Stream::Pollable>,
{
    pub fn new(stream: Stream, registry: Registry) -> Self {
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

pub struct OutputStream<Stream, Registry: PollableRegistry> {
    stream: Subscribable<Stream, Registry>,
}

impl<Stream, Registry> OutputStream<Stream, Registry>
where
    Stream: WasiOutputStream,
    Registry: PollableRegistry<Pollable = Stream::Pollable>,
{
    pub fn new(stream: Stream, registry: Registry) -> Self {
        let stream = Subscribable::new(stream, registry);
        Self { stream }
    }

    pub fn poll_check_write(
        &mut self,
        cx: &mut Context,
    ) -> Poll<Result<OutputStreamPermit<Stream>, Error>> {
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
        src: &InputStream<Stream::InputStream, Registry>,
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

    fn registry(&self) -> &Registry {
        self.stream.registry()
    }
}

pub struct OutputStreamPermit<'a, Stream> {
    stream: &'a Stream,
    size: u64,
}

impl<'a, Stream: WasiOutputStream> OutputStreamPermit<'a, Stream> {
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

pub struct IncomingBody<Body: WasiIncomingBody, Registry: PollableRegistry> {
    // NOTE: order matters; stream must be dropped before body
    stream: InputStream<Body::InputStream, Registry>,
    body: Body,
}

impl<Body, Registry> IncomingBody<Body, Registry>
where
    Body: WasiIncomingBody,
    Registry: PollableRegistry<Pollable = PollableOf<Body::InputStream>>,
{
    pub fn new(body: Body, registry: Registry) -> Result<Self, Error> {
        let stream = InputStream::new(
            body.stream()
                .map_err(|()| Error::WasiInvalidState("incoming-body.stream already called"))?,
            registry,
        );
        Ok(Self { stream, body })
    }

    pub fn stream(&mut self) -> &mut InputStream<Body::InputStream, Registry> {
        &mut self.stream
    }

    pub fn finish(self) -> FutureTrailers<Body::FutureTrailers, Registry> {
        let Self { stream, body } = self;
        let registry = stream.registry().clone();
        let trailers = Subscribable::new(body.finish(), registry);
        FutureTrailers { trailers }
    }
}

pub struct FutureTrailers<Trailers, Registry: PollableRegistry> {
    trailers: Subscribable<Trailers, Registry>,
}

impl<Trailers, Registry> Future for FutureTrailers<Trailers, Registry>
where
    Trailers: WasiFutureTrailers,
    Registry: PollableRegistry<Pollable = Trailers::Pollable>,
{
    type Output = Result<Option<FieldEntries>, Error>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.trailers.get() {
            Some(Ok(Some(fields))) => Poll::Ready(Ok(Some(fields.into()))),
            Some(Ok(None)) => Poll::Ready(Ok(None)),
            Some(Err(err)) => Poll::Ready(Err(Error::wasi_error_code(err))),
            None => {
                self.trailers.register_subscribe(cx);
                Poll::Pending
            }
        }
    }
}

pub struct IncomingRequest<Request: WasiIncomingRequest, Registry: PollableRegistry> {
    request: Request,
    body: IncomingBody<Request::IncomingBody, Registry>,
}

pub type IncomingRequestPollable<Request> =
    PollableOf<<<Request as WasiIncomingRequest>::IncomingBody as WasiIncomingBody>::InputStream>;

impl<Request, Registry> IncomingRequest<Request, Registry>
where
    Request: WasiIncomingRequest,
    Registry: PollableRegistry<Pollable = IncomingRequestPollable<Request>>,
{
    pub fn new(request: Request, registry: Registry) -> Result<Self, Error> {
        let body = request
            .consume()
            .map_err(|()| Error::WasiInvalidState("incoming-request.consume already called"))?;
        let body = IncomingBody::new(body, registry)?;
        Ok(Self { request, body })
    }

    pub fn method(&self) -> Method {
        self.request.method().into_method()
    }

    pub fn path_with_query(&self) -> Option<String> {
        self.request.path_with_query()
    }

    pub fn scheme(&self) -> Option<Scheme> {
        self.request.scheme().map(|scheme| scheme.into_scheme())
    }

    pub fn authority(&self) -> Option<String> {
        self.request.authority()
    }

    pub fn headers(&self) -> FieldEntries {
        self.request.headers().into()
    }

    pub fn body(&mut self) -> &mut IncomingBody<Request::IncomingBody, Registry> {
        &mut self.body
    }

    pub fn into_body(self) -> IncomingBody<Request::IncomingBody, Registry> {
        self.body
    }
}

pub struct IncomingResponse<Response: WasiIncomingResponse, Registry: PollableRegistry> {
    response: Response,
    body: IncomingBody<Response::IncomingBody, Registry>,
}

pub type IncomingResponsePollable<Response> =
    PollableOf<<<Response as WasiIncomingResponse>::IncomingBody as WasiIncomingBody>::InputStream>;

impl<Response, Registry> IncomingResponse<Response, Registry>
where
    Response: WasiIncomingResponse,
    Registry: PollableRegistry<Pollable = IncomingResponsePollable<Response>>,
{
    pub fn new(response: Response, registry: Registry) -> Result<Self, Error> {
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

    pub fn body(&mut self) -> &mut IncomingBody<Response::IncomingBody, Registry> {
        &mut self.body
    }

    pub fn into_body(self) -> IncomingBody<Response::IncomingBody, Registry> {
        self.body
    }
}

pub struct OutgoingBody<Body: WasiOutgoingBody, Registry: PollableRegistry> {
    // NOTE: order matters; stream must be dropped before body
    stream: OutputStream<Body::OutputStream, Registry>,
    body: Body,
}

impl<Body, Registry> OutgoingBody<Body, Registry>
where
    Body: WasiOutgoingBody,
    Registry: PollableRegistry<Pollable = PollableOf<Body::OutputStream>>,
{
    pub fn new(body: Body, registry: Registry) -> Result<Self, Error> {
        let stream = OutputStream::new(
            body.write()
                .map_err(|()| Error::WasiInvalidState("outgoing-body.write already called"))?,
            registry,
        );
        Ok(Self { stream, body })
    }

    pub fn stream(&mut self) -> &mut OutputStream<Body::OutputStream, Registry> {
        &mut self.stream
    }

    pub fn finish(self, trailers: Option<FieldEntries>) -> Result<(), Error> {
        let trailers: Option<Body::Trailers> = match trailers {
            Some(trailers) => Some(trailers.try_into_fields()?),
            None => None,
        };
        drop(self.stream);
        self.body.finish(trailers).map_err(Error::wasi_error_code)
    }

    fn registry(&self) -> &Registry {
        self.stream.registry()
    }
}

pub struct OutgoingRequest<Request: WasiOutgoingRequest, Registry: PollableRegistry> {
    request: Request,
    body: OutgoingBody<Request::OutgoingBody, Registry>,
}

pub type OutgoingRequestPollable<Request> =
    PollableOf<<<Request as WasiOutgoingRequest>::OutgoingBody as WasiOutgoingBody>::OutputStream>;

impl<Request, Registry> OutgoingRequest<Request, Registry>
where
    Request: WasiOutgoingRequest,
    Registry: PollableRegistry<Pollable = OutgoingRequestPollable<Request>>,
{
    pub fn new(request: Request, registry: Registry) -> Result<Self, Error> {
        let body = request
            .body()
            .map_err(|()| Error::WasiInvalidState("outgoing-request.body already called"))?;
        let body = OutgoingBody::new(body, registry)?;
        Ok(Self { request, body })
    }

    pub fn from_headers(headers: &FieldEntries, registry: Registry) -> Result<Self, Error> {
        let fields = headers.try_into_fields()?;
        let response = Request::new(fields);
        Self::new(response, registry)
    }

    pub fn set_method(&mut self, method: Method) -> Result<(), Error> {
        self.request
            .set_method(&Request::Method::from_method(method))
            .map_err(|()| Error::WasiInvalidValue("invalid method"))
    }

    pub fn set_path_with_query(&mut self, path_with_query: Option<&str>) -> Result<(), Error> {
        self.request
            .set_path_with_query(path_with_query)
            .map_err(|()| Error::WasiInvalidValue("invalid path_with_query"))
    }

    pub fn set_scheme(&mut self, scheme: Option<Scheme>) -> Result<(), Error> {
        let scheme = scheme.map(Request::Scheme::from_scheme);
        self.request
            .set_scheme(scheme.as_ref())
            .map_err(|()| Error::WasiInvalidValue("invalid scheme"))
    }

    pub fn set_authority(&mut self, authority: Option<&str>) -> Result<(), Error> {
        self.request
            .set_authority(authority)
            .map_err(|()| Error::WasiInvalidValue("invalid authority"))
    }
}

impl<Request, Registry> OutgoingRequest<Request, Registry>
where
    Request: WasiOutgoingHandler,
    Request::FutureIncomingResponse:
        WasiFutureIncomingResponse<Pollable = OutgoingRequestPollable<Request>>,
    Registry: PollableRegistry<Pollable = OutgoingRequestPollable<Request>>,
{
    pub fn send(
        self,
        options: Option<Request::RequestOptions>,
    ) -> Result<
        ActiveOutgoingRequest<Request::OutgoingBody, Request::FutureIncomingResponse, Registry>,
        Error,
    > {
        let Self { request, body } = self;
        let response = request.handle(options).map_err(Error::wasi_error_code)?;
        let inner = Subscribable::new(response, body.registry().clone());
        let future_response = FutureIncomingResponse { inner };
        Ok(ActiveOutgoingRequest {
            body,
            future_response,
        })
    }
}

pub struct ActiveOutgoingRequest<Body, FutureResponse, Registry>
where
    Body: WasiOutgoingBody,
    FutureResponse: WasiFutureIncomingResponse<Pollable = PollableOf<Body::OutputStream>>,
    Registry: PollableRegistry<Pollable = PollableOf<Body::OutputStream>>,
{
    body: OutgoingBody<Body, Registry>,
    future_response: FutureIncomingResponse<FutureResponse, Registry>,
}

impl<Body, FutureResponse, Registry> ActiveOutgoingRequest<Body, FutureResponse, Registry>
where
    Body: WasiOutgoingBody,
    FutureResponse: WasiFutureIncomingResponse<Pollable = PollableOf<Body::OutputStream>>,
    Registry: PollableRegistry<Pollable = PollableOf<Body::OutputStream>>,
{
    pub fn body(&mut self) -> &mut OutgoingBody<Body, Registry> {
        &mut self.body
    }

    pub fn into_parts(
        self,
    ) -> (
        OutgoingBody<Body, Registry>,
        FutureIncomingResponse<FutureResponse, Registry>,
    ) {
        (self.body, self.future_response)
    }
}

impl<OutgoingBody, IncomingBody, FutureResponse, Registry> IntoFuture
    for ActiveOutgoingRequest<OutgoingBody, FutureResponse, Registry>
where
    OutgoingBody: WasiOutgoingBody,
    FutureResponse: WasiFutureIncomingResponse<Pollable = PollableOf<OutgoingBody::OutputStream>>,
    FutureResponse::IncomingResponse: WasiIncomingResponse<IncomingBody = IncomingBody>,
    IncomingBody: WasiIncomingBody<Pollable = PollableOf<FutureResponse>>,
    Registry: PollableRegistry<Pollable = PollableOf<OutgoingBody::OutputStream>>,
{
    type Output = Result<IncomingResponse<FutureResponse::IncomingResponse, Registry>, Error>;
    type IntoFuture = FutureIncomingResponse<FutureResponse, Registry>;

    fn into_future(self) -> Self::IntoFuture {
        self.future_response
    }
}

pub struct FutureIncomingResponse<FutureResponse, Registry: PollableRegistry> {
    inner: Subscribable<FutureResponse, Registry>,
}

impl<FutureResponse, IncomingBody, Registry> Future
    for FutureIncomingResponse<FutureResponse, Registry>
where
    FutureResponse: WasiFutureIncomingResponse,
    FutureResponse::IncomingResponse: WasiIncomingResponse<IncomingBody = IncomingBody>,
    IncomingBody: WasiIncomingBody<Pollable = PollableOf<FutureResponse>>,
    Registry: PollableRegistry<Pollable = PollableOf<FutureResponse>>,
{
    type Output = Result<IncomingResponse<FutureResponse::IncomingResponse, Registry>, Error>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.inner.get() {
            Some(Ok(res)) => {
                let response: <FutureResponse as WasiFutureIncomingResponse>::IncomingResponse =
                    res.map_err(Error::wasi_error_code)?;
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

pub struct OutgoingResponse<Response: WasiOutgoingResponse, Registry: PollableRegistry> {
    response: Response,
    body: OutgoingBody<Response::OutgoingBody, Registry>,
}

pub type OutgoingResponsePollable<Response> = PollableOf<
    <<Response as WasiOutgoingResponse>::OutgoingBody as WasiOutgoingBody>::OutputStream,
>;

impl<Response, Registry> OutgoingResponse<Response, Registry>
where
    Response: WasiOutgoingResponse,
    Registry: PollableRegistry<Pollable = OutgoingResponsePollable<Response>>,
{
    pub fn new(response: Response, registry: Registry) -> Result<Self, Error> {
        let body = response
            .body()
            .map_err(|()| Error::WasiInvalidState("outgoing-response.body already called"))?;
        let body = OutgoingBody::new(body, registry)?;
        Ok(Self { response, body })
    }

    pub fn from_headers(headers: &FieldEntries, registry: Registry) -> Result<Self, Error> {
        let fields = headers.try_into_fields()?;
        let response = Response::new(fields);
        Self::new(response, registry)
    }

    pub fn set_status_code(&mut self, status_code: u16) -> Result<(), Error> {
        self.response
            .set_status_code(status_code)
            .map_err(|()| Error::WasiInvalidValue("invalid status code"))
    }

    pub fn body(&mut self) -> &mut OutgoingBody<Response::OutgoingBody, Registry> {
        &mut self.body
    }

    pub fn into_body(self) -> OutgoingBody<Response::OutgoingBody, Registry> {
        self.body
    }

    fn into_parts(self) -> (Response, OutgoingBody<Response::OutgoingBody, Registry>) {
        (self.response, self.body)
    }
}

pub struct ResponseOutparam<Outparam> {
    outparam: Outparam,
}

impl<Outparam> ResponseOutparam<Outparam>
where
    Outparam: WasiResponseOutparam,
{
    pub fn new(outparam: Outparam) -> Self {
        Self { outparam }
    }

    pub fn set_response<Registry>(
        self,
        response: OutgoingResponse<Outparam::OutgoingResponse, Registry>,
    ) -> OutgoingBody<<Outparam::OutgoingResponse as WasiOutgoingResponse>::OutgoingBody, Registry>
    where
        Registry: PollableRegistry<Pollable = OutgoingResponsePollable<Outparam::OutgoingResponse>>,
    {
        let (wasi_response, body) = response.into_parts();
        self.outparam.set(Ok(wasi_response));
        body
    }

    pub fn set_error(self, err: &Outparam::ErrorCode) {
        self.outparam.set(Err(err));
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

#[derive(Debug)]
pub enum Scheme {
    Http,
    Https,
    Other(String),
}

#[derive(Debug)]
pub struct FieldEntries(Vec<(String, Vec<u8>)>);

impl FieldEntries {
    pub fn try_into_fields<Fields: WasiFields>(&self) -> Result<Fields, Error> {
        Fields::from_list(&self.0).map_err(|err| Error::WasiFieldsError(err.to_string()))
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

impl<Fields: WasiFields> From<Fields> for FieldEntries {
    fn from(fields: Fields) -> Self {
        Self(fields.entries())
    }
}

pub enum StreamError<IoError> {
    LastOperationFailed(IoError),
    Closed,
}
