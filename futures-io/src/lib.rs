//! Asynchronous I/O
//!
//! This crate contains the `AsyncRead` and `AsyncWrite` traits, the
//! asynchronous analogs to `std::io::{Read, Write}`. The primary difference is
//! that these traits integrate with the asynchronous task system.

#![no_std]
#![deny(missing_docs, missing_debug_implementations)]
#![doc(html_rnoot_url = "https://docs.rs/futures-io/0.2.0-alpha")]

#![feature(specialization)]

macro_rules! if_std {
    ($($i:item)*) => ($(
        #[cfg(feature = "std")]
        $i
    )*)
}

extern crate futures_core;
extern crate iovec;

use core::cmp;
use core::ptr;

use futures_core::{Async, Poll, task};

// Re-export IoVec for convenience
pub use iovec::{IoVec, IoVecMut};

/// A type used to conditionally initialize buffers passed to `AsyncRead`
/// methods, modeled after `std`.
#[derive(Debug)]
pub struct Initializer(bool);

impl Initializer {
    /// Returns a new `Initializer` which will zero out buffers.
    #[inline]
    pub fn zeroing() -> Initializer {
        Initializer(true)
    }

    /// Returns a new `Initializer` which will not zero out buffers.
    ///
    /// # Safety
    ///
    /// This method may only be called by `AsyncRead`ers which guarantee
    /// that they will not read from the buffers passed to `AsyncRead`
    /// methods, and that the return value of the method accurately reflects
    /// the number of bytes that have been written to the head of the buffer.
    #[inline]
    pub unsafe fn nop() -> Initializer {
        Initializer(false)
    }

    /// Indicates if a buffer should be initialized.
    #[inline]
    pub fn should_initialize(&self) -> bool {
        self.0
    }

    /// Initializes a buffer if necessary.
    #[inline]
    pub fn initialize(&self, buf: &mut [u8]) {
        if self.should_initialize() {
            unsafe { ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len()) }
        }
    }
}

