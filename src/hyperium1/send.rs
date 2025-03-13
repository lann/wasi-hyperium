use crate::{
    hyperium1::{incoming_response, Hyperium1OutgoingBodyCopier},
    outgoing::OutgoingBodyCopier,
    poll::PollableRegistry,
    wasi::OutgoingRequest,
    Error, IncomingHttpBody,
};

use super::outgoing_request;

pub fn send_request<HttpBody, Registry>(
    request: http1::Request<HttpBody>,
    registry: Registry,
) -> Result<http1::Response<IncomingHttpBody<Registry>>, Error>
where
    HttpBody: http_body1::Body + Unpin,
    HttpBody::Data: Unpin,
    anyhow::Error: From<HttpBody::Error>,
    Registry: PollableRegistry,
{
    let outgoing: OutgoingRequest<_> = outgoing_request(&request, registry.clone())?;
    let (outgoing_body, future_response) = outgoing.send(None)?.into_parts();
    let copier = Hyperium1OutgoingBodyCopier::new(request.into_body(), outgoing_body)?;
    registry.block_on(copier.copy_all()).unwrap()?;
    let incoming = registry.block_on(future_response).unwrap()?;
    incoming_response(incoming)
}
