use std::{convert::Infallible, task::Context};

use wasi::http::types;

use crate::{
    hyperium1::{incoming_request, outgoing_response},
    outgoing::OutgoingBodyCopier,
    poll::{noop_waker, PollableRegistry},
    wasi::{IncomingRequest, ResponseOutparam},
    Error, IncomingHttpBody,
};

use super::Hyperium1OutgoingBodyCopier;

pub fn handle_service_call<Service, ResponseBody, Registry>(
    mut service: Service,
    request: types::IncomingRequest,
    response_out: types::ResponseOutparam,
    registry: Registry,
) -> Result<(), Error>
where
    Service: tower_service::Service<
        http1::Request<IncomingHttpBody<Registry>>,
        Response = http1::Response<ResponseBody>,
        Error = Infallible,
    >,
    ResponseBody: http_body1::Body + Unpin,
    ResponseBody::Data: Unpin,
    anyhow::Error: From<ResponseBody::Error>,
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