/// The minimum set of variants an IO error type must provide to allow for IO
/// adaptors to be built on top of it.
pub trait CoreIoError: Sized {
    /// An operation could not be completed because a call to `poll_write_core`
    /// returned `Ok(Async::Ready(0))`.
    ///
    /// This typically means that an operation could only succeed if it wrote a
    /// particular number of bytes but only a smaller number of bytes could be
    /// written.
    fn write_zero(msg: &'static str) -> Self;

    /// An operation could not be completed because an "end of file" was
    /// reached prematurely.
    ///
    /// This typically means that an operation could only succeed if it read a
    /// particular number of bytes but only a smaller number of bytes could be
    /// read.
    fn unexpected_eof(msg: &'static str) -> Self;
}

#[derive(Debug, Copy, Clone)]
/// A type providing the minimum set of variants for an IO error, to support
/// types that are otherwise infallible
pub enum MinimalIoError {
    /// An operation could not be completed because a call to `poll_write_core`
    /// returned `Ok(Async::Ready(0))`.
    ///
    /// This typically means that an operation could only succeed if it wrote a
    /// particular number of bytes but only a smaller number of bytes could be
    /// written.
    WriteZero(&'static str),

    /// An operation could not be completed because an "end of file" was
    /// reached prematurely.
    ///
    /// This typically means that an operation could only succeed if it read a
    /// particular number of bytes but only a smaller number of bytes could be
    /// read.
    UnexpectedEof(&'static str),
}

impl CoreIoError for MinimalIoError {
    fn write_zero(msg: &'static str) -> Self {
        MinimalIoError::WriteZero(msg)
    }

    fn unexpected_eof(msg: &'static str) -> Self {
        MinimalIoError::UnexpectedEof(msg)
    }
}

/// `std`-less trait to read bytes asynchronously.
///
/// This trait is analogous to the `std::io::Read` trait, but integrates
/// with the asynchronous task system. In particular, the `poll_read`
/// method, unlike `Read::read`, will automatically queue the current task
/// for wakeup and return if data is not yet available, rather than blocking
/// the calling thread.
pub trait CoreAsyncRead {
    /// TODO
    type Error: CoreIoError;

    /// Determines if this `CoreAsyncRead`er can work with buffers of
    /// uninitialized memory.
    ///
    /// The default implementation returns an initializer which will zero
    /// buffers.
    ///
    /// # Safety
    ///
    /// This method is `unsafe` because and `CoreAsyncRead`er could otherwise
    /// return a non-zeroing `Initializer` from another `CoreAsyncRead` type
    /// without an `unsafe` block.
    #[inline]
    unsafe fn initializer_core(&self) -> Initializer {
        Initializer::zeroing()
    }

    /// Attempt to read from the `CoreAsyncRead` into `buf`.
    ///
    /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
    ///
    /// If no data is available for reading, the method returns
    /// `Ok(Async::Pending)` and arranges for the current task (via
    /// `cx.waker()`) to receive a notification when the object becomes
    /// readable or is closed.
    fn poll_read_core(&mut self, cx: &mut task::Context, buf: &mut [u8])
        -> Poll<usize, Self::Error>;

    /// Attempt to read from the `CoreAsyncRead` into `vec` using vectored
    /// IO operations.
    ///
    /// This method is similar to `poll_read`, but allows data to be read
    /// into multiple buffers using a single operation.
    ///
    /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
    ///
    /// If no data is available for reading, the method returns
    /// `Ok(Async::Pending)` and arranges for the current task (via
    /// `cx.waker()`) to receive a notification when the object becomes
    /// readable or is closed.
    /// By default, this method delegates to using `poll_read` on the first
    /// buffer in `vec`. Objects which support vectored IO should override
    /// this method.
    ///
    fn poll_vectored_read_core(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVecMut])
        -> Poll<usize, Self::Error>
    {
        if let Some(ref mut first_iovec) = vec.get_mut(0) {
            self.poll_read_core(cx, first_iovec)
        } else {
            // `vec` is empty.
            return Ok(Async::Ready(0));
        }
    }
}

/// `std`-less trait to write bytes asynchronously.
///
/// This trait is analogous to the `std::io::Write` trait, but integrates
/// with the asynchronous task system. In particular, the `poll_write`
/// method, unlike `Write::write`, will automatically queue the current task
/// for wakeup and return if data is not yet available, rather than blocking
/// the calling thread.
pub trait CoreAsyncWrite {
    /// TODO
    type Error: CoreIoError;

    /// Attempt to write bytes from `buf` into the object.
    ///
    /// On success, returns `Ok(Async::Ready(num_bytes_written))`.
    ///
    /// If the object is not ready for writing, the method returns
    /// `Ok(Async::Pending)` and arranges for the current task (via
    /// `cx.waker()`) to receive a notification when the object becomes
    /// readable or is closed.
    fn poll_write_core(&mut self, cx: &mut task::Context, buf: &[u8])
        -> Poll<usize, Self::Error>;

    /// Attempt to write bytes from `vec` into the object using vectored
    /// IO operations.
    ///
    /// This method is similar to `poll_write`, but allows data from multiple buffers to be written
    /// using a single operation.
    ///
    /// On success, returns `Ok(Async::Ready(num_bytes_written))`.
    ///
    /// If the object is not ready for writing, the method returns
    /// `Ok(Async::Pending)` and arranges for the current task (via
    /// `cx.waker()`) to receive a notification when the object becomes
    /// readable or is closed.
    ///
    /// By default, this method delegates to using `poll_write` on the first
    /// buffer in `vec`. Objects which support vectored IO should override
    /// this method.
    fn poll_vectored_write_core(&mut self, cx: &mut task::Context, vec: &[&IoVec])
        -> Poll<usize, Self::Error>
    {
        if let Some(ref first_iovec) = vec.get(0) {
            self.poll_write_core(cx, &*first_iovec)
        } else {
            // `vec` is empty.
            return Ok(Async::Ready(0));
        }
    }

    /// Attempt to flush the object, ensuring that any buffered data reach
    /// their destination.
    ///
    /// On success, returns `Ok(Async::Ready(()))`.
    ///
    /// If flushing cannot immediately complete, this method returns
    /// `Ok(Async::Pending)` and arranges for the current task (via
    /// `cx.waker()`) to receive a notification when the object can make
    /// progress towards flushing.
    fn poll_flush_core(&mut self, cx: &mut task::Context) -> Poll<(), Self::Error>;

    /// Attempt to close the object.
    ///
    /// On success, returns `Ok(Async::Ready(()))`.
    ///
    /// If closing cannot immediately complete, this function returns
    /// `Ok(Async::Pending)` and arranges for the current task (via
    /// `cx.waker()`) to receive a notification when the object can make
    /// progress towards closing.
    fn poll_close_core(&mut self, cx: &mut task::Context) -> Poll<(), Self::Error>;
}

impl<'a, T: ?Sized + CoreAsyncRead> CoreAsyncRead for &'a mut T {
    type Error = <T as CoreAsyncRead>::Error;

    unsafe fn initializer_core(&self) -> Initializer {
        (**self).initializer_core()
    }

    fn poll_read_core(&mut self, cx: &mut task::Context, buf: &mut [u8])
        -> Poll<usize, Self::Error>
    {
        (**self).poll_read_core(cx, buf)
    }

    fn poll_vectored_read_core(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVecMut])
        -> Poll<usize, Self::Error>
    {
        (**self).poll_vectored_read_core(cx, vec)
    }
}

impl<'a> CoreAsyncRead for &'a [u8] {
    type Error = MinimalIoError;

    unsafe fn initializer_core(&self) -> Initializer {
        Initializer::nop()
    }

    fn poll_read_core(&mut self, _cx: &mut task::Context, buf: &mut [u8])
        -> Poll<usize, Self::Error>
    {
        let len = cmp::min(self.len(), buf.len());
        let (head, tail) = self.split_at(len);
        buf[..len].copy_from_slice(head);
        *self = tail;
        Ok(Async::Ready(len))
    }
}

if_std! {
    extern crate std;

    use std::boxed::Box;
    use std::io as StdIo;
    use std::vec::Vec;

    // Re-export io::Error so that users don't have to deal
    // with conflicts when `use`ing `futures::io` and `std::io`.
    pub use StdIo::Error as Error;
    pub use StdIo::ErrorKind as ErrorKind;
    pub use StdIo::Result as Result;

    /// Read bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Read` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_read`
    /// method, unlike `Read::read`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncRead {
        /// Determines if this `AsyncRead`er can work with buffers of
        /// uninitialized memory.
        ///
        /// The default implementation returns an initializer which will zero
        /// buffers.
        ///
        /// # Safety
        ///
        /// This method is `unsafe` because and `AsyncRead`er could otherwise
        /// return a non-zeroing `Initializer` from another `AsyncRead` type
        /// without an `unsafe` block.
        #[inline]
        unsafe fn initializer(&self) -> Initializer {
            Initializer::zeroing()
        }

        /// Attempt to read from the `AsyncRead` into `buf`.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object becomes
        /// readable or is closed.
        fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8])
            -> Poll<usize, Error>;

