use asim::{Runtime, time};

fn main() {
    env_logger::init().unwrap();

    let rt = Runtime::new();

    rt.block_on(async {
        let duration = time::Duration::from_days(10);
        println!("The simulated time is now {:?}", time::now());
        time::sleep(duration).await;
        println!("The simulated time is now {:?}", time::now());

        // Ensure as time elapsed
        assert!((time::now() - time::START_TIME) >= duration);
    });
}
