//! Shared Counter Example
//!
//! This example demonstrates synchronization primitives in asim by simulating
//! multiple tasks that concurrently access a shared resource (a counter).
//!
//! Key concepts demonstrated:
//! - **Mutex**: Protecting shared data from concurrent access
//! - **MPSC Channels**: Communication between tasks via message passing
//! - **Task Spawning**: Creating multiple concurrent tasks with `asim::spawn()`
//! - **Time Simulation**: Using `time::sleep()` to simulate realistic delays
//!
//! The simulation:
//! - Spawns 5 tasks, each incrementing a shared counter 3 times (15 total)
//! - Each task has different timing patterns to show realistic concurrency
//! - Tasks report progress through a channel for coordination
//! - The mutex ensures thread-safe access to the shared counter
//! - Verifies correctness by checking final values

use asim::{sync, time, Runtime};
use std::rc::Rc;

fn main() {
    env_logger::init();

    let rt = Runtime::new();

    rt.block_on(async {
        // Create a shared counter protected by a mutex
        let counter = Rc::new(sync::Mutex::new(0i32));

        // Create a channel for tasks to report their work
        let (sender, receiver) = sync::mpsc::channel();
        let sender = Rc::new(sender);

        println!(
            "Starting simulation with shared counter at time {:?}",
            time::now()
        );

        // Spawn multiple tasks that will increment the shared counter
        for task_id in 0..5 {
            let counter_clone = counter.clone();
            let sender_clone = sender.clone();

            asim::spawn(async move {
                // Each task does some work multiple times
                for iteration in 0..3 {
                    // Simulate some work before accessing the shared resource
                    time::sleep(time::Duration::from_millis(100 + task_id * 50)).await;

                    // Access the shared counter (critical section)
                    {
                        let mut guard = counter_clone.lock().await;
                        let old_value = *guard;

                        // Simulate some processing time while holding the lock
                        time::sleep(time::Duration::from_millis(50)).await;

                        *guard += 1;
                        let new_value = *guard;

                        println!(
                            "Task {} (iteration {}) at time {:?}: counter {} -> {}",
                            task_id,
                            iteration,
                            time::now(),
                            old_value,
                            new_value
                        );
                    } // Lock is automatically released here

                    // Notify completion of this iteration
                    sender_clone.send(format!(
                        "Task {} completed iteration {}",
                        task_id, iteration
                    ));
                }

                // Notify task completion
                sender_clone.send(format!("Task {} finished all work", task_id));
            });
        }

        // Drop the original sender reference so the receiver knows when all tasks are done
        // Note: we need to drop the Rc, but since tasks still hold references,
        // we'll use a different approach - count completed tasks instead

        // Collect reports from all tasks
        let mut completed_tasks = 0;
        let mut total_iterations = 0;
        const TOTAL_TASKS: usize = 5;

        while completed_tasks < TOTAL_TASKS {
            let messages = receiver.recv().await;

            for message in messages {
                println!("Report: {}", message);
                if message.contains("finished all work") {
                    completed_tasks += 1;
                } else if message.contains("completed iteration") {
                    total_iterations += 1;
                }
            }
        }

        // Check final results
        let final_counter = *counter.lock().await;
        println!("\nSimulation completed at time {:?}", time::now());
        println!("Final counter value: {}", final_counter);
        println!("Total iterations completed: {}", total_iterations);
        println!("Tasks completed: {}", completed_tasks);

        // Verify correctness: 5 tasks × 3 iterations = 15 increments
        assert_eq!(final_counter, 15);
        assert_eq!(total_iterations, 15);
        assert_eq!(completed_tasks, 5);

        println!("✓ All synchronization worked correctly!");
    });
}
