use crate::{
    hyperium1::{incoming_response, Hyperium1OutgoingBodyCopier},
    outgoing::OutgoingBodyCopier,
    poll::PollableRegistry,
    wasi::{
        traits::{
            WasiFutureIncomingResponse, WasiIncomingBody, WasiIncomingResponse, WasiOutgoingBody,
            WasiOutgoingHandler, WasiOutputStream,
        },
        OutgoingRequest,
    },
    Error, IncomingHttpBody,
};

use super::outgoing_request;

type IncomingResponseBody<Request> = <<<Request as WasiOutgoingHandler>::FutureIncomingResponse as WasiFutureIncomingResponse>::IncomingResponse as WasiIncomingResponse>::IncomingBody;

pub fn send_request<WasiRequest, HttpBody, Registry>(
    request: http1::Request<HttpBody>,
    registry: Registry,
) -> Result<http1::Response<IncomingHttpBody<IncomingResponseBody<WasiRequest>, Registry>>, Error>
where
    HttpBody: http_body1::Body + Unpin,
    HttpBody::Data: Unpin,
    anyhow::Error: From<HttpBody::Error>,
    WasiRequest: WasiOutgoingHandler,
    <WasiRequest::OutgoingBody as WasiOutgoingBody>::OutputStream:
        WasiOutputStream<Pollable = Registry::Pollable>,
    WasiRequest::FutureIncomingResponse: WasiFutureIncomingResponse<Pollable = Registry::Pollable>,
    <<WasiRequest::FutureIncomingResponse as WasiFutureIncomingResponse>::IncomingResponse as WasiIncomingResponse>::IncomingBody:
        WasiIncomingBody<Pollable = Registry::Pollable>,
    Registry: PollableRegistry,
{
    let outgoing: OutgoingRequest<WasiRequest, _> = outgoing_request(&request, registry.clone())?;
    let (outgoing_body, future_response) = outgoing.send(None)?.into_parts();
    let copier = Hyperium1OutgoingBodyCopier::new(request.into_body(), outgoing_body)?;
    registry.block_on(copier.copy_all()).unwrap()?;
    let incoming = registry.block_on(future_response).unwrap()?;
    incoming_response(incoming)
}
