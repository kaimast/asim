use std::cell::RefCell;
use std::rc::Rc;

use asim::network::{Bandwidth, Latency, NetworkMessage, ObjectId};
use asim::sync::oneshot;
use asim::time::Duration;

#[derive(Clone, Debug)]
struct Message {}

struct NodeCallback {}

struct NodeData {
    notifier: RefCell<Option<oneshot::Sender<()>>>,
}
impl asim::network::NodeData for NodeData {}

type Node = asim::network::Node<Message, NodeData>;

impl NetworkMessage for Message {
    /// Every message is 1kb
    fn get_size(&self) -> u64 {
        20 * 1024 * 1024
    }
}

#[async_trait::async_trait(?Send)]
impl asim::network::NodeCallback<Message, NodeData> for NodeCallback {
    async fn handle_message(&self, node: &Rc<Node>, _source: ObjectId, _message: Message) {
        node.get_data()
            .notifier
            .borrow_mut()
            .take()
            .unwrap()
            .send(())
            .unwrap();
    }
}

#[derive(Default)]
pub struct LinkCallback {}
impl asim::network::LinkCallback<Message, NodeData> for LinkCallback {}

#[asim::test]
async fn main() {
    let latency = Latency::from_seconds(3);
    let bandwidth = Bandwidth::from_megabytes_per_second(2);

    let (notify_sender, notify_receiver) = oneshot::channel();

    let sender_data = NodeData {
        notifier: Default::default(),
    };
    let receiver_data = NodeData {
        notifier: RefCell::new(Some(notify_sender)),
    };

    // Create two nodes and connect them
    let sender = Node::new(bandwidth, sender_data, Box::new(NodeCallback {}));
    let receiver = Node::new(bandwidth, receiver_data, Box::new(NodeCallback {}));

    Node::connect(sender.clone(), receiver, latency, Box::new(LinkCallback {}));

    let start = asim::time::now();
    sender.broadcast(Message {}, None);

    notify_receiver.await.unwrap();

    let elapsed = asim::time::now() - start;

    // Transfer should take 10 seconds
    // and latency adds another 3 seconds
    assert_eq!(elapsed, Duration::from_seconds(13));
}
