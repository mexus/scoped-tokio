//! [![travis](https://img.shields.io/travis/mexus/scoped-tokio.svg)](https://travis-ci.org/mexus/scoped-tokio)
//! [![crates.io](https://img.shields.io/crates/v/scoped-tokio.svg)](https://crates.io/crates/scoped-tokio)
//! [![docs.rs](https://docs.rs/scoped-tokio/badge.svg)](https://docs.rs/scoped-tokio)
//!
//! [Master docs](https://mexus.github.io/scoped-tokio/scoped-tokio/index.html)!
//!
//! Scoped tokio runtime.
//!
//! This is a minimalistic library, providing only the minimal necessary subset of executors API,
//! but enabling you to spawn non-static futures on executor, like the following:
//!
//! ```rust
//! use scoped_tokio::threadpool::{scoped, Scope};
//! # use std::future::Future;
//!
//! fn increase1(counter: &mut usize) -> impl Future<Output = ()> + '_ {
//!     *counter += 1;
//!     async {}
//! }
//!
//! // Unfortunately, this lifetime handling stuff is too complicated for async-await generators, so
//! // `async fn` can not be used here.
//! fn increase2<'env, 'cnt>(
//!     scope: &Scope<'env>,
//!     counter: &'cnt mut usize,
//! ) -> impl Future<Output = ()> + 'cnt
//! where
//!     'cnt: 'env,
//! {
//!     scope.spawn(async move {
//!         *counter += 2;
//!     });
//!     async { () }
//! }
//!
//! async fn set_string(s: &mut String) {
//!     *s = "Hooray!".into();
//! }
//!
//! fn run(counter: &mut usize) -> usize {
//!     let mut counter2 = 0;
//!     let mut string = String::new();
//!     let res = scoped(|s| {
//!         s.spawn(increase1(counter));
//!         s.spawn_ctx(|ctx| increase2(ctx, &mut counter2));
//!         string = "Hmpf...".into();
//!         s.spawn(set_string(&mut string));
//!
//!         // // The following line won't compile since `string` is mutable borrowed.
//!         // string = "Race?".into();
//!
//!         async { 10 }
//!     });
//!     assert_eq!(res, 10);
//!     assert_eq!(string, "Hooray!");
//!     counter2
//! }
//!
//! fn main() {
//!     let mut counter = 0;
//!     let counter2 = run(&mut counter);
//!     assert_eq!(counter, 1);
//!     assert_eq!(counter2, 2);
//!     println!("counter = {}", counter);
//!     println!("counter2 = {}", counter2);
//! }
//! ```
//!
//! No more `Arc<Mutex<_>>` and `Rc<RefCell<_>>` just to fight the borrow checker when you don't
//! really need them :)
//!
//! ## License
//!
//! Licensed under either of
//!
//!  * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
//!  * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
//!
//! at your option.
//!
//! ### Contribution
//!
//! Unless you explicitly state otherwise, any contribution intentionally submitted
//! for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
//! additional terms or conditions.

#![deny(missing_docs)]

use futures_core::Future;
use std::mem::transmute;
use std::pin::Pin;

/// Current-threaded scoped executor.
pub mod current_thread {
    use super::*;
    use std::marker::PhantomData;
    use tokio::runtime::current_thread::{RunError, Runtime};

    /// Boxed (and pinned) future without a `Send` requirement.
    pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

    /// Limited execution scope.
    pub struct Scope<'scope> {
        rt: Runtime,
        _pd: PhantomData<&'scope mut ()>,
    }

    impl<'scope> Scope<'scope> {
        /// Spawns a future on the executor.
        ///
        /// Similar to [`tokio::runtime::current_thread::Runtime::spawn`], but doesn't require the
        /// future to be `'static`.
        ///
        /// # Notice
        ///
        /// The future is boxed before sending it to the executor!
        pub fn spawn<'local, F>(&'local mut self, f: F)
        where
            F: Future<Output = ()> + 'local,
            'local: 'scope,
        {
            self.spawn_boxed(Box::pin(f))
        }

        /// Spawns a future on the executor.
        ///
        /// Similar to [`tokio::runtime::current_thread::Runtime::spawn`], but doesn't require the
        /// future to be `'static`.
        pub fn spawn_boxed<'local>(&'local mut self, f: LocalBoxFuture<'local, ()>)
        where
            'local: 'scope,
        {
            // We guarantee that the future can not outlive the `scoped` call by triggering
            // `Runtime::run` explicitely at the end of the scope.
            let boxed: LocalBoxFuture<'static, ()> = unsafe { transmute(f) };
            self.rt.spawn(boxed);
        }

        /// Blocks on a future.
        ///
        /// Simply calls [`tokio::runtime::current_thread::Runtime::block_on`].
        pub fn block_on<'local, F>(&'local mut self, f: F) -> F::Output
        where
            F: Future + 'local,
        {
            self.rt.block_on(f)
        }
    }

    /// Creates a scope for futures execution.
    pub fn scoped<'scope, F>(f: F) -> Result<(), RunError>
    where
        for<'a> F: FnOnce(&'a mut Scope<'scope>),
    {
        let rt = Runtime::new().expect("Can't build current-thread runtime");
        let mut scope = Scope {
            rt,
            _pd: PhantomData,
        };
        f(&mut scope);
        // The safety happens here :)
        scope.rt.run()
    }
}

