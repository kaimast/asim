/// Two simba tasks that interact with each other
use asim::sync::oneshot;

#[asim::main]
async fn main() {
    println!("This will print Ping first and then Pong");

    let (ping_sender, ping_receiver) = oneshot::channel();
    let (pong_sender, pong_receiver) = oneshot::channel();

    asim::spawn(async move {
        ping_receiver.await.unwrap();
        println!("Pong!");
        pong_sender.send(()).unwrap();
    });

    asim::spawn(async move {
        println!("Ping?");
        ping_sender.send(()).unwrap();
    });

    pong_receiver.await.unwrap();
}
