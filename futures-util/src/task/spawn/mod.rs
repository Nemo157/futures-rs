#[cfg(feature = "std")]
use std::{any::Any, sync::Arc};

use futures_core::task::{LocalSpawn, Spawn};

#[cfg(feature = "compat")] use crate::compat::Compat;

#[cfg(feature = "std")]
use crate::future::{FutureExt, RemoteHandle};
#[cfg(feature = "alloc")]
use futures_core::future::{Future, FutureObj, LocalFutureObj};
#[cfg(feature = "alloc")]
use futures_core::task::SpawnError;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;

#[cfg(feature = "std")]
mod catch_unwind;
#[cfg(feature = "std")]
pub use self::catch_unwind::CatchUnwind;

impl<Sp: ?Sized> SpawnExt for Sp where Sp: Spawn {}
impl<Sp: ?Sized> LocalSpawnExt for Sp where Sp: LocalSpawn {}

/// Extension trait for `Spawn`.
pub trait SpawnExt: Spawn {
    /// Spawns a task that polls the given future with output `()` to
    /// completion.
    ///
    /// This method returns a [`Result`] that contains a [`SpawnError`] if
    /// spawning fails.
    ///
    /// You can use [`spawn_with_handle`](SpawnExt::spawn_with_handle) if
    /// you want to spawn a future with output other than `()` or if you want
    /// to be able to await its completion.
    ///
    /// Note this method will eventually be replaced with the upcoming
    /// `Spawn::spawn` method which will take a `dyn Future` as input.
    /// Technical limitations prevent `Spawn::spawn` from being implemented
    /// today. Feel free to use this method in the meantime.
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::executor::ThreadPool;
    /// use futures::task::SpawnExt;
    ///
    /// let mut executor = ThreadPool::new().unwrap();
    ///
    /// let future = async { /* ... */ };
    /// executor.spawn(future).unwrap();
    /// ```
    #[cfg(feature = "alloc")]
    fn spawn<Fut>(&mut self, future: Fut) -> Result<(), SpawnError>
    where
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.spawn_obj(FutureObj::new(Box::new(future)))
    }

    /// Spawns a task that polls the given future to completion and returns a
    /// future that resolves to the spawned future's output.
    ///
    /// This method returns a [`Result`] that contains a [`RemoteHandle`], or, if
    /// spawning fails, a [`SpawnError`]. [`RemoteHandle`] is a future that
    /// resolves to the output of the spawned future.
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::executor::ThreadPool;
    /// use futures::future;
    /// use futures::task::SpawnExt;
    ///
    /// let mut executor = ThreadPool::new().unwrap();
    ///
    /// let future = future::ready(1);
    /// let join_handle_fut = executor.spawn_with_handle(future).unwrap();
    /// assert_eq!(executor.run(join_handle_fut), 1);
    /// ```
    #[cfg(feature = "std")]
    fn spawn_with_handle<Fut>(
        &mut self,
        future: Fut
    ) -> Result<RemoteHandle<Fut::Output>, SpawnError>
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send,
    {
        let (future, handle) = future.remote_handle();
        self.spawn(future)?;
        Ok(handle)
    }

    /// Wraps self in a new spawner that will catch any panics occurring in the
    /// spawned futures and route them to the supplied closure.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// # futures::executor::block_on(async {
    /// use std::sync::{Arc, Mutex};
    /// use futures::task::SpawnExt;
    ///
    /// let errors = Arc::new(Mutex::new(vec![]));
    ///
    /// let pool = futures::executor::ThreadPool::new().unwrap();
    ///
    /// let mut spawner = pool.clone().catch_unwind({
    ///     let errors = errors.clone();
    ///     move |err| {
    ///         let err = *err.downcast_ref::<&'static str>().unwrap();
    ///         errors.lock().unwrap().push(err);
    ///     }
    /// });
    ///
    /// spawner.spawn(async move {
    ///     panic!("boom");
    /// }).unwrap();
    //
    // TODO: We want to wait until the pool has completed running all
    // futures, but it doesn't provide an API to do so
    /// # std::thread::sleep(std::time::Duration::from_millis(100));
    ///
    /// assert_eq!(&*errors.lock().unwrap(), &["boom"]);
    /// # });
    /// ```
    #[cfg(feature = "std")]
    fn catch_unwind<F>(self, f: F) -> CatchUnwind<Self, F>
        where Self: Sized,
              F: Fn(Box<dyn Any + Send + 'static>) + Send + Sync + 'static,
    {
        CatchUnwind::new(self, Arc::new(f))
    }

    /// Wraps a [`Spawn`] and makes it usable as a futures 0.1 `Executor`.
    /// Requires the `compat` feature to enable.
    #[cfg(feature = "compat")]
    fn compat(self) -> Compat<Self>
        where Self: Sized,
    {
        Compat::new(self)
    }
}

/// Extension trait for `LocalSpawn`.
pub trait LocalSpawnExt: LocalSpawn {
    /// Spawns a task that polls the given future with output `()` to
    /// completion.
    ///
    /// This method returns a [`Result`] that contains a [`SpawnError`] if
    /// spawning fails.
    ///
    /// You can use [`spawn_with_handle`](SpawnExt::spawn_with_handle) if
    /// you want to spawn a future with output other than `()` or if you want
    /// to be able to await its completion.
    ///
    /// Note this method will eventually be replaced with the upcoming
    /// `Spawn::spawn` method which will take a `dyn Future` as input.
    /// Technical limitations prevent `Spawn::spawn` from being implemented
    /// today. Feel free to use this method in the meantime.
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::executor::LocalPool;
    /// use futures::task::LocalSpawnExt;
    ///
    /// let executor = LocalPool::new();
    /// let mut spawner = executor.spawner();
    ///
    /// let future = async { /* ... */ };
    /// spawner.spawn_local(future).unwrap();
    /// ```
    #[cfg(feature = "alloc")]
    fn spawn_local<Fut>(&mut self, future: Fut) -> Result<(), SpawnError>
    where
        Fut: Future<Output = ()> + 'static,
    {
        self.spawn_local_obj(LocalFutureObj::new(Box::new(future)))
    }

    /// Spawns a task that polls the given future to completion and returns a
    /// future that resolves to the spawned future's output.
    ///
    /// This method returns a [`Result`] that contains a [`RemoteHandle`], or, if
    /// spawning fails, a [`SpawnError`]. [`RemoteHandle`] is a future that
    /// resolves to the output of the spawned future.
    ///
    /// ```
    /// #![feature(async_await, await_macro, futures_api)]
    /// use futures::executor::LocalPool;
    /// use futures::future;
    /// use futures::task::LocalSpawnExt;
    ///
    /// let mut executor = LocalPool::new();
    /// let mut spawner = executor.spawner();
    ///
    /// let future = future::ready(1);
    /// let join_handle_fut = spawner.spawn_local_with_handle(future).unwrap();
    /// assert_eq!(executor.run_until(join_handle_fut), 1);
    /// ```
    #[cfg(feature = "std")]
    fn spawn_local_with_handle<Fut>(
        &mut self,
        future: Fut
    ) -> Result<RemoteHandle<Fut::Output>, SpawnError>
    where
        Fut: Future + 'static,
    {
        let (future, handle) = future.remote_handle();
        self.spawn_local(future)?;
        Ok(handle)
    }
}