/// Multithreaded scoped executor.
pub mod threadpool {
    use super::*;
    use std::marker::PhantomData;
    use tokio::runtime::Runtime;

    /// Boxed (and pinned) future with a `Send` requirement.
    pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a + Send>>;

    /// Limited execution scope.
    pub struct Scope<'env> {
        rt: Runtime,
        _pd: PhantomData<&'env mut &'env ()>,
    }

    impl<'env> Scope<'env> {
        /// Spawns a future on the executor, providing a `Scope` as a context for creating the future.
        ///
        /// Similar to [`tokio::runtime::Runtime::spawn`], but doesn't require the future to be
        /// `'static` and provides a reference to the current [`Scope`] as a closure argument.
        ///
        /// # Notice
        ///
        /// The future is boxed before sending it to the executor!
        pub fn spawn_ctx<F, Fut>(&self, f: F)
        where
            F: FnOnce(&Scope<'env>) -> Fut + Send + 'env,
            Fut: Future<Output = ()> + Send + 'env,
        {
            let fut = f(self);
            self.spawn_boxed(Box::pin(fut))
        }

        /// Spawns a future on the executor.
        ///
        /// Similar to [`tokio::runtime::Runtime::spawn`], but doesn't require the future to be
        /// `'static`.
        ///
        /// # Notice
        ///
        /// The future is boxed before sending it to the executor!
        pub fn spawn<Fut>(&self, fut: Fut)
        where
            Fut: Future<Output = ()> + Send + 'env,
        {
            self.spawn_boxed(Box::pin(fut))
        }

        /// Spawns a boxed future onto the executor, providing a `Scope` as a context for creating the future.
        ///
        /// Similar to [`tokio::runtime::Runtime::spawn`], but doesn't require the future to be
        /// `'static` and provides a reference to the current [`Scope`] as a closure argument.
        pub fn spawn_boxed_ctx<F>(&self, f: F)
        where
            F: FnOnce(&Scope<'env>) -> BoxFuture<'env, ()>,
            F: Send + 'env,
        {
            let fut = f(self);
            self.spawn_boxed(fut)
        }

        /// Spawns a boxed future onto the executor.
        ///
        /// Similar to [`tokio::runtime::Runtime::spawn`], but doesn't require the future to be
        /// `'static`.
        pub fn spawn_boxed(&self, f: BoxFuture<'env, ()>) {
            // We guarantee that the future can not outlive the `scoped` call by triggering
            // `Runtime::shutdown_on_idle` explicitely at the end of the scope.
            let boxed: BoxFuture<'static, ()> = unsafe { transmute(f) };
            self.rt.spawn(boxed);
        }

        /// Blocks on a future.
        ///
        /// Simply calls [`tokio::runtime::Runtime::block_on`].
        pub fn block_on<F>(&self, f: F) -> F::Output
        where
            F: Future,
        {
            self.rt.block_on(f)
        }
    }

    /// Creates a scope for futures execution.
    pub fn scoped<'env, F, Fut>(f: F) -> Fut::Output
    where
        F: FnOnce(&Scope<'env>) -> Fut,
        Fut: Future,
    {
        let rt = Runtime::new().expect("Can't build threadpool runtime");
        let scope = Scope::<'env> {
            rt,
            _pd: PhantomData,
        };
        let fut = f(&scope);
        let rt = scope.rt;
        let res = rt.block_on(fut);
        // The safety happens here :)
        rt.shutdown_on_idle();
        res
    }
}
