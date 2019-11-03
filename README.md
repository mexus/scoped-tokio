# scoped-tokio

[![travis](https://img.shields.io/travis/mexus/scoped-tokio.svg)](https://travis-ci.org/mexus/scoped-tokio)
[![crates.io](https://img.shields.io/crates/v/scoped-tokio.svg)](https://crates.io/crates/scoped-tokio)
[![docs.rs](https://docs.rs/scoped-tokio/badge.svg)](https://docs.rs/scoped-tokio)

[Master docs](https://mexus.github.io/scoped-tokio/scoped-tokio/index.html)!

Scoped tokio runtime.

This is a minimalistic library, providing only the minimal necessary subset of executors API,
but enabling you to spawn non-static futures on executor, like the following:

```rust
use scoped_tokio::threadpool::{scoped, Scope};

fn increase1(counter: &mut usize) -> impl Future<Output = ()> + '_ {
    *counter += 1;
    async {}
}

// Unfortunately, this lifetime handling stuff is too complicated for async-await generators, so
// `async fn` can not be used here.
fn increase2<'env, 'cnt>(
    scope: &Scope<'env>,
    counter: &'cnt mut usize,
) -> impl Future<Output = ()> + 'cnt
where
    'cnt: 'env,
{
    scope.spawn(async move {
        *counter += 2;
    });
    async { () }
}

async fn set_string(s: &mut String) {
    *s = "Hooray!".into();
}

fn run(counter: &mut usize) -> usize {
    let mut counter2 = 0;
    let mut string = String::new();
    let res = scoped(|s| {
        s.spawn(increase1(counter));
        s.spawn_ctx(|ctx| increase2(ctx, &mut counter2));
        string = "Hmpf...".into();
        s.spawn(set_string(&mut string));

        // // The following line won't compile since `string` is mutably borrowed.
        // string = "Race?".into();

        async { 10 }
    });
    assert_eq!(res, 10);
    assert_eq!(string, "Hooray!");
    counter2
}

fn main() {
    let mut counter = 0;
    let counter2 = run(&mut counter);
    assert_eq!(counter, 1);
    assert_eq!(counter2, 2);
    println!("counter = {}", counter);
    println!("counter2 = {}", counter2);
}
```

No more `Arc<Mutex<_>>` and `Rc<RefCell<_>>` just to fight the borrow checker when you don't
really need them :)

### License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

License: MIT/Apache-2.0
