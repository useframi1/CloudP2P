use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// ============================================================================
// METRICS FOR PRIORITY CALCULATION
// ============================================================================

#[derive(Debug, Clone)]
pub struct ServerMetrics {
    current_load: Arc<AtomicU64>,      // Stored as u64 (multiply by 1000)
    reliability_score: Arc<AtomicU64>, // Stored as u64 (multiply by 1000)
    avg_response_time: Arc<AtomicU64>, // In milliseconds
}

impl ServerMetrics {
    pub fn new(initial_load: f64, initial_reliability: f64, initial_response_time: f64) -> Self {
        Self {
            current_load: Arc::new(AtomicU64::new((initial_load * 1000.0) as u64)),
            reliability_score: Arc::new(AtomicU64::new((initial_reliability * 1000.0) as u64)),
            avg_response_time: Arc::new(AtomicU64::new(initial_response_time as u64)),
        }
    }

    pub fn get_load(&self) -> f64 {
        self.current_load.load(Ordering::Relaxed) as f64 / 1000.0
    }

    pub fn get_reliability(&self) -> f64 {
        self.reliability_score.load(Ordering::Relaxed) as f64 / 1000.0
    }

    pub fn get_response_time(&self) -> f64 {
        self.avg_response_time.load(Ordering::Relaxed) as f64
    }

    /// Calculate priority for Modified Bully Algorithm
    /// Higher priority = better candidate for leadership
    pub fn calculate_priority(&self) -> f64 {
        const W1: f64 = 0.4; // Load weight
        const W2: f64 = 0.3; // Reliability weight
        const W3: f64 = 0.3; // Response time weight

        let load = self.get_load();
        let reliability = self.get_reliability();
        let response_time = self.get_response_time();

        let load_score = 1.0 - load; // Lower load is better
        let response_score = 1.0 / (1.0 + response_time / 100.0); // Lower response time is better

        W1 * load_score + W2 * reliability + W3 * response_score
    }
}
