use std::task::{Context, Poll};

use bytes::Bytes;
use http_body1::Frame;

use crate::{
    incoming::{IncomingHttpBody, IncomingState},
    poll::PollableRegistry,
    wasi::traits::WasiIncomingBody,
    Error,
};

impl<IncomingBody, Registry> http_body1::Body for IncomingHttpBody<IncomingBody, Registry>
where
    IncomingBody: WasiIncomingBody,
    Registry: PollableRegistry<Pollable = IncomingBody::Pollable>,
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
    IncomingBody: WasiIncomingBody,
    Registry: PollableRegistry<Pollable = IncomingBody::Pollable>,
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
