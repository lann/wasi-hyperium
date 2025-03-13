use futures_util::future;

use crate::{
    hyperium1::{incoming_response, Hyperium1OutgoingBodyCopier},
    outgoing::OutgoingBodyCopier,
    poll::PollableRegistry,
    wasi::OutgoingRequest,
    Error, IncomingHttpBody,
};

use super::outgoing_request;

pub fn block_on_outbound_request<HttpBody, Registry>(
    request: http1::Request<HttpBody>,
    registry: Registry,
) -> Result<http1::Response<IncomingHttpBody<Registry>>, Error>
where
    HttpBody: http_body1::Body + Unpin,
    HttpBody::Data: Unpin,
    anyhow::Error: From<HttpBody::Error>,
    Registry: PollableRegistry,
{
    registry
        .clone()
        .block_on(send_outbound_request(request, registry))
        .unwrap()
}

pub async fn send_outbound_request<HttpBody, Registry>(
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
    let req_body_copier = Hyperium1OutgoingBodyCopier::new(request.into_body(), outgoing_body)?;
    let copier = req_body_copier.copy_all();
    let (response, _) = future::try_join(future_response, copier).await?;
    incoming_response(response)
}
