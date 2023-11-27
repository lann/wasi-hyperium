use std::task::{Context, Poll};

use bytes::Bytes;

use crate::{
    poll::PollableRegistry,
    wasi::{traits::WasiIncomingBody, FieldEntries, FutureTrailers, IncomingBody},
    Error,
};

pub struct IncomingHttpBody<Body, Registry>
where
    Body: WasiIncomingBody,
    Registry: PollableRegistry,
{
    pub(crate) state: IncomingState<Body, Registry>,
}

pub(crate) enum IncomingState<Body: WasiIncomingBody, Registry: PollableRegistry> {
    Empty,
    Body(IncomingBody<Body, Registry>),
    Trailers(FutureTrailers<Body::FutureTrailers, Registry>),
}

const READ_FRAME_SIZE: usize = 16 * 1024;

impl<Body, Registry> IncomingHttpBody<Body, Registry>
where
    Body: WasiIncomingBody,
    Registry: PollableRegistry<Pollable = Body::Pollable>,
{
    pub fn new(body: Body, registry: Registry) -> Result<Self, Error> {
        Ok(IncomingBody::new(body, registry)?.into())
    }

    pub fn poll_incoming_body(&mut self, cx: &mut Context) -> Poll<Option<Result<Bytes, Error>>> {
        let IncomingState::Body(incoming_body) = &mut self.state else {
            panic!("poll_incoming_body called on non-body state")
        };

        match incoming_body.stream().poll_read(READ_FRAME_SIZE, cx) {
            Poll::Ready(Ok(data)) => Poll::Ready(Some(Ok(data.into()))),
            Poll::Ready(Err(Error::WasiStreamClosed)) => {
                self.state = IncomingState::Trailers(self.take_body().finish());
                Poll::Ready(None)
            }
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            Poll::Pending => Poll::Pending,
        }
    }

    pub fn poll_incoming_trailers(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<Option<FieldEntries>, Error>> {
        match &mut self.state {
            IncomingState::Empty => Poll::Ready(Ok(None)),
            IncomingState::Body { .. } => panic!("poll_trailers called before body completion"),
            IncomingState::Trailers(trailers) => match trailers.poll_trailers(cx) {
                Poll::Ready(Ok(Some(trailers))) => {
                    self.state = IncomingState::Empty;
                    Poll::Ready(Ok(Some(trailers)))
                }
                Poll::Ready(Ok(None)) => {
                    self.state = IncomingState::Empty;
                    Poll::Ready(Ok(None))
                }
                // TODO: figure out why this is happening
                Poll::Ready(Err(Error::WasiErrorCode(s))) if s.contains("ConnectionTerminated") => {
                    self.state = IncomingState::Empty;
                    Poll::Ready(Ok(None))
                }
                Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
                Poll::Pending => Poll::Pending,
            },
        }
    }

    pub(crate) fn take_body(&mut self) -> IncomingBody<Body, Registry> {
        match std::mem::replace(&mut self.state, IncomingState::Empty) {
            IncomingState::Body(body) => body,
            _ => panic!("called take_body on non-body state"),
        }
    }
}

impl<Body, Registry> From<IncomingBody<Body, Registry>> for IncomingHttpBody<Body, Registry>
where
    Body: WasiIncomingBody,
    Registry: PollableRegistry<Pollable = Body::Pollable>,
{
    fn from(body: IncomingBody<Body, Registry>) -> Self {
        Self {
            state: IncomingState::Body(body),
        }
    }
}
