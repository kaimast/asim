//! Chat Network Example
//!
//! This example demonstrates asim's network simulation capabilities by creating
//! a simple chat/messaging system with multiple nodes connected in a network.
//!
//! Key concepts demonstrated:
//! - **Network Nodes**: Creating nodes with custom message handling logic
//! - **Network Links**: Connecting nodes with realistic latency and bandwidth
//! - **Message Broadcasting**: Sending messages to multiple peers
//! - **Network Topology**: Building a realistic network structure
//! - **Bandwidth/Latency Simulation**: Modeling real network constraints
//!
//! The simulation:
//! - Creates 4 chat nodes representing different users
//! - Connects them in a hub topology (one central node, others connect to it)
//! - Simulates a chat conversation with messages being sent and relayed
//! - Shows how network delays affect message delivery timing
//! - Demonstrates both direct messages and broadcast messages

use asim::network::{NetworkMessage, Object};
use asim::{network, sync, time, Runtime};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
struct ChatMessage {
    sender_name: String,
    content: String,
    timestamp: time::Time,
}

impl network::NetworkMessage for ChatMessage {
    fn get_size(&self) -> u64 {
        // Simulate realistic message size: name + content + overhead
        (self.sender_name.len() + self.content.len() + 50) as u64
    }
}

struct ChatNodeData {
    name: String,
    message_log: RefCell<Vec<ChatMessage>>,
    notification_sender: RefCell<Option<sync::mpsc::Sender<ChatMessage>>>,
}

impl network::NodeData for ChatNodeData {}

struct ChatNodeCallback;

#[async_trait::async_trait(?Send)]
impl network::NodeCallback<ChatMessage, ChatNodeData> for ChatNodeCallback {
    async fn handle_message(
        &self,
        node: &Rc<network::Node<ChatMessage, ChatNodeData>>,
        source: network::ObjectId,
        message: ChatMessage,
    ) {
        let node_name = &node.get_data().name;

        // Log the received message
        node.get_data()
            .message_log
            .borrow_mut()
            .push(message.clone());

        println!(
            "[{}] {} received message from {}: \"{}\" (sent at {:?}, received at {:?})",
            time::now(),
            node_name,
            message.sender_name,
            message.content,
            message.timestamp,
            time::now()
        );

        // If this node is the hub, relay the message to other connected nodes
        if node_name == "Hub" && message.sender_name != "Hub" {
            println!(
                "[{}] {} relaying message to other nodes",
                time::now(),
                node_name
            );

            // Broadcast to all peers except the sender
            for peer_id in node.get_peers() {
                if peer_id != source {
                    node.send_to(&peer_id, message.clone());
                }
            }
        }

        // Notify any waiting listeners
        if let Some(sender) = node.get_data().notification_sender.borrow_mut().as_ref() {
            sender.send(message);
        }
    }

    fn node_started(&self, node: &network::Node<ChatMessage, ChatNodeData>) {
        println!(
            "[{}] Chat node '{}' joined the network",
            time::now(),
            node.get_data().name
        );
    }

    fn peer_disconnected(
        &self,
        node: &network::Node<ChatMessage, ChatNodeData>,
        peer: network::ObjectId,
    ) {
        println!(
            "[{}] {} lost connection to peer {}",
            time::now(),
            node.get_data().name,
            peer
        );
    }
}

#[derive(Default)]
struct ChatLinkCallback;

impl network::LinkCallback<ChatMessage, ChatNodeData> for ChatLinkCallback {
    fn message_sent(
        &self,
        source: &network::ObjectId,
        destination: &network::ObjectId,
        message: &ChatMessage,
    ) {
        println!(
            "[{}] Message \"{}\" sent from {} to {} (size: {} bytes)",
            time::now(),
            message.content,
            source,
            destination,
            message.get_size()
        );
    }
}