        /// Attempt to read from the `AsyncRead` into `vec` using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_read`, but allows data to be read
        /// into multiple buffers using a single operation.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_read))`.
        ///
        /// If no data is available for reading, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object becomes
        /// readable or is closed.
        /// By default, this method delegates to using `poll_read` on the first
        /// buffer in `vec`. Objects which support vectored IO should override
        /// this method.
        ///
        fn poll_vectored_read(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVecMut])
            -> Poll<usize, Error>
        {
            if let Some(ref mut first_iovec) = vec.get_mut(0) {
                self.poll_read(cx, first_iovec)
            } else {
                // `vec` is empty.
                return Ok(Async::Ready(0));
            }
        }
    }

    /// Write bytes asynchronously.
    ///
    /// This trait is analogous to the `std::io::Write` trait, but integrates
    /// with the asynchronous task system. In particular, the `poll_write`
    /// method, unlike `Write::write`, will automatically queue the current task
    /// for wakeup and return if data is not yet available, rather than blocking
    /// the calling thread.
    pub trait AsyncWrite {
        /// Attempt to write bytes from `buf` into the object.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object becomes
        /// readable or is closed.
        fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8])
            -> Poll<usize, Error>;

        /// Attempt to write bytes from `vec` into the object using vectored
        /// IO operations.
        ///
        /// This method is similar to `poll_write`, but allows data from multiple buffers to be written
        /// using a single operation.
        ///
        /// On success, returns `Ok(Async::Ready(num_bytes_written))`.
        ///
        /// If the object is not ready for writing, the method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object becomes
        /// readable or is closed.
        ///
        /// By default, this method delegates to using `poll_write` on the first
        /// buffer in `vec`. Objects which support vectored IO should override
        /// this method.
        fn poll_vectored_write(&mut self, cx: &mut task::Context, vec: &[&IoVec])
            -> Poll<usize, Error>
        {
            if let Some(ref first_iovec) = vec.get(0) {
                self.poll_write(cx, &*first_iovec)
            } else {
                // `vec` is empty.
                return Ok(Async::Ready(0));
            }
        }

        /// Attempt to flush the object, ensuring that any buffered data reach
        /// their destination.
        ///
        /// On success, returns `Ok(Async::Ready(()))`.
        ///
        /// If flushing cannot immediately complete, this method returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object can make
        /// progress towards flushing.
        fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Error>;

        /// Attempt to close the object.
        ///
        /// On success, returns `Ok(Async::Ready(()))`.
        ///
        /// If closing cannot immediately complete, this function returns
        /// `Ok(Async::Pending)` and arranges for the current task (via
        /// `cx.waker()`) to receive a notification when the object can make
        /// progress towards closing.
        fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Error>;
    }

    impl<T> AsyncRead for T
        where
            T: CoreAsyncRead,
            T::Error: Into<Error>,
    {
        unsafe fn initializer(&self) -> Initializer {
            self.initializer_core()
        }

        fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8])
            -> Poll<usize, Error>
        {
            self.poll_read_core(cx, buf).map_err(Into::into)
        }

        fn poll_vectored_read(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVecMut])
            -> Poll<usize, Error>
        {
            self.poll_vectored_read_core(cx, vec).map_err(Into::into)
        }
    }

    // macro_rules! deref_async_read {
    //     () => {
    //         unsafe fn initializer(&self) -> Initializer {
    //             (**self).initializer()
    //         }

    //         fn poll_read(&mut self, cx: &mut task::Context, buf: &mut [u8])
    //             -> Poll<usize, Error>
    //         {
    //             (**self).poll_read(cx, buf)
    //         }

    //         fn poll_vectored_read(&mut self, cx: &mut task::Context, vec: &mut [&mut IoVecMut])
    //             -> Poll<usize, Error>
    //         {
    //             (**self).poll_vectored_read(cx, vec)
    //         }
    //     }
    // }

    // impl<T: ?Sized + AsyncRead> AsyncRead for Box<T> {
    //     deref_async_read!();
    // }

    // impl<'a, T: ?Sized + AsyncRead> AsyncRead for &'a mut T {
    //     deref_async_read!();
    // }

    /// `unsafe` because the `StdIo::Read` type must not access the buffer
    /// before reading data into it.
    macro_rules! unsafe_delegate_async_read_to_stdio {
        () => {
            unsafe fn initializer(&self) -> Initializer {
                Initializer::nop()
            }

            fn poll_read(&mut self, _: &mut task::Context, buf: &mut [u8])
                -> Poll<usize, Error>
            {
                Ok(Async::Ready(StdIo::Read::read(self, buf)?))
            }
        }
    }

    // impl<'a> AsyncRead for &'a [u8] {
    //     unsafe_delegate_async_read_to_stdio!();
    // }

    impl AsyncRead for StdIo::Repeat {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl<T: AsRef<[u8]>> AsyncRead for StdIo::Cursor<T> {
        unsafe_delegate_async_read_to_stdio!();
    }

    impl<T> AsyncWrite for T where T: CoreAsyncWrite, T::Error: Into<Error> {
        fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8])
            -> Poll<usize, Error>
        {
            self.poll_write_core(cx, buf).map_err(Into::into)
        }

        fn poll_vectored_write(&mut self, cx: &mut task::Context, vec: &[&IoVec])
            -> Poll<usize, Error>
        {
            self.poll_vectored_write_core(cx, vec).map_err(Into::into)
        }

        fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Error> {
            self.poll_flush_core(cx).map_err(Into::into)
        }

        fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Error> {
            self.poll_close_core(cx).map_err(Into::into)
        }
    }

    // macro_rules! deref_async_write {
    //     () => {
    //         fn poll_write(&mut self, cx: &mut task::Context, buf: &[u8])
    //             -> Poll<usize, Error>
    //         {
    //             (**self).poll_write(cx, buf)
    //         }

    //         fn poll_vectored_write(&mut self, cx: &mut task::Context, vec: &[&IoVec])
    //             -> Poll<usize, Error>
    //         {
    //             (**self).poll_vectored_write(cx, vec)
    //         }

    //         fn poll_flush(&mut self, cx: &mut task::Context) -> Poll<(), Error> {
    //             (**self).poll_flush(cx)
    //         }

    //         fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Error> {
    //             (**self).poll_close(cx)
    //         }
    //     }
    // }

    // impl<T: ?Sized + AsyncWrite> AsyncWrite for Box<T> {
    //     deref_async_write!();
    // }

    // impl<'a, T: ?Sized + AsyncWrite> AsyncWrite for &'a mut T {
    //     deref_async_write!();
    // }

    macro_rules! delegate_async_write_to_stdio {
        () => {
            fn poll_write(&mut self, _: &mut task::Context, buf: &[u8])
                -> Poll<usize, Error>
            {
                Ok(Async::Ready(StdIo::Write::write(self, buf)?))
            }

            fn poll_flush(&mut self, _: &mut task::Context) -> Poll<(), Error> {
                Ok(Async::Ready(StdIo::Write::flush(self)?))
            }

            fn poll_close(&mut self, cx: &mut task::Context) -> Poll<(), Error> {
                self.poll_flush(cx)
            }
        }
    }

    impl<'a> AsyncWrite for StdIo::Cursor<&'a mut [u8]> {
        delegate_async_write_to_stdio!();
    }

    impl AsyncWrite for StdIo::Cursor<Vec<u8>> {
        delegate_async_write_to_stdio!();
    }

    impl AsyncWrite for StdIo::Cursor<Box<[u8]>> {
        delegate_async_write_to_stdio!();
    }

    impl AsyncWrite for StdIo::Sink {
        delegate_async_write_to_stdio!();
    }

    impl CoreIoError for Error {
        fn write_zero(msg: &'static str) -> Self {
            Error::new(ErrorKind::WriteZero, msg)
        }

        fn unexpected_eof(msg: &'static str) -> Self {
            Error::new(ErrorKind::UnexpectedEof, msg)
        }
    }
}
