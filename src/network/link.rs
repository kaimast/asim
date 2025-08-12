use super::node::Node;

use std::cmp::Ordering;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering as AtomicOrdering};

use crate::network::node::{DummyNodeData, NodeData};
use crate::network::{DummyNetworkMessage, Latency, NetworkMessage, Object, ObjectId};
use crate::time::Duration;

/// Each link consists of two messages queues, one for each direction
pub struct Link<Message: NetworkMessage, Data: NodeData> {
    identifier: ObjectId,

    queue1: Rc<LinkQueue<Message, Data>>,
    queue2: Rc<LinkQueue<Message, Data>>,

    callback: Box<dyn LinkCallback<Message, Data>>,

    active_queues: AtomicU32,
}

pub trait LinkCallback<Message: NetworkMessage, Data: NodeData> {
    fn message_sent(&self, _source: &ObjectId, _destination: &ObjectId, _message: &Message) {}
    fn link_became_active(&self, _link: &Link<Message, Data>) {}
    fn link_became_inactive(&self, _link: &Link<Message, Data>) {}
}

#[derive(Default)]
pub struct DummyLinkCallback {}

impl LinkCallback<DummyNetworkMessage, DummyNodeData> for DummyLinkCallback {}

impl<Message: NetworkMessage, Data: NodeData> Link<Message, Data> {
    pub(super) fn new(
        node1: Rc<Node<Message, Data>>,
        node2: Rc<Node<Message, Data>>,
        latency: Latency,
        callback: Box<dyn LinkCallback<Message, Data>>,
    ) -> Rc<Self> {
        let queue1 = Rc::new(LinkQueue::new(latency, node1.clone(), node2.clone()));

        let queue2 = Rc::new(LinkQueue::new(latency, node2, node1));

        let active_queues = AtomicU32::new(0);

        let obj = Rc::new(Self {
            identifier: ObjectId::random(),
            queue1,
            queue2,
            active_queues,
            callback,
        });

        let (node1, node2) = obj.get_nodes();
        node1.add_link(node2.get_identifier(), obj.clone());
        node2.add_link(node1.get_identifier(), obj.clone());

        obj
    }

    /// Does the link currently have any messages in transit?
    pub fn is_active(&self) -> bool {
        self.active_queues.load(AtomicOrdering::Relaxed) > 0
    }

    /// Get the two nodes connected with this link
    /// Always sorted by smallest id first
    #[allow(clippy::type_complexity)]
    pub fn get_nodes(&self) -> (&Rc<Node<Message, Data>>, &Rc<Node<Message, Data>>) {
        let node1 = self.queue1.get_source();
        let node2 = self.queue1.get_destination();

        match node1.get_identifier().cmp(&node2.get_identifier()) {
            Ordering::Less => (node1, node2),
            Ordering::Greater => (node2, node1),
            Ordering::Equal => panic!("Invalid state"),
        }
    }

    pub fn send(self_ptr: &Rc<Self>, source: ObjectId, message: Message) {
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

impl<Message: NetworkMessage, Data: NodeData> Object for Link<Message, Data> {
    fn get_identifier(&self) -> ObjectId {
        self.identifier
    }
}

struct LinkQueue<Message: NetworkMessage, Data: NodeData> {
    latency: Duration,

    source: Rc<Node<Message, Data>>,
    dest: Rc<Node<Message, Data>>,

    current_message_count: AtomicU32,
    total_message_count: AtomicU64,
}

impl<Message: NetworkMessage, Data: NodeData> LinkQueue<Message, Data> {
    fn new(
        latency: Latency,
        source: Rc<Node<Message, Data>>,
        dest: Rc<Node<Message, Data>>,
    ) -> Self {
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
        self_ptr: Rc<LinkQueue<Message, Data>>,
        link: Rc<Link<Message, Data>>,
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

    fn get_source(&self) -> &Rc<Node<Message, Data>> {
        &self.source
    }

    fn get_destination(&self) -> &Rc<Node<Message, Data>> {
        &self.dest
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::network::node::{DummyNodeCallback, DummyNodeData, Node};
    use crate::network::{Bandwidth, DummyNetworkMessage, Object};
    use crate::time::Duration;

    use super::{DummyLinkCallback, Link};

    #[test]
    fn is_active() {
        let asim = Rc::new(crate::Runtime::default());
        let (node1, node2, link);

        {
            let _ctx = asim.with_context();
            node1 = Node::new(
                Bandwidth::from_megabits_per_second(1000),
                DummyNodeData::default(),
                Box::new(DummyNodeCallback::default()),
            );
            node2 = Node::new(
                Bandwidth::from_megabits_per_second(1000),
                DummyNodeData::default(),
                Box::new(DummyNodeCallback::default()),
            );

            link = Rc::new(Link::new(
                node1.clone(),
                node2.clone(),
                Duration::from_millis(50),
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
}
