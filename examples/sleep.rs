use asim::time;

#[asim::main]
fn main() {
    let duration = time::Duration::from_days(10);
    println!("The simulated time is now {:?}", time::now());
    time::sleep(duration).await;
    println!("The simulated time is now {:?}", time::now());

    // Ensure as time elapsed
    assert!((time::now() - time::START_TIME) >= duration);
}
