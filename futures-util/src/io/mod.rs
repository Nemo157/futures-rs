//! IO
//!
//! This module contains a number of functions for working with
//! `AsyncRead` and `AsyncWrite` types, including the
//! `AsyncReadExt` and `AsyncWriteExt` traits which add methods
//! to the `AsyncRead` and `AsyncWrite` types.


pub use futures_io::{CoreAsyncRead, CoreAsyncWrite, IoVec, IoVecMut};

pub use self::copy_into::CopyInto;
pub use self::flush::Flush;
pub use self::read::Read;
pub use self::read_exact::ReadExact;
pub use self::close::Close;
pub use self::window::Window;
pub use self::write_all::WriteAll;

// Temporarily removed until AsyncBufRead is implemented
// pub use io::lines::{lines, Lines};
// pub use io::read_until::{read_until, ReadUntil};
// mod lines;
// mod read_until;

mod copy_into;
mod flush;
mod read;
mod read_exact;
mod close;
mod window;
mod write_all;

if_std! {
    use std::vec::Vec;
    use std::boxed::Box;

    pub use futures_io::{AsyncRead, AsyncWrite};

    pub use self::allow_std::AllowStdIo;
    pub use self::read_to_end::ReadToEnd;
    pub use self::split::{ReadHalf, WriteHalf};

    mod allow_std;
    mod read_to_end;
    mod split;
}

/// An extension trait which adds utility methods to `CoreAsyncRead` types.
pub trait AsyncReadExt: CoreAsyncRead {
    /// Creates a future which copies all the bytes from one object to another.
    ///
    /// The returned future will copy all the bytes read from this `AsyncRead` into the
    /// `writer` specified. This future will only complete once the `reader` has hit
    /// EOF and all bytes have been written to and flushed from the `writer`
    /// provided.
    ///
    /// On success the number of bytes is returned and this `AsyncRead` and `writer` are
    /// consumed. On error the error is returned and the I/O objects are consumed as
    /// well.
    #[cfg(feature = "std")]
    fn copy_into<W>(self, writer: W) -> CopyInto<Self, W, Box<[u8]>>
        where W: CoreAsyncWrite,
              Self: Sized,
    {
        self.copy_into_with_buffer(writer, Box::new([0; 2048]))
    }

    /// Creates a future which copies all the bytes from one object to another.
    ///
    /// The returned future will copy all the bytes read from this `AsyncRead` into the
    /// `writer` specified. This future will only complete once the `reader` has hit
    /// EOF and all bytes have been written to and flushed from the `writer`
    /// provided.
    ///
    /// On success the number of bytes is returned and this `AsyncRead` and `writer` are
    /// consumed. On error the error is returned and the I/O objects are consumed as
    /// well.
    fn copy_into_with_buffer<W, B>(self, writer: W, buf: B) -> CopyInto<Self, W, B>
        where W: CoreAsyncWrite,
              B: AsRef<[u8]> + AsMut<[u8]>,
              Self: Sized,
    {
        copy_into::copy_into(self, writer, buf)
    }

    /// Tries to read some bytes directly into the given `buf` in asynchronous
    /// manner, returning a future type.
    ///
    /// The returned future will resolve to both the I/O stream and the buffer
    /// as well as the number of bytes read once the read operation is completed.
    fn read<T>(self, buf: T) -> Read<Self, T>
        where T: AsMut<[u8]>,
              Self: Sized,
    {
        read::read(self, buf)
    }

    /// Creates a future which will read exactly enough bytes to fill `buf`,
    /// returning an error if EOF is hit sooner.
    ///
    /// The returned future will resolve to both the I/O stream as well as the
    /// buffer once the read operation is completed.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded. In the case of success the object will be destroyed and
    /// the buffer will be returned, with all data read from the stream appended to
    /// the buffer.
    fn read_exact<T>(self, buf: T) -> ReadExact<Self, T>
        where T: AsMut<[u8]>,
              Self: Sized,
    {
        read_exact::read_exact(self, buf)
    }

    /// Creates a future which will read all the bytes from this `AsyncRead`.
    ///
    /// In the case of an error the buffer and the object will be discarded, with
    /// the error yielded. In the case of success the object will be destroyed and
    /// the buffer will be returned, with all data read from the stream appended to
    /// the buffer.
    #[cfg(feature = "std")]
    fn read_to_end(self, buf: Vec<u8>) -> ReadToEnd<Self>
        where Self: Sized + AsyncRead,
    {
        read_to_end::read_to_end(self, buf)
    }

    /// Helper method for splitting this read/write object into two halves.
    ///
    /// The two halves returned implement the `Read` and `Write` traits,
    /// respectively.
    #[cfg(feature = "std")]
    fn split(self) -> (ReadHalf<Self>, WriteHalf<Self>)
        where Self: CoreAsyncWrite + Sized,
    {
        split::split(self)
    }
}

impl<T: CoreAsyncRead + ?Sized> AsyncReadExt for T {}

/// An extension trait which adds utility methods to `CoreAsyncWrite` types.
pub trait AsyncWriteExt: CoreAsyncWrite {
    /// Creates a future which will entirely flush this `CoreAsyncWrite` and then return `self`.
    ///
    /// This function will consume `self` if an error occurs.
    fn flush(self) -> Flush<Self>
        where Self: Sized,
    {
        flush::flush(self)
    }

    /// Creates a future which will entirely close this `CoreAsyncWrite` and then return `self`.
    ///
    /// This function will consume the object provided if an error occurs.
    fn close(self) -> Close<Self>
        where Self: Sized,
    {
        close::close(self)
    }

    /// Write a `Buf` into this value, returning how many bytes were written.
    /// Creates a future that will write the entire contents of the buffer `buf` into
    /// this `CoreAsyncWrite`.
    ///
    /// The returned future will not complete until all the data has been written.
    /// The future will resolve to a tuple of `self` and `buf`
    /// (so the buffer can be reused as needed).
    ///
    /// Any error which happens during writing will cause both the stream and the
    /// buffer to be destroyed.
    ///
    /// The `buf` parameter here only requires the `AsRef<[u8]>` trait, which should
    /// be broadly applicable to accepting data which can be converted to a slice.
    /// The `Window` struct is also available in this crate to provide a different
    /// window into a slice if necessary.
    fn write_all<T>(self, buf: T) -> WriteAll<Self, T>
        where T: AsRef<[u8]>,
              Self: Sized,
    {
        write_all::write_all(self, buf)
    }
}

impl<T: CoreAsyncWrite + ?Sized> AsyncWriteExt for T {}
