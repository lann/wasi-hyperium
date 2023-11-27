use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use crate::Error;

pub enum Copied {
    Body(usize),
    Trailers,
}

pub trait OutgoingBodyCopier {
    fn poll_copy(&mut self, cx: &mut Context) -> Poll<Option<Result<Copied, Error>>>;

    fn copy_all(self) -> CopyAllFuture<Self>
    where
        Self: Sized + Unpin,
    {
        CopyAllFuture(self)
    }
}

pub struct CopyAllFuture<T>(T);

impl<T: OutgoingBodyCopier + Unpin> Future for CopyAllFuture<T> {
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.0.poll_copy(cx) {
                Poll::Ready(Some(Ok(_))) => (),
                Poll::Ready(Some(Err(err))) => return Poll::Ready(Err(err)),
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
