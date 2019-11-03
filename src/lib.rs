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
//! use scoped_tokio::threadpool::scoped;
//! # use std::future::Future;
//!
//! async fn increase1(counter: &mut usize) {
//!     *counter += 1;
//! }
//!
//! fn increase2(counter: &mut usize) -> impl Future<Output = ()> + '_ {
//!     *counter += 2;
//!     async {}
//! }
//!
//! fn run(counter: &mut usize) -> usize {
//!     let mut counter2 = 0;
//!     let mut string = String::new();
//!     scoped(|s| {
//!         s.spawn(increase1(counter));
//!         s.spawn(increase2(&mut counter2));
//!         string = "Hmpf...".into();
//!         s.spawn(async { string = "Hooray!".into() });
//!
//!         // The following line won't compile since `string` is mutable borrowed.
//!         // string = "Hmpf...".into();
//!     });
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
    use tokio::runtime::current_thread::{RunError, Runtime};

    /// Boxed (and pinned) future without a `Send` requirement.
    pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

    /// Limited execution scope.
    pub struct Scope<'a> {
        rt: &'a mut Runtime,
    }

    impl<'a> Scope<'a> {
        /// Spawns a future on the executor.
        ///
        /// Similar to [`tokio::runtime::current_thread::Runtime::spawn`], but doesn't require the
        /// future to be `'static`.
        ///
        /// # Notice
        ///
        /// The future is boxed before sending it to the executor!
        pub fn spawn<F>(&mut self, f: F)
        where
            F: Future<Output = ()> + 'a,
        {
            self.spawn_boxed(Box::pin(f))
        }

        /// Spawns a future on the executor.
        ///
        /// Similar to [`tokio::runtime::current_thread::Runtime::spawn`], but doesn't require the
        /// future to be `'static`.
        pub fn spawn_boxed(&mut self, f: LocalBoxFuture<'a, ()>) {
            // We guarantee that the future can not outlive the `scoped` call by triggering
            // `Runtime::run` explicitely at the end of the scope.
            let boxed: LocalBoxFuture<'static, ()> = unsafe { transmute(f) };
            self.rt.spawn(boxed);
        }

        /// Blocks on a future.
        ///
        /// Simply calls [`tokio::runtime::current_thread::Runtime::block_on`].
        pub fn block_on<F>(&mut self, f: F) -> F::Output
        where
            F: Future,
        {
            self.rt.block_on(f)
        }
    }

    /// Creates a scope for futures execution.
    pub fn scoped<F>(f: F) -> Result<(), RunError>
    where
        F: FnOnce(&Scope),
    {
        let mut rt = Runtime::new().expect("Can't build current-thread runtime");
        let scope = Scope { rt: &mut rt };
        f(&scope);
        // The safety happens here :)
        rt.run()
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
        _pd: PhantomData<&'env ()>,
    }

    impl<'env> Scope<'env> {
        /// Spawns a future on the executor.
        ///
        /// Similar to [`tokio::runtime::Runtime::spawn`], but doesn't require the future to be
        /// `'static`.
        ///
        /// # Notice
        ///
        /// The future is boxed before sending it to the executor!
        pub fn spawn<F>(&mut self, f: F)
        where
            F: Future<Output = ()> + Send + 'env,
        {
            self.spawn_boxed(Box::pin(f))
        }

        /// Spawns a boxed future onto the executor.
        ///
        /// Similar to [`tokio::runtime::Runtime::spawn`], but doesn't require the future to be
        /// `'static`.
        pub fn spawn_boxed(&mut self, f: BoxFuture<'env, ()>) {
            // We guarantee that the future can not outlive the `scoped` call by triggering
            // `Runtime::shutdown_on_idle` explicitely at the end of the scope.
            let boxed: BoxFuture<'static, ()> = unsafe { transmute(f) };
            self.rt.spawn(boxed);
        }

        /// Blocks on a future.
        ///
        /// Simply calls [`tokio::runtime::Runtime::block_on`].
        pub fn block_on<F>(&mut self, f: F) -> F::Output
        where
            F: Future,
        {
            self.rt.block_on(f)
        }
    }

    /// Creates a scope for futures execution.
    pub fn scoped<'env, F>(f: F)
    where
        F: FnOnce(&mut Scope<'env>),
    {
        let rt = Runtime::new().expect("Can't build threadpool runtime");
        let mut scope = Scope::<'env> {
            rt,
            _pd: PhantomData,
        };
        f(&mut scope);
        let rt = scope.rt;
        // The safety happens here :)
        rt.shutdown_on_idle()
    }
}