fn create_chat_node(
    name: &str,
    bandwidth: network::Bandwidth,
) -> Rc<network::Node<ChatMessage, ChatNodeData>> {
    let data = ChatNodeData {
        name: name.to_string(),
        message_log: RefCell::new(Vec::new()),
        notification_sender: RefCell::new(None),
    };

    network::Node::new(bandwidth, data, Box::new(ChatNodeCallback))
}

fn main() {
    env_logger::init();

    let rt = Runtime::new();

    rt.block_on(async {
        println!("=== Chat Network Simulation Starting ===\n");

        // Create network parameters
        let high_bandwidth = network::Bandwidth::from_megabits_per_second(100); // Fast connection
        let medium_bandwidth = network::Bandwidth::from_megabits_per_second(50); // Medium connection
        let lan_latency = network::Latency::from_millis(5);    // Local network
        let wan_latency = network::Latency::from_millis(50);   // Internet connection

        // Create chat nodes
        let hub = create_chat_node("Hub", high_bandwidth);
        let alice = create_chat_node("Alice", medium_bandwidth);
        let bob = create_chat_node("Bob", medium_bandwidth);
        let charlie = create_chat_node("Charlie", medium_bandwidth);

        // Create hub topology - all nodes connect through the hub
        println!("Setting up network topology (hub-and-spoke):");
        network::Node::connect(hub.clone(), alice.clone(), lan_latency, Box::new(ChatLinkCallback));
        network::Node::connect(hub.clone(), bob.clone(), wan_latency, Box::new(ChatLinkCallback));
        network::Node::connect(hub.clone(), charlie.clone(), lan_latency, Box::new(ChatLinkCallback));

        // Give nodes time to initialize
        time::sleep(time::Duration::from_millis(100)).await;

        println!("\n=== Starting Chat Conversation ===\n");

        // Alice sends a message (will be relayed by hub to others)
        let alice_message = ChatMessage {
            sender_name: "Alice".to_string(),
            content: "Hello everyone! How's the simulation going?".to_string(),
            timestamp: time::now(),
        };

        // Alice broadcasts to hub (which will relay)
        let hub_id = hub.get_identifier();
        alice.send_to(&hub_id, alice_message);

        // Wait for message propagation
        time::sleep(time::Duration::from_millis(200)).await;

        // Bob responds
        let bob_message = ChatMessage {
            sender_name: "Bob".to_string(),
            content: "Great! The network delays feel realistic.".to_string(),
            timestamp: time::now(),
        };

        bob.send_to(&hub_id, bob_message);

        // Wait a bit
        time::sleep(time::Duration::from_millis(150)).await;

        // Charlie sends a longer message (will take more time due to size)
        let charlie_message = ChatMessage {
            sender_name: "Charlie".to_string(),
            content: "This is a much longer message to demonstrate how bandwidth affects larger messages. The network simulation is working perfectly and shows realistic timing behavior!".to_string(),
            timestamp: time::now(),
        };

        charlie.send_to(&hub_id, charlie_message);

        // Wait for all messages to propagate
        time::sleep(time::Duration::from_millis(300)).await;

        // Hub sends a final message
        let hub_message = ChatMessage {
            sender_name: "Hub".to_string(),
            content: "Simulation complete! All messages delivered.".to_string(),
            timestamp: time::now(),
        };

        hub.broadcast(hub_message, None);

        // Wait for final propagation
        time::sleep(time::Duration::from_millis(100)).await;

        println!("\n=== Simulation Results ===");
        println!("Final simulation time: {:?}", time::now());

        // Show message logs for each node
        for node in [&alice, &bob, &charlie] {
            let data = node.get_data();
            let log = data.message_log.borrow();
            println!("\n{} received {} messages:", data.name, log.len());
            for msg in log.iter() {
                println!("  - From {}: \"{}\"", msg.sender_name, msg.content);
            }
        }

        println!("\n✓ Chat network simulation completed successfully!");
    });
}
