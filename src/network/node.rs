use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::sync::mpsc;

use crate::network::{DummyNetworkMessage, Latency, NetworkMessage};

use crate::network::link::{get_size_delay, Bandwidth, Link, LinkCallback};
use crate::network::{Object, ObjectId};

pub type NotifyDeliveryFn = Box<dyn FnOnce()>;

/// Implement this trait to add custom logic to a node
#[ async_trait::async_trait(?Send) ]
pub trait NodeCallback<Message: NetworkMessage> {
    fn node_started(&self, _node: &Node<Message>) {}
    fn node_stopped(&self, _node: &Node<Message>) {}

    async fn handle_message(&self, _node: &Node<Message>, _source: ObjectId, message: Message);

    fn peer_disconnected(&self, _node: &Node<Message>, _peer: ObjectId) {}
}

#[derive(Default)]
pub struct DummyNodeCallback {}

#[ async_trait::async_trait(?Send) ]
impl NodeCallback<DummyNetworkMessage> for DummyNodeCallback {
    async fn handle_message(
        &self,
        _node: &Node<DummyNetworkMessage>,
        _source: ObjectId,
        _message: DummyNetworkMessage,
    ) {
    }
}

/// A Node represents a node in the network
/// It can communicate with other nodes using a Link
pub struct Node<Message: NetworkMessage> {
    identifier: ObjectId,
    inbox_sender: mpsc::Sender<(ObjectId, Message, NotifyDeliveryFn)>,
    bandwidth: Bandwidth,
    callback: Box<dyn NodeCallback<Message>>,
    network_links: RefCell<HashMap<ObjectId, Rc<Link<Message>>>>,
}

impl<Message: NetworkMessage> Node<Message> {
    /// Create a new node
    ///
    /// * bandwidth: The network bandwidth of this node
    /// * logic: The custom logic for your simulation
    pub fn new(bandwidth: Bandwidth, callback: Box<dyn NodeCallback<Message>>) -> Rc<Self> {
        let (inbox_sender, inbox_receiver) = mpsc::channel();

        let obj = Rc::new(Self {
            identifier: ObjectId::random(),
            bandwidth,
            inbox_sender,
            callback,
            network_links: RefCell::new(HashMap::default()),
        });

        obj.callback.node_started(&*obj);

        {
            let obj = obj.clone();
            crate::spawn(async move {
                Self::inbox_loop(obj, inbox_receiver).await;
            });
        }

        obj
    }

    /// Shut down this node
    pub fn stop(&self) {
        self.callback.node_stopped(self);
    }

    /// Close all connections to/from this node
    pub fn disconnect_all(&self) {
        let mut links = self.network_links.borrow_mut();

        for (peer_id, link) in links.iter() {
            log::trace!("Disconnecting node {} and {}", self.identifier, peer_id);

            let (node1, node2) = link.get_nodes();

            let node = if node1.get_identifier() == *peer_id {
                node1
            } else if node2.get_identifier() == *peer_id {
                node2
            } else {
                panic!("Invalid state");
            };

            node.network_links
                .borrow_mut()
                .remove(&self.identifier)
                .expect("Connection did not exist");
            node.callback.peer_disconnected(node, self.identifier);
            self.callback.peer_disconnected(self, *peer_id);
        }

        links.clear();
    }

    /// Connect this node to another one
    pub fn connect(
        node1: &Rc<Self>,
        node2: &Rc<Self>,
        link_latency: Latency,
        callback: Box<dyn LinkCallback<Message>>,
    ) {
        log::trace!(
            "Connecting node {} and {}",
            node1.get_identifier(),
            node2.get_identifier()
        );

        let link = Rc::new(Link::new(
            link_latency,
            node1.clone(),
            node2.clone(),
            callback,
        ));

        node1
            .network_links
            .borrow_mut()
            .insert(node2.get_identifier(), link.clone());
        node2
            .network_links
            .borrow_mut()
            .insert(node1.get_identifier(), link);
    }

    pub(super) fn deliver_message(
        &self,
        source: ObjectId,
        message: Message,
        notify_delivery_fn: NotifyDeliveryFn,
    ) {
        self.inbox_sender
            .send((source, message, notify_delivery_fn));
    }

    async fn inbox_loop(
        self_ptr: Rc<Self>,
        inbox_receiver: mpsc::Receiver<(ObjectId, Message, NotifyDeliveryFn)>,
    ) {
        loop {
            for (source, message, notify_delivery_fn) in inbox_receiver.recv().await.drain(..) {
                let size = message.get_size();
                let size_delay = get_size_delay(size, self_ptr.bandwidth);

                if !size_delay.is_zero() {
                    crate::time::sleep(size_delay).await;
                }

                notify_delivery_fn();

                let self_ptr2 = self_ptr.clone();
                crate::spawn(async move {
                    self_ptr2
                        .callback
                        .handle_message(&*self_ptr2, source, message)
                        .await;
                });
            }
        }
    }

    /// Returns the connection to another node with the specified identifier (if it exists)
    pub fn get_link_to(&self, node_id: &ObjectId) -> Option<Rc<Link<Message>>> {
        match self.network_links.borrow().get(node_id) {
            Some(link) => Some(link.clone()),
            None => {
                log::warn!(
                    "There exists no network link from node {} to {node_id}",
                    self.identifier
                );
                None
            }
        }
    }

    /// Send a message to the node with the specified identifier
    ///
    /// Returns false if no connection to the node existed
    pub fn send_to(&self, node_id: &ObjectId, message: Message) -> bool {
        if let Some(link) = self.get_link_to(node_id) {
            Link::send(&link, self.identifier, message);
            true
        } else {
            false
        }
    }

    /// Get the callback associated with this node
    pub fn get_callback(&self) -> &dyn NodeCallback<Message> {
        &*self.callback
    }
}

impl<Message: NetworkMessage> Object for Node<Message> {
    fn get_identifier(&self) -> ObjectId {
        self.identifier
    }
}
