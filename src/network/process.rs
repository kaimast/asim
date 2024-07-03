use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::sync::mpsc;

use crate::network::{get_size_delay, Bandwidth, Latency, Link, NetworkMessage};

/// The unique identifier of a project
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(u32);

impl ProcessId {
    fn random() -> Self {
        Self(rand::random())
    }
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "#{:x}", self.0)
    }
}

pub type NotifyDeliveryFn = Box<dyn FnOnce()>;

/// Implement this trait to add custom logic to a process
#[ async_trait::async_trait(?Send) ]
pub trait ProcessLogic<Message: NetworkMessage>: Any {
    fn start(&self, process: &Process<Message>);
    fn stop(&self, process: &Process<Message>);

    async fn handle_message(&self, process: &Process<Message>, source: ProcessId, message: Message);

    fn handle_disconnect(&self, _process: &Process<Message>, _peer: ProcessId) {}
}

/// A Process represents a node in the network
/// It can communicate with other processed using a Link
pub struct Process<Message: NetworkMessage> {
    identifier: ProcessId,
    inbox_sender: mpsc::Sender<(ProcessId, Message, NotifyDeliveryFn)>,
    bandwidth: Bandwidth,
    logic: Box<dyn ProcessLogic<Message>>,
    network_links: RefCell<HashMap<ProcessId, Rc<Link<Message>>>>,
}

impl<Message: NetworkMessage> Process<Message> {
    /// Create a new process
    ///
    /// * bandwidth: The network bandwidth of this node
    /// * logic: The custom logic for your simulation
    pub fn new(bandwidth: Bandwidth, logic: Box<dyn ProcessLogic<Message>>) -> Rc<Self> {
        let (inbox_sender, inbox_receiver) = mpsc::channel();

        let obj = Rc::new(Self {
            identifier: ProcessId::random(),
            bandwidth,
            inbox_sender,
            logic,
            network_links: RefCell::new(HashMap::default()),
        });

        obj.logic.start(&obj);

        {
            let obj = obj.clone();
            crate::spawn(async move {
                Self::inbox_loop(obj, inbox_receiver).await;
            });
        }

        obj
    }

    pub fn get_identifier(&self) -> ProcessId {
        self.identifier
    }

    /// Shut down this process
    pub fn stop(&self) {
        self.logic.stop(self);
    }

    /// Close all connections to/from this process
    pub fn disconnect_all(&self) {
        let mut links = self.network_links.borrow_mut();

        for (peer_id, link) in links.iter() {
            log::trace!("Disconnecting process {} and {}", self.identifier, peer_id);

            let (proc1, proc2) = link.get_processes();

            let proc = if proc1.get_identifier() == *peer_id {
                proc1
            } else if proc2.get_identifier() == *peer_id {
                proc2
            } else {
                panic!("Invalid state");
            };

            proc.network_links
                .borrow_mut()
                .remove(&self.identifier)
                .expect("Connection did not exist");
            proc.logic.handle_disconnect(&proc, self.identifier);
            self.logic.handle_disconnect(self, *peer_id);
        }

        links.clear();
    }

    /// Connect this process to another one
    pub fn connect(process1: &Rc<Self>, process2: &Rc<Self>, link_latency: Latency) {
        log::trace!(
            "Connecting process {} and {}",
            process1.get_identifier(),
            process2.get_identifier()
        );

        let link = Rc::new(Link::new(
            link_latency,
            Rc::downgrade(process1),
            Rc::downgrade(process2),
        ));

        process1
            .network_links
            .borrow_mut()
            .insert(process2.get_identifier(), link.clone());
        process2
            .network_links
            .borrow_mut()
            .insert(process1.get_identifier(), link);
    }

    pub(super) fn deliver_message(
        &self,
        source: ProcessId,
        message: Message,
        notify_delivery_fn: NotifyDeliveryFn,
    ) {
        self.inbox_sender
            .send((source, message, notify_delivery_fn));
    }

    async fn inbox_loop(
        self_ptr: Rc<Self>,
        inbox_receiver: mpsc::Receiver<(ProcessId, Message, NotifyDeliveryFn)>,
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
                        .logic
                        .handle_message(&*self_ptr2, source, message)
                        .await;
                });
            }
        }
    }

    /// Returns the connection to another process with the specified identifier (if it exists)
    pub fn get_link_to(&self, process_id: &ProcessId) -> Option<Rc<Link<Message>>> {
        match self.network_links.borrow().get(process_id) {
            Some(link) => Some(link.clone()),
            None => {
                log::warn!(
                    "There exists no network link from process {} to {process_id}",
                    self.identifier
                );
                None
            }
        }
    }

    /// Send a message to the process with the specified identifier
    ///
    /// Returns false if no connection to the process existed
    pub fn send_to(&self, process_id: &ProcessId, message: Message) -> bool {
        if let Some(link) = self.get_link_to(process_id) {
            Link::send(&link, self.identifier, message);
            true
        } else {
            false
        }
    }

    /// Get the custom logic associated with this process and convert it
    /// to the specified type.
    ///
    /// Allows to easily attach different logics to a process.
    pub fn get_logic_as<T: ProcessLogic<Message>>(&self) -> &'_ T {
        let logic_ref = &self.logic as &dyn Any;

        logic_ref
            .downcast_ref::<T>()
            .expect("Logic was not of the expected type")
    }
}
