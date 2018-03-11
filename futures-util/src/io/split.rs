use {Async, Poll, task};
use lock::BiLock;

use futures_io::{CoreAsyncRead, CoreAsyncWrite, IoVec, IoVecMut};

/// The readable half of an object returned from `CoreAsyncRead::split`.
#[derive(Debug)]
pub struct ReadHalf<T> {
    handle: BiLock<T>,
}

/// The writable half of an object returned from `CoreAsyncRead::split`.
#[derive(Debug)]
pub struct WriteHalf<T> {
    handle: BiLock<T>,
}

fn lock_and_then<T, U, E, F>(lock: &BiLock<T>, cx: &mut task::Context, f: F) -> Result<Async<U>, E>
    where F: FnOnce(&mut T, &mut task::Context) -> Result<Async<U>, E>
{
    match lock.poll_lock(cx) {
        Async::Ready(ref mut l) => f(l, cx),
        Async::Pending => Ok(Async::Pending),
    }
}

pub fn split<T: CoreAsyncRead + CoreAsyncWrite>(t: T) -> (ReadHalf<T>, WriteHalf<T>) {
    let (a, b) = BiLock::new(t);
    (ReadHalf { handle: a }, WriteHalf { handle: b })
}

impl<T: CoreAsyncRead> CoreAsyncRead for ReadHalf<T> {
    type Error = T::Error;

    fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8])
        -> Poll<usize, Self::Error>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_read(cx, buf))
    }

    fn poll_vectored_read(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVecMut])
        -> Poll<usize, Self::Error>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_vectored_read(cx, vec))
    }
}

impl<T: CoreAsyncWrite> CoreAsyncWrite for WriteHalf<T> {
    type Error = T::Error;

    fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8])
        -> Poll<usize, Self::Error>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_write(cx, buf))
    }

    fn poll_vectored_write(&mut self, cx: &mut task::Context, vec: &[&IoVec])
        -> Poll<usize, Self::Error>
    {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_vectored_write(cx, vec))
    }

    fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Self::Error> {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_flush(cx))
    }

    fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Self::Error> {
        lock_and_then(&self.handle, cx, |l, cx| l.poll_close(cx))
    }
}
