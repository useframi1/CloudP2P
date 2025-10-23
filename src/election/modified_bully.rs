use crate::server::metrics::ServerMetrics;

pub struct ModifiedBullyElection {
    _server_id: u32,
    metrics: ServerMetrics,
}

impl ModifiedBullyElection {
    pub fn new(server_id: u32, metrics: ServerMetrics) -> Self {
        Self {
            _server_id: server_id,
            metrics,
        }
    }

    pub fn calculate_priority(&self) -> f64 {
        self.metrics.calculate_priority()
    }
}
