use std::{convert::Infallible, task::Context};

use crate::{
    hyperium1::{incoming_request, outgoing_response},
    outgoing::OutgoingBodyCopier,
    poll::{noop_waker, PollableRegistry},
    wasi::{
        traits::{
            WasiIncomingBody, WasiIncomingRequest, WasiOutgoingBody, WasiOutgoingResponse,
            WasiOutputStream, WasiResponseOutparam,
        },
        IncomingRequest, ResponseOutparam,
    },
    Error, IncomingHttpBody,
};

use super::Hyperium1OutgoingBodyCopier;

pub fn handle_service_call<
    Service,
    Request,
    Outparam,
    ResponseBody,
    Registry,
>(
    mut service: Service,
    request: Request,
    response_out: Outparam,
    registry: Registry,
) -> Result<(), Error>
where
    Service: tower_service::Service<
        http1::Request<
            IncomingHttpBody<Request::IncomingBody, Registry>,
        >,
        Response = http1::Response<ResponseBody>,
        Error = Infallible,
    >,
    ResponseBody: http_body1::Body + Unpin,
    ResponseBody::Data: Unpin,
    anyhow::Error: From<ResponseBody::Error>,
    Request: WasiIncomingRequest,
    Request::IncomingBody: WasiIncomingBody<Pollable = Registry::Pollable>,
    Outparam: WasiResponseOutparam,
    <<Outparam::OutgoingResponse as WasiOutgoingResponse>::OutgoingBody as WasiOutgoingBody>::OutputStream: WasiOutputStream<Pollable = Registry::Pollable>,
    Registry: PollableRegistry,
{
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    while service.poll_ready(&mut cx).is_pending() {
        if !registry.poll() {
            panic!("service never became ready");
        }
    }

    let incoming = IncomingRequest::new(request, registry.clone())?;
    let req = incoming_request(incoming)?;

    let resp = registry.block_on(service.call(req)).unwrap().unwrap();

    let outgoing = outgoing_response(&resp, registry.clone())?;
    let dest = ResponseOutparam::new(response_out).set_response(outgoing);

    let copier = Hyperium1OutgoingBodyCopier::new(resp.into_body(), dest)?;
    registry.block_on(copier.copy_all()).unwrap()
}
