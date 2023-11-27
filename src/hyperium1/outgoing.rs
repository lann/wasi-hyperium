use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Buf;

use crate::{
    outgoing::{Copied, OutgoingBodyCopier},
    poll::PollableRegistry,
    wasi::{traits::WasiOutgoingBody, OutgoingBody, PollableOf},
    Error,
};

pub struct Hyperium1OutgoingBodyCopier<HttpBody, WasiBody, Registry>
where
    HttpBody: http_body1::Body,
    WasiBody: WasiOutgoingBody,
    Registry: PollableRegistry,
{
    src: HttpBody,
    dest: Option<OutgoingBody<WasiBody, Registry>>,
    buf: Option<HttpBody::Data>,
}

impl<HttpBody, WasiBody, Registry> Hyperium1OutgoingBodyCopier<HttpBody, WasiBody, Registry>
where
    HttpBody: http_body1::Body,
    WasiBody: WasiOutgoingBody,
    Registry: PollableRegistry<Pollable = PollableOf<WasiBody::OutputStream>>,
{
    pub fn new(src: HttpBody, dest: WasiBody, registry: Registry) -> Result<Self, Error> {
        let dest = OutgoingBody::new(dest, registry)?;
        Ok(Self {
            src,
            dest: Some(dest),
            buf: None,
        })
    }
}

impl<HttpBody, WasiBody, Registry> OutgoingBodyCopier
    for Hyperium1OutgoingBodyCopier<HttpBody, WasiBody, Registry>
where
    HttpBody: http_body1::Body + Unpin,
    anyhow::Error: From<HttpBody::Error>,
    WasiBody: WasiOutgoingBody,
    WasiBody::Trailers: Sized,
    Registry: PollableRegistry<Pollable = PollableOf<WasiBody::OutputStream>>,
{
    fn poll_copy(&mut self, cx: &mut Context) -> Poll<Option<Result<Copied, Error>>> {
        if self.dest.is_none() {
            return Poll::Ready(None);
        }

        if self.buf.is_none() {
            // Fill buffer
            match Pin::new(&mut self.src)
                .poll_frame(cx)
                .map_err(|err| Error::BodyError(err.into()))?
            {
                Poll::Ready(Some(frame)) => {
                    if frame.is_data() {
                        self.buf =
                            Some(frame.into_data().unwrap_or_else(|_| {
                                panic!("into_data failed when is_data = true")
                            }));
                    } else {
                        // Got trailers; finish outgoing-body
                        let trailers = frame.into_trailers().unwrap_or_else(|_| {
                            panic!("into_trailers failed when is_data = false")
                        });
                        self.dest.take().unwrap().finish(Some(trailers.into()))?;
                        return Poll::Ready(Some(Ok(Copied::Trailers)));
                    }
                }
                Poll::Ready(None) => {
                    // End of body (no trailers); finish outgoing-body
                    self.dest.take().unwrap().finish(None)?;
                    return Poll::Ready(None);
                }
                Poll::Pending => return Poll::Pending,
            }
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
