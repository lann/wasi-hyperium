use std::task::Poll;

use bytes::Bytes;
use wasi::http::types;

use crate::{
    incoming::{IncomingHttpBody, IncomingState},
    poll::PollableRegistry,
    wasi::IncomingRequest,
    Error,
};

pub fn incoming_request<Registry>(
    request: types::IncomingRequest,
    registry: Registry,
) -> Result<http0::Request<IncomingHttpBody<Registry>>, Error>
where
    Registry: PollableRegistry,
{
    let req = IncomingRequest::new(request, registry)?;
    let uri = {
        let mut builder = http0::Uri::builder();
        if let Some(scheme) = req.scheme() {
            builder = builder.scheme(scheme);
        }
        if let Some(auth) = req.authority() {
            builder = builder.authority(auth)
        }
        if let Some(p_and_q) = req.path_with_query() {
            builder = builder.path_and_query(p_and_q);
        }
        builder.build()?
    };
    let mut builder = http0::Request::builder().method(req.method()).uri(uri);
    for (name, val) in req.headers() {
        builder = builder.header(name, val);
    }
    Ok(builder.body(req.into_body().into())?)
}

impl<Registry> http_body0::Body for IncomingHttpBody<Registry>
where
    Registry: PollableRegistry,
{
    type Data = Bytes;
    type Error = Error;

    fn poll_data(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        let IncomingState::Body { .. } = &self.state else {
            return Poll::Ready(None);
        };
        self.poll_incoming_body(cx)
    }

    fn poll_trailers(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<http0::HeaderMap>, Self::Error>> {
        match self.poll_incoming_trailers(cx)? {
            Poll::Ready(Some(trailers)) => Poll::Ready(Ok(Some(trailers.try_into()?))),
            Poll::Ready(None) => Poll::Ready(Ok(None)),
            Poll::Pending => Poll::Pending,
        }
    }
}
