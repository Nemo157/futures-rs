use core::mem;

use {Poll, Future, task};

use futures_io::{CoreAsyncRead, CoreIoError};

/// A future which can be used to easily read exactly enough bytes to fill
/// a buffer.
///
/// Created by the [`read_exact`] function.
///
/// [`read_exact`]: fn.read_exact.html
#[derive(Debug)]
pub struct ReadExact<A, T> {
    state: State<A, T>,
}

#[derive(Debug)]
enum State<A, T> {
    Reading {
        a: A,
        buf: T,
        pos: usize,
    },
    Empty,
}

pub fn read_exact<A, T>(a: A, buf: T) -> ReadExact<A, T>
    where A: CoreAsyncRead,
          T: AsMut<[u8]>,
{
    ReadExact {
        state: State::Reading {
            a,
            buf,
            pos: 0,
        },
    }
}

impl<A, T> Future for ReadExact<A, T>
    where A: CoreAsyncRead,
          T: AsMut<[u8]>,
{
    type Item = (A, T);
    type Error = A::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(A, T), Self::Error> {
        match self.state {
            State::Reading { ref mut a, ref mut buf, ref mut pos } => {
                let buf = buf.as_mut();
                while *pos < buf.len() {
                    let n = try_ready!(a.poll_read(cx, &mut buf[*pos..]));
                    *pos += n;
                    if n == 0 {
                        return Err(A::Error::unexpected_eof("early eof"))
                    }
                }
            }
            State::Empty => panic!("poll a ReadExact after it's done"),
        }

        match mem::replace(&mut self.state, State::Empty) {
            State::Reading { a, buf, .. } => Ok((a, buf).into()),
            State::Empty => panic!(),
        }
    }
}
