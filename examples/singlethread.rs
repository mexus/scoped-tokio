use scoped_tokio::current_thread::{scoped, Scope};
use std::future::Future;

fn increase1(counter: &mut usize) -> impl Future<Output = ()> + '_ {
    *counter += 1;
    async {}
}

// Unfortunately, this lifetime handling stuff is too complicated for async-await generators, so
// `async fn` can not be used here.
fn increase2<'env, 'cnt>(
    scope: &mut Scope<'env>,
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
    })
    .unwrap();
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
