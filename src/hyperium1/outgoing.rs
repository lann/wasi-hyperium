use std::{
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Buf;

use crate::{
    outgoing::{Copied, OutgoingBodyCopier},
    poll::PollableRegistry,
    wasi::{OutgoingBody, OutgoingRequest, OutgoingResponse},
    Error,
};

pub fn outgoing_request<B, Registry>(
    request: &http1::Request<B>,
    registry: Registry,
) -> Result<OutgoingRequest<Registry>, Error>
where
    Registry: PollableRegistry,
{
    let mut req = OutgoingRequest::from_headers(&request.headers().into(), registry)?;
    req.set_method(request.method().into())?;
    if let Some(path_with_query) = request.uri().path_and_query() {
        req.set_path_with_query(Some(path_with_query.as_str()))?;
    }
    if let Some(scheme) = request.uri().scheme() {
        req.set_scheme(Some(scheme.into()))?;
    }
    if let Some(authority) = request.uri().authority() {
        req.set_authority(Some(authority.as_str()))?;
    }

    Ok(req)
}

pub fn outgoing_response<B, Registry>(
    resp: &http1::Response<B>,
    registry: Registry,
) -> Result<OutgoingResponse<Registry>, Error>
where
    Registry: PollableRegistry,
{
    let mut outgoing = OutgoingResponse::from_headers(&resp.headers().into(), registry)?;
    outgoing.set_status_code(resp.status().as_u16())?;
    Ok(outgoing)
}

pub struct Hyperium1OutgoingBodyCopier<HttpBody, Registry>
where
    HttpBody: http_body1::Body,
    Registry: PollableRegistry,
{
    src: HttpBody,
    dest: Option<OutgoingBody<Registry>>,
    buf: Option<HttpBody::Data>,
}

impl<HttpBody, Registry> Hyperium1OutgoingBodyCopier<HttpBody, Registry>
where
    HttpBody: http_body1::Body,
    Registry: PollableRegistry,
{
    pub fn new(src: HttpBody, dest: OutgoingBody<Registry>) -> Result<Self, Error> {
        Ok(Self {
            src,
            dest: Some(dest),
            buf: None,
        })
    }
}

impl<HttpBody, Registry> OutgoingBodyCopier for Hyperium1OutgoingBodyCopier<HttpBody, Registry>
where
    HttpBody: http_body1::Body + Unpin,
    anyhow::Error: From<HttpBody::Error>,
    Registry: PollableRegistry,
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
