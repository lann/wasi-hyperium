use std::{convert::Infallible, task::Context};

use wasi::http::types;

use crate::{
    hyperium0::{incoming_request, outgoing_response, Hyperium0OutgoingBodyCopier},
    outgoing::OutgoingBodyCopier,
    poll::{noop_waker, PollableRegistry, Poller},
    wasi::ResponseOutparam,
    Error, IncomingHttpBody,
};

pub fn handle_service_call<Service, Request, Outparam, ResponseBody>(
    mut service: Service,
    request: types::IncomingRequest,
    response_out: types::ResponseOutparam,
) -> Result<(), Error>
where
    Service: tower_service::Service<
        http0::Request<IncomingHttpBody<Poller>>,
        Response = http0::Response<ResponseBody>,
        Error = Infallible,
    >,
    ResponseBody: http_body0::Body + Unpin,
    ResponseBody::Data: Unpin,
    anyhow::Error: From<ResponseBody::Error>,
{
    let poller = Poller::default();
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    while service.poll_ready(&mut cx).is_pending() {
        if !poller.poll() {
            panic!("service never became ready");
        }
    }

    let req = incoming_request(request, poller.clone())?;
    let resp = poller.block_on(service.call(req)).unwrap().unwrap();

    let outgoing = outgoing_response(&resp, poller.clone())?;
    let dest = ResponseOutparam::new(response_out).set_response(outgoing);

    let copier = Hyperium0OutgoingBodyCopier::new(resp.into_body(), dest)?;
    poller.block_on(copier.copy_all()).unwrap()
}
