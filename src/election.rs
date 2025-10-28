use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use sysinfo::System;

// ============================================================================
// METRICS FOR PRIORITY CALCULATION
// ============================================================================

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ServerMetrics {
    active_tasks: Arc<AtomicU64>, // How many tasks currently running
    total_tasks: Arc<AtomicU64>,  // Total tasks processed (for stats)
    system: Arc<std::sync::Mutex<System>>, // System info for metrics
}

#[allow(dead_code)]
impl ServerMetrics {
    pub fn new() -> Self {
        Self {
            active_tasks: Arc::new(AtomicU64::new(0)),
            total_tasks: Arc::new(AtomicU64::new(0)),
            system: Arc::new(std::sync::Mutex::new(System::new_all())),
        }
    }

    /// Get current CPU usage (0-100)
    pub fn get_cpu_usage(&self) -> f64 {
        let mut sys = self.system.lock().unwrap();

        // Refresh CPU information
        sys.refresh_cpu_all();

        // Get global CPU usage (average across all cores)
        sys.global_cpu_usage() as f64
    }

    /// Get number of active tasks
    pub fn get_active_tasks(&self) -> u64 {
        self.active_tasks.load(Ordering::Relaxed)
    }

    /// Get available memory percentage (0-100)
    pub fn get_available_memory_percent(&self) -> f64 {
        let mut sys = self.system.lock().unwrap();

        // Refresh memory information
        sys.refresh_memory();

        let total = sys.total_memory();
        let available = sys.available_memory();

        if total == 0 {
            return 100.0;
        }

        (available as f64 / total as f64) * 100.0
    }

    /// Increment task count when starting a task
    pub fn task_started(&self) {
        self.active_tasks.fetch_add(1, Ordering::Relaxed);
        self.total_tasks.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement task count when finishing a task
    pub fn task_finished(&self) {
        self.active_tasks.fetch_sub(1, Ordering::Relaxed);
    }

    /// Calculate priority for Modified Bully Algorithm
    /// LOWER score = BETTER candidate (less loaded)
    pub fn calculate_priority(&self) -> f64 {
        const W_CPU: f64 = 0.5; // Weight for CPU usage
        const W_TASKS: f64 = 0.3; // Weight for active tasks
        const W_MEMORY: f64 = 0.2; // Weight for memory

        let cpu_usage = self.get_cpu_usage();
        let active_tasks = self.get_active_tasks() as f64;
        let memory_available = self.get_available_memory_percent();

        // Normalize active tasks (assuming max 10 concurrent tasks is "full load")
        let tasks_normalized = (active_tasks / 10.0).min(1.0) * 100.0;

        // Memory score: lower available = higher score (worse)
        let memory_score = 100.0 - memory_available;

        // Calculate composite score (lower = better)
        let priority = W_CPU * cpu_usage + W_TASKS * tasks_normalized + W_MEMORY * memory_score;

        priority
    }

    /// Get a load value between 0.0 and 1.0 for heartbeats
    pub fn get_load(&self) -> f64 {
        // Simple version: just normalize the priority to 0-1
        self.calculate_priority() / 100.0
    }
}
