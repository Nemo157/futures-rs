use std::{any::Any, sync::Arc};

use futures_core::{future::FutureObj, task::{Spawn, SpawnError}};

use crate::{future::FutureExt, try_future::TryFutureExt, task::SpawnExt};

/// Spawn for the [`catch_unwind`](SpawnExt::catch_unwind) combinator.
#[derive(Clone, Debug)]
pub struct CatchUnwind<Sp, F>
where
    Sp: Spawn,
    F: Fn(Box<dyn Any + Send + 'static>) + Send + Sync + ?Sized + 'static,
{
    spawn: Sp,
    f: Arc<F>,
}

impl<Sp, F> CatchUnwind<Sp, F>
where
    Sp: Spawn,
    F: Fn(Box<dyn Any + Send + 'static>) + Send + Sync + ?Sized + 'static,
{
    pub(crate) fn new(spawn: Sp, f: Arc<F>) -> Self {
        Self { spawn, f }
    }
}

impl<Sp, F> Spawn for CatchUnwind<Sp, F>
where
    Sp: Spawn,
    F: Fn(Box<dyn Any + Send + 'static>) + Send + Sync + ?Sized + 'static,
{
    fn spawn_obj(&mut self, future: FutureObj<'static, ()>) -> Result<(), SpawnError>
    {
        let f = self.f.clone();
        self.spawn.spawn(future.catch_unwind().unwrap_or_else(move |err| f(err)))
    }

    fn status(&self) -> Result<(), SpawnError> {
        self.spawn.status()
    }
}
