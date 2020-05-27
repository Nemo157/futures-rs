use futures_core::future::Future;
use futures_core::task::{Context, Poll};
use futures_io::{AsyncRead, ReadBuf};
use std::io;
use std::pin::Pin;
use std::vec::Vec;
use std::mem::MaybeUninit;

/// Future for the [`read_to_end`](super::AsyncReadExt::read_to_end) method.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct ReadToEnd<'a, R: ?Sized> {
    reader: &'a mut R,
    buf: &'a mut Vec<u8>,
    start_len: usize,
    initialized: usize,
}

impl<R: ?Sized + Unpin> Unpin for ReadToEnd<'_, R> {}

impl<'a, R: AsyncRead + ?Sized + Unpin> ReadToEnd<'a, R> {
    pub(super) fn new(reader: &'a mut R, buf: &'a mut Vec<u8>) -> Self {
        let start_len = buf.len();
        Self {
            reader,
            buf,
            start_len,
            initialized: 0,
        }
    }
}

// This uses an adaptive system to extend the vector when it fills. We want to
// avoid paying to allocate and zero a huge chunk of memory if the reader only
// has 4 bytes while still making large reads if the reader does have a ton
// of data to return. Simply tacking on an extra DEFAULT_BUF_SIZE space every
// time is 4,500 times (!) slower than this if the reader has a very small
// amount of data to return.
//
// Because we're extending the buffer with uninitialized data for trusted
// readers, we need to make sure to truncate that if any of this panics.
pub(super) fn read_to_end_internal<R: AsyncRead + ?Sized>(
    mut rd: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut Vec<u8>,
    start_len: usize,
    initialized: &mut usize,
) -> Poll<io::Result<usize>> {
    loop {
        if buf.capacity() == buf.len() {
            buf.reserve(32);
        }

        let read_len = {
            let spare_len = buf.capacity() - buf.len();
            assert!(spare_len > 0);
            let spare_ptr = unsafe { buf.as_mut_ptr().add(buf.len()).cast::<MaybeUninit<u8>>() };
            let spare_slice = unsafe { std::slice::from_raw_parts_mut(spare_ptr, spare_len) };
            let mut read_buf = ReadBuf::uninit(spare_slice);
            unsafe {
                read_buf.assume_init(*initialized);
            }

            dbg!(&read_buf);
            ready!(rd.as_mut().poll_read_buf(cx, &mut read_buf))?;
            dbg!(&read_buf);

            if read_buf.filled().is_empty() {
                break;
            }

            *initialized = read_buf.initialized().len() - read_buf.filled().len();
            read_buf.filled().len()
        };

        unsafe {
            buf.set_len(buf.len() + read_len);
        }
    }

    Poll::Ready(Ok(buf.len() - start_len))
}

impl<A> Future for ReadToEnd<'_, A>
    where A: AsyncRead + ?Sized + Unpin,
{
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = &mut *self;
        read_to_end_internal(Pin::new(&mut this.reader), cx, this.buf, this.start_len, &mut this.initialized)
    }
}
