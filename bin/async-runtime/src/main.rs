use std::time::Duration;

use async_runtime::{new_executor_and_spawner, TimerFuture};

fn main() {
    let (executor, spawner) = new_executor_and_spawner();

    spawner.spawn(async {
        println!("start!!!");
        TimerFuture::new(Duration::new(2, 0)).await;
        println!("done!!!");
    });

    drop(spawner);

    executor.run();
}
