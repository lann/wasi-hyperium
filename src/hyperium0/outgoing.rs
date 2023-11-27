use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Buf;

use crate::{
    outgoing::{Copied, OutgoingBodyCopier},
    poll::PollableRegistry,
    wasi::{
        traits::{WasiOutgoingBody, WasiOutgoingResponse},
        OutgoingBody, OutgoingResponse, PollableOf,
    },
    Error,
};

pub fn outgoing_response<B, Response, Registry>(
    resp: &http0::Response<B>,
    registry: Registry,
) -> Result<OutgoingResponse<Response, Registry>, Error>
where
    Response: WasiOutgoingResponse,
    Registry: PollableRegistry<
        Pollable = PollableOf<<Response::OutgoingBody as WasiOutgoingBody>::OutputStream>,
    >,
{
    let mut outgoing = OutgoingResponse::from_headers(&resp.headers().into(), registry)?;
    outgoing.set_status_code(resp.status().as_u16())?;
    Ok(outgoing)
}

pub struct Hyperium0OutgoingBodyCopier<
    HttpBody: http_body0::Body,
    WasiBody: WasiOutgoingBody,
    Registry: PollableRegistry,
> {
    src: HttpBody,
    dest: Option<OutgoingBody<WasiBody, Registry>>,
    buf: Option<HttpBody::Data>,
    trailers_pending: bool,
}

impl<HttpBody, WasiBody, Registry> Hyperium0OutgoingBodyCopier<HttpBody, WasiBody, Registry>
where
    HttpBody: http_body0::Body,
    WasiBody: WasiOutgoingBody,
    Registry: PollableRegistry<Pollable = PollableOf<WasiBody::OutputStream>>,
{
    pub fn new(src: HttpBody, dest: OutgoingBody<WasiBody, Registry>) -> Result<Self, Error> {
        Ok(Self {
            src,
            dest: Some(dest),
            buf: None,
            trailers_pending: false,
        })
    }
}

impl<HttpBody, WasiBody, Registry> OutgoingBodyCopier
    for Hyperium0OutgoingBodyCopier<HttpBody, WasiBody, Registry>
where
    HttpBody: http_body0::Body + Unpin,
    anyhow::Error: From<HttpBody::Error>,
    WasiBody: WasiOutgoingBody,
    WasiBody::Trailers: Sized,
    Registry: PollableRegistry<Pollable = PollableOf<WasiBody::OutputStream>>,
{
    fn poll_copy(&mut self, cx: &mut Context) -> Poll<Option<Result<Copied, Error>>> {
        if self.dest.is_none() {
            return Poll::Ready(None);
        }

        if self.buf.is_none() && !self.trailers_pending {
            // Fill buffer
            match Pin::new(&mut self.src)
                .poll_data(cx)
                .map_err(|err| Error::BodyError(err.into()))?
            {
                Poll::Ready(Some(frame)) => {
                    self.buf = Some(frame);
                }
                Poll::Ready(None) => {
                    // End of body; poll for trailers next
                    self.trailers_pending = true;
                }
                Poll::Pending => return Poll::Pending,
            }
        }

        if self.trailers_pending {
            return match Pin::new(&mut self.src)
                .poll_trailers(cx)
                .map_err(|err| Error::BodyError(err.into()))?
            {
                Poll::Ready(Some(trailers)) => {
                    self.dest.take().unwrap().finish(Some(trailers.into()))?;
                    Poll::Ready(Some(Ok(Copied::Trailers)))
                }
                Poll::Ready(None) => {
                    self.dest.take().unwrap().finish(None)?;
                    Poll::Ready(None)
                }
                Poll::Pending => Poll::Pending,
            };
        }

        // Write buffer
        let stream = self.dest.as_mut().unwrap().stream();
        match stream.poll_check_write(cx)? {
            Poll::Ready(permit) => {
                let buf = self.buf.as_mut().unwrap();
                let len = permit.write(buf.chunk())?;
                buf.advance(len);
                if !buf.has_remaining() {
                    self.buf = None;
                }
                Poll::Ready(Some(Ok(Copied::Body(len))))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
