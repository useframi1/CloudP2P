use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetric {
    pub request_id: u64,
    pub start_time: u64, // milliseconds since epoch
    pub latency_ms: u64,
    pub success: bool,
    pub failure_reason: Option<String>,
    pub assigned_server_id: Option<u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AggregatedStats {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub failure_rate: f64,

    // Latency statistics (milliseconds)
    pub latency_min_ms: u64,
    pub latency_max_ms: u64,
    pub latency_avg_ms: f64,
    pub latency_p50_ms: u64,
    pub latency_p95_ms: u64,
    pub latency_p99_ms: u64,

    // Load balancing - requests per server
    pub server_distribution: HashMap<u32, usize>,

    // Failure reasons breakdown
    pub failure_reasons: HashMap<String, usize>,
}

#[derive(Debug)]
pub struct ClientMetrics {
    client_name: String,
    start_time: Instant,
    requests: Vec<RequestMetric>,
}

impl ClientMetrics {
    pub fn new(client_name: String) -> Self {
        Self {
            client_name,
            start_time: Instant::now(),
            requests: Vec::new(),
        }
    }

    pub fn record_request(
        &mut self,
        request_id: u64,
        latency: Duration,
        success: bool,
        failure_reason: Option<String>,
        assigned_server_id: Option<u32>,
    ) {
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        self.requests.push(RequestMetric {
            request_id,
            start_time,
            latency_ms: latency.as_millis() as u64,
            success,
            failure_reason,
            assigned_server_id,
        });
    }

    pub fn aggregate(&self) -> AggregatedStats {
        let mut stats = AggregatedStats::default();

        if self.requests.is_empty() {
            return stats;
        }

        stats.total_requests = self.requests.len();
        stats.successful_requests = self.requests.iter().filter(|r| r.success).count();
        stats.failed_requests = self.requests.iter().filter(|r| !r.success).count();
        stats.failure_rate = (stats.failed_requests as f64 / stats.total_requests as f64) * 100.0;

        // Calculate latency statistics from successful requests
        let mut successful_latencies: Vec<u64> = self.requests
            .iter()
            .filter(|r| r.success)
            .map(|r| r.latency_ms)
            .collect();

        if !successful_latencies.is_empty() {
            successful_latencies.sort_unstable();

            stats.latency_min_ms = *successful_latencies.first().unwrap();
            stats.latency_max_ms = *successful_latencies.last().unwrap();
            stats.latency_avg_ms = successful_latencies.iter().sum::<u64>() as f64
                / successful_latencies.len() as f64;

            stats.latency_p50_ms = percentile(&successful_latencies, 50.0);
            stats.latency_p95_ms = percentile(&successful_latencies, 95.0);
            stats.latency_p99_ms = percentile(&successful_latencies, 99.0);
        }

        // Calculate server distribution
        for request in &self.requests {
            if let Some(server_id) = request.assigned_server_id {
                *stats.server_distribution.entry(server_id).or_insert(0) += 1;
            }
        }

        // Calculate failure reasons
        for request in self.requests.iter().filter(|r| !r.success) {
            if let Some(reason) = &request.failure_reason {
                *stats.failure_reasons.entry(reason.clone()).or_insert(0) += 1;
            }
        }

        stats
    }

    pub fn export_to_json<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let stats = self.aggregate();

        let output = serde_json::json!({
            "client_name": self.client_name,
            "test_duration_secs": self.start_time.elapsed().as_secs(),
            "aggregated_stats": stats,
        });

        let json_string = serde_json::to_string_pretty(&output)?;
        let mut file = File::create(path)?;
        file.write_all(json_string.as_bytes())?;

        Ok(())
    }
}

fn percentile(sorted_data: &[u64], percentile: f64) -> u64 {
    if sorted_data.is_empty() {
        return 0;
    }

    let index = (percentile / 100.0 * (sorted_data.len() - 1) as f64).round() as usize;
    sorted_data[index.min(sorted_data.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(percentile(&data, 50.0), 5);
        assert_eq!(percentile(&data, 95.0), 10);
        assert_eq!(percentile(&data, 0.0), 1);
    }

    #[test]
    fn test_metrics_aggregation() {
        let mut metrics = ClientMetrics::new("TestClient".to_string());

        metrics.record_request(1, Duration::from_millis(100), true, None, Some(1));
        metrics.record_request(2, Duration::from_millis(200), true, None, Some(2));
        metrics.record_request(3, Duration::from_millis(150), false, Some("timeout".to_string()), Some(1));

        let stats = metrics.aggregate();

        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.successful_requests, 2);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.latency_min_ms, 100);
        assert_eq!(stats.latency_max_ms, 200);
        assert_eq!(stats.server_distribution.get(&1), Some(&2));
        assert_eq!(stats.server_distribution.get(&2), Some(&1));
    }
}
