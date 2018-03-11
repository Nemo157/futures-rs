use core::mem;

use {Future, Poll, task};

use futures_io::CoreAsyncRead;

#[derive(Debug)]
enum State<R, T> {
    Pending {
        rd: R,
        buf: T,
    },
    Empty,
}

pub fn read<R, T>(rd: R, buf: T) -> Read<R, T>
    where R: CoreAsyncRead,
          T: AsMut<[u8]>
{
    Read { state: State::Pending { rd, buf } }
}

/// A future which can be used to easily read available number of bytes to fill
/// a buffer.
///
/// Created by the [`read`] function.
#[derive(Debug)]
pub struct Read<R, T> {
    state: State<R, T>,
}

impl<R, T> Future for Read<R, T>
    where R: CoreAsyncRead,
          T: AsMut<[u8]>
{
    type Item = (R, T, usize);
    type Error = R::Error;

    fn poll(&mut self, cx: &mut task::Context) -> Poll<(R, T, usize), Self::Error> {
        let nread = match self.state {
            State::Pending { ref mut rd, ref mut buf } =>
                try_ready!(rd.poll_read_core(cx, &mut buf.as_mut()[..])),
            State::Empty => panic!("poll a Read after it's done"),
        };

        match mem::replace(&mut self.state, State::Empty) {
            State::Pending { rd, buf } => Ok((rd, buf, nread).into()),
            State::Empty => panic!("invalid internal state"),
        }
    }
}
