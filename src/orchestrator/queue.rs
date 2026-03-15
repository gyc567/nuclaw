//! Task queue with priority and concurrency support

use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};

use super::task::Task;

/// A task entry for the priority queue
#[derive(Clone)]
struct TaskEntry {
    task: Task,
    insertion_order: u64,
}

impl PartialEq for TaskEntry {
    fn eq(&self, other: &Self) -> bool {
        self.task.priority == other.task.priority && self.insertion_order == other.insertion_order
    }
}

impl Eq for TaskEntry {}

impl PartialOrd for TaskEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first (BinaryHeap returns largest), then FIFO
        let priority_cmp = self.task.priority.cmp(&other.task.priority);
        if priority_cmp != Ordering::Equal {
            return priority_cmp;
        }
        // FIFO: lower insertion_order comes first (BinaryHeap is max-heap)
        other.insertion_order.cmp(&self.insertion_order)
    }
}

/// Task queue with priority support
#[derive(Clone)]
pub struct TaskQueue {
    queue: Arc<Mutex<BinaryHeap<TaskEntry>>>,
    running_count: Arc<Mutex<usize>>,
    max_concurrency: usize,
    next_insertion_order: Arc<Mutex<u64>>,
}

impl TaskQueue {
    /// Create a new task queue
    pub fn new(max_concurrency: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            running_count: Arc::new(Mutex::new(0)),
            max_concurrency,
            next_insertion_order: Arc::new(Mutex::new(0)),
        }
    }

    /// Enqueue a task
    pub fn enqueue(&self, task: Task) {
        let order = {
            let mut counter = self.next_insertion_order.lock().unwrap();
            let order = *counter;
            *counter += 1;
            order
        };
        
        let entry = TaskEntry {
            task,
            insertion_order: order,
        };
        
        self.queue.lock().unwrap().push(entry);
    }

    /// Dequeue the next task if concurrency allows
    pub fn dequeue(&self) -> Option<Task> {
        let running = *self.running_count.lock().unwrap();
        
        if running >= self.max_concurrency {
            return None;
        }
        
        let entry = self.queue.lock().unwrap().pop()?;
        
        *self.running_count.lock().unwrap() += 1;
        
        Some(entry.task)
    }

    /// Mark a task as completed (frees up a concurrency slot)
    pub fn complete(&self) {
        let mut count = self.running_count.lock().unwrap();
        if *count > 0 {
            *count -= 1;
        }
    }

    /// Get the number of pending tasks
    pub fn pending_count(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Get the number of running tasks
    pub fn running_count(&self) -> usize {
        *self.running_count.lock().unwrap()
    }

    /// Get the total number of tasks (pending + running)
    pub fn total_count(&self) -> usize {
        let pending = self.queue.lock().unwrap().len();
        let running = *self.running_count.lock().unwrap();
        pending + running
    }

    /// Check if at capacity (running >= max_concurrency)
    pub fn is_at_capacity(&self) -> bool {
        *self.running_count.lock().unwrap() >= self.max_concurrency
    }

    /// Peek at the next task without removing it
    pub fn peek(&self) -> Option<Task> {
        self.queue.lock().unwrap().peek().map(|e| e.task.clone())
    }

    /// Get all pending tasks (for debugging/inspection)
    pub fn pending_tasks(&self) -> Vec<Task> {
        self.queue.lock().unwrap()
            .iter()
            .map(|e| e.task.clone())
            .collect()
    }

    /// Re-queue a task (e.g., for retry)
    pub fn requeue(&self, task: Task) {
        self.enqueue(task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::task::{TaskSource, Priority};

    fn create_test_task(payload: &str) -> Task {
        Task::new(payload.to_string())
    }

    fn create_test_task_with_priority(payload: &str, priority: Priority) -> Task {
        Task::new(payload.to_string()).with_priority(priority)
    }

    #[test]
    fn test_queue_new() {
        let queue = TaskQueue::new(2);
        assert_eq!(queue.pending_count(), 0);
        assert_eq!(queue.running_count(), 0);
    }

    #[test]
    fn test_queue_enqueue_dequeue() {
        let queue = TaskQueue::new(2);
        
        queue.enqueue(create_test_task("task1"));
        assert_eq!(queue.pending_count(), 1);
        
        let task = queue.dequeue();
        assert!(task.is_some());
        assert_eq!(task.unwrap().payload, "task1");
        assert_eq!(queue.pending_count(), 0);
        assert_eq!(queue.running_count(), 1);
    }

    #[test]
    fn test_queue_empty_dequeue() {
        let queue = TaskQueue::new(2);
        
        let task = queue.dequeue();
        assert!(task.is_none());
    }

    #[test]
    fn test_queue_complete() {
        let queue = TaskQueue::new(2);
        
        queue.enqueue(create_test_task("task1"));
        let _task = queue.dequeue();
        assert_eq!(queue.running_count(), 1);
        
        queue.complete();
        assert_eq!(queue.running_count(), 0);
    }

    #[test]
    fn test_queue_priority_ordering() {
        let queue = TaskQueue::new(10);
        
        queue.enqueue(create_test_task_with_priority("low", Priority::Low));
        queue.enqueue(create_test_task_with_priority("high", Priority::High));
        queue.enqueue(create_test_task_with_priority("normal1", Priority::Normal));
        queue.enqueue(create_test_task_with_priority("normal2", Priority::Normal));
        queue.enqueue(create_test_task_with_priority("critical", Priority::Critical));
        
        let task = queue.dequeue().unwrap();
        assert_eq!(task.payload, "critical");
        
        let task = queue.dequeue().unwrap();
        assert_eq!(task.payload, "high");
        
        let task = queue.dequeue().unwrap();
        assert!(task.payload == "normal1" || task.payload == "normal2");
        
        let task = queue.dequeue().unwrap();
        assert!(task.payload == "normal1" || task.payload == "normal2");
        
        let task = queue.dequeue().unwrap();
        assert_eq!(task.payload, "low");
    }

    #[test]
    fn test_queue_fifo_same_priority() {
        let queue = TaskQueue::new(10);
        
        queue.enqueue(create_test_task("task1"));
        queue.enqueue(create_test_task("task2"));
        queue.enqueue(create_test_task("task3"));
        
        assert_eq!(queue.dequeue().unwrap().payload, "task1");
        assert_eq!(queue.dequeue().unwrap().payload, "task2");
        assert_eq!(queue.dequeue().unwrap().payload, "task3");
    }

    #[test]
    fn test_queue_concurrency_limit() {
        let queue = TaskQueue::new(2);
        
        queue.enqueue(create_test_task("task1"));
        queue.enqueue(create_test_task("task2"));
        queue.enqueue(create_test_task("task3"));
        
        let _t1 = queue.dequeue();
        let _t2 = queue.dequeue();
        assert_eq!(queue.running_count(), 2);
        
        // Third task should not be available yet
        let t3 = queue.dequeue();
        assert!(t3.is_none());
        
        assert!(queue.is_at_capacity());
        
        queue.complete();
        assert!(!queue.is_at_capacity());
        
        let t3 = queue.dequeue();
        assert!(t3.is_some());
        assert_eq!(t3.unwrap().payload, "task3");
    }

    #[test]
    fn test_queue_total_count() {
        let queue = TaskQueue::new(2);
        
        queue.enqueue(create_test_task("task1"));
        queue.enqueue(create_test_task("task2"));
        
        let _ = queue.dequeue();
        
        assert_eq!(queue.total_count(), 2); // 1 pending + 1 running
    }

    #[test]
    fn test_queue_peek() {
        let queue = TaskQueue::new(2);
        
        queue.enqueue(create_test_task("task1"));
        queue.enqueue(create_test_task("task2"));
        
        let peeked = queue.peek();
        assert!(peeked.is_some());
        assert_eq!(peeked.unwrap().payload, "task1");
        
        // Peek doesn't remove
        assert_eq!(queue.pending_count(), 2);
    }

    #[test]
    fn test_queue_pending_tasks() {
        let queue = TaskQueue::new(2);
        
        queue.enqueue(create_test_task("task1"));
        queue.enqueue(create_test_task("task2"));
        
        let tasks = queue.pending_tasks();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_queue_requeue() {
        let queue = TaskQueue::new(2);
        
        queue.enqueue(create_test_task("task1"));
        let mut task2 = create_test_task("task2");
        task2.start();
        
        queue.requeue(task2);
        
        assert_eq!(queue.pending_count(), 2);
    }
}
