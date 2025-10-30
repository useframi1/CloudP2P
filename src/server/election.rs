//! # Server Metrics and Priority Calculation
//!
//! This module provides server performance metrics and priority calculation
//! for the Modified Bully Algorithm leader election.
//!
//! ## Priority Formula
//!
//! The priority score is calculated as a weighted combination of:
//! - **CPU Usage** (50% weight): 0-100% from system metrics
//! - **Active Tasks** (30% weight): Normalized task count (10 tasks = 100%)
//! - **Memory Usage** (20% weight): 100% - available memory percentage
//!
//! **Lower scores indicate better candidates** (less loaded servers).
//!
//! Example: A server with 20% CPU, 2 active tasks, and 80% available memory:
//! ```text
//! priority = 0.5 * 20 + 0.3 * 20 + 0.2 * 20 = 20.0
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use sysinfo::System;

/// Server performance metrics used for leader election priority calculation.
///
/// Tracks real-time CPU usage, memory availability, and active task count
/// to determine which server is least loaded and should become the leader.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ServerMetrics {
    /// Number of tasks currently being processed
    active_tasks: Arc<AtomicU64>,
    /// Total number of tasks processed over server lifetime (for statistics)
    total_tasks: Arc<AtomicU64>,
    /// System information provider for CPU and memory metrics
    system: Arc<std::sync::Mutex<System>>,
}

#[allow(dead_code)]
impl ServerMetrics {
    /// Create a new ServerMetrics instance with all counters at zero.
    ///
    /// # Example
    /// ```ignore
    /// let metrics = ServerMetrics::new();
    /// ```
    pub fn new() -> Self {
        Self {
            active_tasks: Arc::new(AtomicU64::new(0)),
            total_tasks: Arc::new(AtomicU64::new(0)),
            system: Arc::new(std::sync::Mutex::new(System::new_all())),
        }
    }

    /// Get current CPU usage as a percentage (0.0 to 100.0).
    ///
    /// Returns the average CPU usage across all cores.
    ///
    /// # Returns
    /// - CPU usage percentage (0.0 = idle, 100.0 = fully utilized)
    ///
    /// # Example
    /// ```ignore
    /// let cpu = metrics.get_cpu_usage();
    /// println!("CPU usage: {:.1}%", cpu);
    /// ```
    pub fn get_cpu_usage(&self) -> f64 {
        let mut sys = self.system.lock().unwrap();

        // Refresh CPU information to get current readings
        sys.refresh_cpu_all();

        // Get global CPU usage (average across all cores)
        sys.global_cpu_usage() as f64
    }

    /// Get the number of currently active (running) tasks.
    ///
    /// # Returns
    /// - Count of tasks currently being processed
    ///
    /// # Example
    /// ```ignore
    /// let active = metrics.get_active_tasks();
    /// println!("Active tasks: {}", active);
    /// ```
    pub fn get_active_tasks(&self) -> u64 {
        self.active_tasks.load(Ordering::Relaxed)
    }

    /// Get available memory as a percentage (0.0 to 100.0).
    ///
    /// # Returns
    /// - Percentage of memory that is available/free (0.0 = none available, 100.0 = all available)
    ///
    /// # Example
    /// ```ignore
    /// let mem = metrics.get_available_memory_percent();
    /// println!("Available memory: {:.1}%", mem);
    /// ```
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

    /// Increment the active task counter when a task starts processing.
    ///
    /// Should be called at the beginning of task processing.
    ///
    /// # Example
    /// ```ignore
    /// metrics.task_started();
    /// // ... process task ...
    /// metrics.task_finished();
    /// ```
    pub fn task_started(&self) {
        self.active_tasks.fetch_add(1, Ordering::Relaxed);
        self.total_tasks.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the active task counter when a task finishes processing.
    ///
    /// Should be called when task processing completes (success or failure).
    ///
    /// # Example
    /// ```ignore
    /// metrics.task_started();
    /// // ... process task ...
    /// metrics.task_finished();
    /// ```
    pub fn task_finished(&self) {
        self.active_tasks.fetch_sub(1, Ordering::Relaxed);
    }

    /// Calculate priority score for Modified Bully Algorithm leader election.
    ///
    /// **IMPORTANT**: LOWER scores indicate BETTER candidates (less loaded servers).
    ///
    /// # Formula
    ///
    /// ```text
    /// priority = 0.5 * CPU_usage + 0.3 * normalized_tasks + 0.2 * memory_used
    /// ```
    ///
    /// Where:
    /// - `CPU_usage`: 0-100% from system metrics
    /// - `normalized_tasks`: (active_tasks / 10) * 100, capped at 100%
    /// - `memory_used`: 100% - available_memory_percent
    ///
    /// # Returns
    /// - Priority score (0.0 = best/unloaded, 100.0 = worst/overloaded)
    ///
    /// # Examples
    ///
    /// Idle server:
    /// ```text
    /// CPU: 0%, Tasks: 0, Memory Available: 100%
    /// priority = 0.5*0 + 0.3*0 + 0.2*0 = 0.0 (best)
    /// ```
    ///
    /// Moderately loaded server:
    /// ```text
    /// CPU: 40%, Tasks: 5, Memory Available: 60%
    /// priority = 0.5*40 + 0.3*50 + 0.2*40 = 43.0
    /// ```
    ///
    /// Heavily loaded server:
    /// ```text
    /// CPU: 80%, Tasks: 10, Memory Available: 20%
    /// priority = 0.5*80 + 0.3*100 + 0.2*80 = 86.0 (poor)
    /// ```
    pub fn calculate_priority(&self) -> f64 {
        const W_CPU: f64 = 0.5;     // Weight for CPU usage (50%)
        const W_TASKS: f64 = 0.3;   // Weight for active tasks (30%)
        const W_MEMORY: f64 = 0.2;  // Weight for memory (20%)

        let cpu_usage = self.get_cpu_usage();
        let active_tasks = self.get_active_tasks() as f64;
        let memory_available = self.get_available_memory_percent();

        // Normalize active tasks (assuming max 10 concurrent tasks = "full load")
        let tasks_normalized = (active_tasks / 10.0).min(1.0) * 100.0;

        // Memory score: lower available memory = higher score (worse)
        let memory_score = 100.0 - memory_available;

        // Calculate composite score (lower = better candidate)
        let priority = W_CPU * cpu_usage + W_TASKS * tasks_normalized + W_MEMORY * memory_score;

        priority
    }

    /// Get the current load value as a percentage (0.0 to 100.0).
    ///
    /// This is an alias for [`calculate_priority()`](Self::calculate_priority)
    /// and represents the overall server load.
    ///
    /// # Returns
    /// - Load percentage (0.0 = no load, 100.0 = maximum load)
    ///
    /// # Example
    /// ```ignore
    /// let load = metrics.get_load();
    /// if load < 30.0 {
    ///     println!("Server is lightly loaded");
    /// }
    /// ```
    pub fn get_load(&self) -> f64 {
        self.calculate_priority()
    }
}
