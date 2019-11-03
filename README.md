# scoped-tokio

[![travis](https://img.shields.io/travis/mexus/scoped-tokio.svg)](https://travis-ci.org/mexus/scoped-tokio)
[![crates.io](https://img.shields.io/crates/v/scoped-tokio.svg)](https://crates.io/crates/scoped-tokio)
[![docs.rs](https://docs.rs/scoped-tokio/badge.svg)](https://docs.rs/scoped-tokio)

[Master docs](https://mexus.github.io/scoped-tokio/scoped-tokio/index.html)!

Scoped tokio runtime.

This is a minimalistic library, providing only the minimal necessary subset of executors API,
but enabling you to spawn non-static futures on executor, like the following:

```rust
use scoped_tokio::threadpool::scoped;

async fn increase1(counter: &mut usize) {
    *counter += 1;
}

fn increase2(counter: &mut usize) -> impl Future<Output = ()> + '_ {
    *counter += 2;
    async {}
}

fn run(counter: &mut usize) -> usize {
    let mut counter2 = 0;
    let mut string = String::new();
    scoped(|s| {
        s.spawn(increase1(counter));
        s.spawn(increase2(&mut counter2));
        string = "Hmpf...".into();
        s.spawn(async { string = "Hooray!".into() });

        // The following line won't compile since `string` is mutable borrowed.
        // string = "Hmpf...".into();
    });
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
