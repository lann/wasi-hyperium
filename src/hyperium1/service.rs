use std::{convert::Infallible, task::Context};

use crate::{
    hyperium1::{incoming_request, outgoing_response},
    outgoing::OutgoingBodyCopier,
    poll::{noop_waker, PollableRegistry, Poller},
    wasi::{
        traits::{
            WasiIncomingRequest, WasiOutgoingBody, WasiOutgoingResponse, WasiOutputStream,
            WasiPoll, WasiResponseOutparam,
        },
        IncomingRequestPollable, ResponseOutparam,
    },
    Error, IncomingHttpBody,
};

use super::Hyperium1OutgoingBodyCopier;

pub fn handle_service_call<
    Service,
    Request,
    Outparam,
    ResponseBody,
    OutputStream,
    OutgoingBody,
    OutgoingResponse,
>(
    mut service: Service,
    request: Request,
    response_out: Outparam,
) -> Result<(), Error>
where
    Service: tower_service::Service<
        http1::Request<
            IncomingHttpBody<Request::IncomingBody, Poller<IncomingRequestPollable<Request>>>,
        >,
        Response = http1::Response<ResponseBody>,
        Error = Infallible,
    >,
    IncomingRequestPollable<Request>: WasiPoll,
    ResponseBody: http_body1::Body + Unpin,
    ResponseBody::Data: Unpin,
    anyhow::Error: From<ResponseBody::Error>,
    Request: WasiIncomingRequest,
    Outparam: WasiResponseOutparam<OutgoingResponse = OutgoingResponse>,
    OutgoingResponse: WasiOutgoingResponse<OutgoingBody = OutgoingBody>,
    OutgoingBody: WasiOutgoingBody<OutputStream = OutputStream>,
    OutputStream: WasiOutputStream<Pollable = IncomingRequestPollable<Request>>,
{
    let poller = Poller::<IncomingRequestPollable<Request>>::default();
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

    let copier = Hyperium1OutgoingBodyCopier::new(resp.into_body(), dest)?;
    poller.block_on(copier.copy_all()).unwrap()
}
