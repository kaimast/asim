use super::node::Node;

use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering as AtomicOrdering};

use crate::network::{DummyNetworkMessage, Latency, NetworkMessage, Object, ObjectId};
use crate::time::Duration;

/// Network bandwidth in Megabits per second
pub type Bandwidth = u64;

/// Each link consists of two messages queues, one for each direction
pub struct Link<Message: NetworkMessage> {
    identifier: ObjectId,

    queue1: Rc<LinkQueue<Message>>,
    queue2: Rc<LinkQueue<Message>>,

    callback: Box<dyn LinkCallback<Message>>,

    active_queues: AtomicU32,
}

pub trait LinkCallback<Message: NetworkMessage> {
    fn link_became_active(&self, _link: &Link<Message>) {}
    fn link_became_inactive(&self, _link: &Link<Message>) {}
}

#[derive(Default)]
pub struct DummyLinkCallback {}

impl LinkCallback<DummyNetworkMessage> for DummyLinkCallback {}

impl<Message: NetworkMessage> Link<Message> {
    pub(super) fn new(
        latency: Latency,
        node1: Rc<Node<Message>>,
        node2: Rc<Node<Message>>,
        callback: Box<dyn LinkCallback<Message>>,
    ) -> Self {
        let queue1 = Rc::new(LinkQueue::new(latency, node1.clone(), node2.clone()));

        let queue2 = Rc::new(LinkQueue::new(latency, node2, node1));

        let active_queues = AtomicU32::new(0);

        Self {
            identifier: ObjectId::random(),
            queue1,
            queue2,
            active_queues,
            callback,
        }
    }

    /// Does the link currently have any messages in transit?
    pub fn is_active(&self) -> bool {
        self.active_queues.load(AtomicOrdering::Relaxed) > 0
    }

    /// Get the two nodes connected with this link
    /// Always sorted by smallest id first
    pub fn get_nodes(&self) -> (&Rc<Node<Message>>, &Rc<Node<Message>>) {
        let node1 = self.queue1.get_source();
        let node2 = self.queue1.get_destination();

        match node1.get_identifier().cmp(&node2.get_identifier()) {
            Ordering::Less => (node1, node2),
            Ordering::Greater => (node2, node1),
            Ordering::Equal => panic!("Invalid state"),
        }
    }

    pub fn send(self_ptr: &Rc<Link<Message>>, source: ObjectId, message: Message) {
        if self_ptr.queue1.get_source().get_identifier() == source {
            LinkQueue::send(self_ptr.queue1.clone(), self_ptr.clone(), message);
        } else if self_ptr.queue2.get_source().get_identifier() == source {
            LinkQueue::send(self_ptr.queue2.clone(), self_ptr.clone(), message);
        } else {
            panic!("Invalid state");
        }
    }

    /// Get the number of all messages ever sent through this link
    pub fn num_total_messages(&self) -> u64 {
        self.queue1
            .total_message_count
            .load(AtomicOrdering::Relaxed)
            + self
                .queue2
                .total_message_count
                .load(AtomicOrdering::Relaxed)
    }
}

impl<Message: NetworkMessage> Object for Link<Message> {
    fn get_identifier(&self) -> ObjectId {
        self.identifier
    }
}

struct LinkQueue<Message: NetworkMessage> {
    latency: Duration,

    source: Rc<Node<Message>>,
    dest: Rc<Node<Message>>,

    current_message_count: AtomicU32,
    total_message_count: AtomicU64,
}

#[allow(dead_code)]
pub fn get_size_delay(size: u64, bandwidth: Bandwidth) -> Duration {
    // Converts bandwidth bits / microsecond and size to bits
    let micros = (size * 8 * 1000 * 1000) / (bandwidth * 1024 * 1024);

    Duration::from_micros(micros)
}

impl<Message: NetworkMessage> LinkQueue<Message> {
    fn new(latency: Latency, source: Rc<Node<Message>>, dest: Rc<Node<Message>>) -> Self {
        let current_message_count = AtomicU32::new(0);
        let total_message_count = AtomicU64::new(0);

        Self {
            latency,
            total_message_count,
            source,
            dest,
            current_message_count,
        }
    }

    fn send(
        self_ptr: Rc<LinkQueue<Message>>,
        link: Rc<Link<Message>>,
        message: Message,
    ) -> (bool, Duration) {
        let latency = self_ptr.latency;
        //let size_delay = Self::get_size_delay(message.get_size(), self_ptr.bandwidth);

        let was_empty = {
            self_ptr
                .total_message_count
                .fetch_add(1, AtomicOrdering::Relaxed);
            let prev = self_ptr
                .current_message_count
                .fetch_add(1, AtomicOrdering::Relaxed);
            prev == 0
        };

        if was_empty {
            let prev = link.active_queues.fetch_add(1, AtomicOrdering::SeqCst);

            if prev == 0 {
                link.callback.link_became_active(&link);
            }
        }

        crate::spawn(async move {
            // Sleep for how long the latency delays a message
            if !latency.is_zero() {
                crate::time::sleep(latency).await;
            }

            //TODO re-add link bandwidth

            let notify_delivery_fn = {
                let self_ptr = self_ptr.clone();

                Box::new(move || {
                    let prev = self_ptr
                        .current_message_count
                        .fetch_sub(1, AtomicOrdering::SeqCst);
                    assert!(prev > 0);

                    // This was the message in this queue, so mark it as inactive
                    if prev == 1 {
                        let prev = link.active_queues.fetch_sub(1, AtomicOrdering::SeqCst);

                        // No queues are active anymore, so mark the link as inactive
                        if prev == 1 {
                            link.callback.link_became_inactive(&link);
                        }
                    }
                })
            };

            let dst = self_ptr.get_destination();
            dst.deliver_message(
                self_ptr.source.get_identifier(),
                message,
                notify_delivery_fn,
            );
        });

        (was_empty, latency)
    }

    fn get_source(&self) -> &Rc<Node<Message>> {
        &self.source
    }

    fn get_destination(&self) -> &Rc<Node<Message>> {
        &self.dest
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::network::link::{get_size_delay, DummyLinkCallback, Link};
    use crate::network::node::{DummyNodeCallback, Node};
    use crate::network::{DummyNetworkMessage, Object};
    use crate::time::Duration;

    #[test]
    fn is_active() {
        let asim = Rc::new(crate::Runtime::default());
        let (node1, node2, link);

        {
            let _ctx = asim.with_context();
            node1 = Node::new(1000, Box::new(DummyNodeCallback::default()));
            node2 = Node::new(1000, Box::new(DummyNodeCallback::default()));

            link = Rc::new(Link::new(
                Duration::from_millis(50),
                node1.clone(),
                node2.clone(),
                Box::new(DummyLinkCallback::default()),
            ));
        }

        {
            let _ctx = asim.with_context();
            Link::send(
                &link,
                node2.get_identifier(),
                DummyNetworkMessage::default(),
            );
        }

        // Sending messages is a two step process (link latency + bandwidth)
        asim.execute_tasks();

        assert!(link.is_active());

        asim.get_timer().advance();
        asim.execute_tasks();
        asim.execute_tasks();

        assert!(!link.is_active());
    }

    #[test]
    fn delay() {
        // 3 Mbs
        let size = 3 * 1024 * 1024;

        // 24 Mbits
        let bandwidth = 24;

        let delay = get_size_delay(size, bandwidth);

        // 8*3 == 24
        assert_eq!(delay, Duration::from_seconds(1));
    }
}
