use {Future, Poll, task};

use futures_io::{CoreAsyncRead, CoreAsyncWrite, CoreIoError};

/// A future which will copy all data from a reader into a writer.
///
/// Created by the [`copy_into`] function, this future will resolve to the number of
/// bytes copied or an error if one happens.
///
/// [`copy_into`]: fn.copy_into.html
#[derive(Debug)]
pub struct CopyInto<R, W, B> {
    reader: Option<R>,
    read_done: bool,
    writer: Option<W>,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Option<B>,
}

pub fn copy_into<R, W, B>(reader: R, writer: W, buf: B) -> CopyInto<R, W, B> {
    CopyInto {
        reader: Some(reader),
        read_done: false,
        writer: Some(writer),
        amt: 0,
        pos: 0,
        cap: 0,
        buf: Some(buf),
    }
}

impl<R, W, B> Future for CopyInto<R, W, B>
    where R: CoreAsyncRead,
          W: CoreAsyncWrite<Error = R::Error>,
          B: AsRef<[u8]> + AsMut<[u8]>,
{
    type Item = (u64, R, W, B);
    type Error = R::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(u64, R, W, B), Self::Error> {
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            if self.pos == self.cap && !self.read_done {
                let reader = self.reader.as_mut().unwrap();
                let buf = self.buf.as_mut().unwrap().as_mut();
                let n = try_ready!(reader.poll_read_core(cx, buf));
                if n == 0 {
                    self.read_done = true;
                } else {
                    self.pos = 0;
                    self.cap = n;
                }
            }

            // If our buffer has some data, let's write it out!
            while self.pos < self.cap {
                let writer = self.writer.as_mut().unwrap();
                let buf = self.buf.as_ref().unwrap().as_ref();
                let i = try_ready!(writer.poll_write_core(cx, &buf[self.pos..self.cap]));
                if i == 0 {
                    return Err(Self::Error::write_zero("write zero byte into writer"));
                } else {
                    self.pos += i;
                    self.amt += i as u64;
                }
            }

            // If we've written al the data and we've seen EOF, flush out the
            // data and finish the transfer.
            // done with the entire transfer.
            if self.pos == self.cap && self.read_done {
                try_ready!(self.writer.as_mut().unwrap().poll_flush_core(cx));
                let reader = self.reader.take().unwrap();
                let writer = self.writer.take().unwrap();
                let buf = self.buf.take().unwrap();
                return Ok((self.amt, reader, writer, buf).into())
            }
        }
    }
}
