use std::task::{Context, Poll};

use bytes::Bytes;
use http_body1::Frame;

use crate::{
    incoming::{IncomingHttpBody, IncomingState},
    poll::PollableRegistry,
    wasi::{
        traits::{WasiIncomingBody, WasiIncomingRequest, WasiIncomingResponse},
        IncomingRequest, IncomingResponse,
    },
    Error,
};

pub fn incoming_request<Request, Registry>(
    request: IncomingRequest<Request, Registry>,
) -> Result<http1::Request<IncomingHttpBody<Request::IncomingBody, Registry>>, Error>
where
    Request: WasiIncomingRequest,
    Request::IncomingBody: WasiIncomingBody<Pollable = Registry::Pollable>,
    Registry: PollableRegistry,
{
    let uri = {
        let mut builder = http1::Uri::builder();
        if let Some(scheme) = request.scheme() {
            builder = builder.scheme(scheme);
        }
        if let Some(auth) = request.authority() {
            builder = builder.authority(auth)
        }
        if let Some(p_and_q) = request.path_with_query() {
            builder = builder.path_and_query(p_and_q);
        }
        builder.build()?
    };
    let mut builder = http1::Request::builder().method(request.method()).uri(uri);
    for (name, val) in request.headers() {
        builder = builder.header(name, val);
    }
    Ok(builder.body(request.into_body().into())?)
}

pub fn incoming_response<Response, Registry>(
    response: IncomingResponse<Response, Registry>,
) -> Result<http1::Response<IncomingHttpBody<Response::IncomingBody, Registry>>, Error>
where
    Response: WasiIncomingResponse,
    Response::IncomingBody: WasiIncomingBody<Pollable = Registry::Pollable>,
    Registry: PollableRegistry,
{
    let mut builder = http1::Response::builder().status(response.status());
    for (name, val) in response.headers() {
        builder = builder.header(name, val);
    }
    Ok(builder.body(response.into_body().into())?)
}

impl<IncomingBody, Registry> http_body1::Body for IncomingHttpBody<IncomingBody, Registry>
where
    IncomingBody: WasiIncomingBody<Pollable = Registry::Pollable>,
    Registry: PollableRegistry,
{
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match &self.state {
            IncomingState::Empty => Poll::Ready(None),
            IncomingState::Body { .. } => match self.poll_incoming_body(cx)? {
                Poll::Ready(Some(frame)) => Poll::Ready(Some(Ok(Frame::data(frame)))),
                Poll::Ready(None) => self.poll_hyperium1_trailers(cx),
                Poll::Pending => Poll::Pending,
            },
            IncomingState::Trailers(_) => self.poll_hyperium1_trailers(cx),
        }
    }
}

impl<IncomingBody, Registry> IncomingHttpBody<IncomingBody, Registry>
where
    IncomingBody: WasiIncomingBody<Pollable = Registry::Pollable>,
    Registry: PollableRegistry,
{
    #[allow(clippy::type_complexity)]
    fn poll_hyperium1_trailers(
        &mut self,
        cx: &mut Context,
    ) -> Poll<Option<Result<Frame<Bytes>, Error>>> {
        match self.poll_incoming_trailers(cx)? {
            Poll::Ready(Some(trailers)) => {
                Poll::Ready(Some(Ok(Frame::trailers(trailers.try_into()?))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
