use crate::time::Duration;

use std::cmp::Ordering as CmpOrdering;
use std::rc::{Rc, Weak as WeakRc};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

use crate::network::{Latency, NetworkMessage, Process, ProcessId};

/// Represents the connection between two processes
///
/// Each link consists of two messages queues, one for each direction
pub struct Link<Message: NetworkMessage> {
    queue1: Rc<LinkQueue<Message>>,
    queue2: Rc<LinkQueue<Message>>,

    active_queues: AtomicU32,
}

impl<Message: NetworkMessage> Link<Message> {
    pub(super) fn new(
        latency: Latency,
        process1: WeakRc<Process<Message>>,
        process2: WeakRc<Process<Message>>,
    ) -> Self {
        let queue1 = Rc::new(LinkQueue::new(latency, process1.clone(), process2.clone()));

        let queue2 = Rc::new(LinkQueue::new(latency, process2, process1));

        let active_queues = AtomicU32::new(0);

        Self {
            queue1,
            queue2,
            active_queues,
        }
    }

    /// Get the two processs connected with this link
    /// Always sorted by smallest id first
    pub fn get_processes(&self) -> (Rc<Process<Message>>, Rc<Process<Message>>) {
        let process1 = self.queue1.get_source();
        let process2 = self.queue1.get_destination();

        match process1.get_identifier().cmp(&process2.get_identifier()) {
            CmpOrdering::Less => (process1, process2),
            CmpOrdering::Greater => (process2, process1),
            CmpOrdering::Equal => panic!("Invalid state: src and dst process are the same"),
        }
    }

    pub fn send(self_ptr: &Rc<Link<Message>>, source: ProcessId, message: Message) {
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
        self.queue1.total_message_count.load(Ordering::SeqCst)
            + self.queue2.total_message_count.load(Ordering::SeqCst)
    }
}

struct LinkQueue<Message: NetworkMessage> {
    latency: Duration,

    source: WeakRc<Process<Message>>,
    dest: WeakRc<Process<Message>>,

    current_message_count: AtomicU32,
    total_message_count: AtomicU64,
}

impl<Message: NetworkMessage> LinkQueue<Message> {
    fn new(
        latency: Latency,
        source: WeakRc<Process<Message>>,
        dest: WeakRc<Process<Message>>,
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
        self_ptr: Rc<LinkQueue<Message>>,
        link: Rc<Link<Message>>,
        message: Message,
    ) -> (bool, Duration) {
        let latency = self_ptr.latency;

        let was_empty = {
            self_ptr.total_message_count.fetch_add(1, Ordering::SeqCst);
            let prev = self_ptr
                .current_message_count
                .fetch_add(1, Ordering::SeqCst);
            prev == 0
        };

        if was_empty {
            link.active_queues.fetch_add(1, Ordering::SeqCst);
        }

        crate::spawn(async move {
            //TODO re-add link bandwidth

            let notify_delivery_fn = {
                let self_ptr = self_ptr.clone();
                let link = link.clone();

                Box::new(move || {
                    let prev = self_ptr
                        .current_message_count
                        .fetch_sub(1, Ordering::SeqCst);
                    assert!(prev > 0);

                    if prev == 1 {
                        link.active_queues.fetch_sub(1, Ordering::SeqCst);
                    }
                })
            };

            let dst = self_ptr.get_destination();
            dst.deliver_message(
                self_ptr.get_source().get_identifier(),
                message,
                notify_delivery_fn,
            );
        });

        (was_empty, latency)
    }

    fn get_source(&self) -> Rc<Process<Message>> {
        self.source.upgrade().unwrap()
    }

    fn get_destination(&self) -> Rc<Process<Message>> {
        self.dest.upgrade().unwrap()
    }
}

/* TOOD re-add event handling
#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::mpsc;

    use crate::events::{Event, LinkEvent, EVENT_HANDLER};
    use crate::logic::DummyLogic;
    use crate::message::DummyMessage;
    use crate::process::Node;
    use crate::Location;

    use task_runner::time::Duration;
    use task_runner::Timer;

    use super::Link;

    fn get_events(event_receiver: &mpsc::Receiver<Event>) -> Vec<Event> {
        let mut result = vec![];

        while let Ok(event) = event_receiver.try_recv() {
            result.push(event);
        }

        result
    }

    #[test]
    fn is_active() {
        let task_runner = Rc::new(TaskRunner::default());

        let logic = Rc::new(DummyLogic::default());

        let (event_sender, event_receiver) = mpsc::channel();

        EVENT_HANDLER.with(|hdl| {
            let mut handler = hdl.borrow_mut();
            if handler.is_none() {
                *handler = Some(event_sender);
            }
        });

        let process1 = Node::new(
            2,
            0,
            Location::default(),
            1000,
            &task_runner,
            logic.clone(),
        );

        let process2 = Node::new(
            2,
            1,
            Location::default(),
            1000,
            &task_runner,
            logic,
        );

        let link = Rc::new(Link::new(
            50,
            process1.clone(),
            process2.clone(),
            task_runner.clone(),
        ));

        let events = get_events(&event_receiver);
        assert!(events.is_empty());
        Link::send(&link, 1, DummyMessage::default().into());

        // Sending messages is a two step process (link latency + bandwidth)
        task_runner.execute_tasks();

        let events = get_events(&event_receiver);
        assert_eq!(events.len(), 1);
        assert_eq!(
            Event::Link {
                identifier: 3,
                event: LinkEvent::Active
            },
            events[0]
        );

        rutnime.get_timer().advance();
        task_runner.execute_tasks();
        task_runner.execute_tasks();

        let events = get_events(&event_receiver);
        assert_eq!(events.len(), 1);
        assert_eq!(
            Event::Link {
                identifier: 3,
                event: LinkEvent::Inactive
            },
            events[0]
        );
    }
}*/
