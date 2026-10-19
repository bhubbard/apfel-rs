// ============================================================================
// metrics.rs — Production-grade Prometheus Metrics & Observability Registry
// Part of apfel-rs
// ============================================================================

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, RwLock};
use std::time::Instant;

/// Global singleton metrics registry for apfel-rs.
static GLOBAL_METRICS: LazyLock<Arc<ApfelMetrics>> = LazyLock::new(|| Arc::new(ApfelMetrics::new()));

/// Fixed bucket definitions for Prometheus histograms.
pub const TTFT_BUCKETS: &[f64] = &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0];
pub const DURATION_BUCKETS: &[f64] = &[0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0];
pub const TOKENS_PER_SEC_BUCKETS: &[f64] = &[5.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 80.0, 100.0];

/// Atomic histogram with fixed buckets.
#[derive(Debug)]
pub struct AtomicHistogram {
    buckets: &'static [f64],
    bucket_counts: Vec<AtomicU64>,
    count: AtomicU64,
    sum: AtomicU64, // Microseconds stored as integer to allow lock-free atomic addition
}

impl AtomicHistogram {
    pub fn new(buckets: &'static [f64]) -> Self {
        let mut bucket_counts = Vec::with_capacity(buckets.len());
        for _ in 0..buckets.len() {
            bucket_counts.push(AtomicU64::new(0));
        }
        Self {
            buckets,
            bucket_counts,
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
        }
    }

    pub fn observe(&self, val_secs: f64) {
        if val_secs < 0.0 || !val_secs.is_finite() {
            return;
        }
        for (i, &le) in self.buckets.iter().enumerate() {
            if val_secs <= le {
                self.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        let val_micros = (val_secs * 1_000_000.0) as u64;
        self.sum.fetch_add(val_micros, Ordering::Relaxed);
    }

    pub fn render(&self, metric_name: &str, help: &str, out: &mut String) {
        out.push_str(&format!("# HELP {} {}\n", metric_name, help));
        out.push_str(&format!("# TYPE {} histogram\n", metric_name));

        for (i, &le) in self.buckets.iter().enumerate() {
            let count = self.bucket_counts[i].load(Ordering::Relaxed);
            out.push_str(&format!("{}_bucket{{le=\"{}\"}} {}\n", metric_name, le, count));
        }
        let total_count = self.count.load(Ordering::Relaxed);
        out.push_str(&format!("{}_bucket{{le=\"+Inf\"}} {}\n", metric_name, total_count));
        let sum_secs = (self.sum.load(Ordering::Relaxed) as f64) / 1_000_000.0;
        out.push_str(&format!("{}_sum {:.6}\n", metric_name, sum_secs));
        out.push_str(&format!("{}_count {}\n\n", metric_name, total_count));
    }
}

/// Key identifying endpoint request counts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RequestKey {
    endpoint: String,
    status: u16,
}

/// Central metrics collector and reporter for apfel-rs.
#[derive(Debug)]
pub struct ApfelMetrics {
    // Counters
    requests: RwLock<HashMap<RequestKey, Arc<AtomicU64>>>,
    pub prompt_tokens_total: AtomicU64,
    pub generated_tokens_total: AtomicU64,
    pub errors_total: AtomicU64,
    pub cache_hits_total: AtomicU64,
    pub cache_misses_total: AtomicU64,

    // Gauges
    pub active_requests: AtomicI64,
    pub active_streaming_connections: AtomicI64,
    pub last_tokens_per_second: AtomicU64, // tokens/sec * 1000

    // Histograms
    pub time_to_first_token: AtomicHistogram,
    pub request_duration: AtomicHistogram,
    pub tokens_per_second_rate: AtomicHistogram,
}

impl Default for ApfelMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ApfelMetrics {
    /// Creates a new metrics registry.
    pub fn new() -> Self {
        Self {
            requests: RwLock::new(HashMap::new()),
            prompt_tokens_total: AtomicU64::new(0),
            generated_tokens_total: AtomicU64::new(0),
            errors_total: AtomicU64::new(0),
            cache_hits_total: AtomicU64::new(0),
            cache_misses_total: AtomicU64::new(0),
            active_requests: AtomicI64::new(0),
            active_streaming_connections: AtomicI64::new(0),
            last_tokens_per_second: AtomicU64::new(0),
            time_to_first_token: AtomicHistogram::new(TTFT_BUCKETS),
            request_duration: AtomicHistogram::new(DURATION_BUCKETS),
            tokens_per_second_rate: AtomicHistogram::new(TOKENS_PER_SEC_BUCKETS),
        }
    }

    /// Access the global singleton instance.
    pub fn global() -> Arc<Self> {
        GLOBAL_METRICS.clone()
    }

    /// Increments request count for an endpoint and status code.
    pub fn record_request(&self, endpoint: &str, status: u16, duration_secs: f64) {
        let key = RequestKey {
            endpoint: endpoint.to_string(),
            status,
        };

        let counter = {
            let read_guard = self.requests.read().unwrap();
            read_guard.get(&key).cloned()
        };

        let counter = match counter {
            Some(c) => c,
            None => {
                let mut write_guard = self.requests.write().unwrap();
                write_guard
                    .entry(key)
                    .or_insert_with(|| Arc::new(AtomicU64::new(0)))
                    .clone()
            }
        };

        counter.fetch_add(1, Ordering::Relaxed);
        self.request_duration.observe(duration_secs);

        if status >= 400 {
            self.errors_total.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records Time To First Token in seconds.
    pub fn record_ttft(&self, ttft_secs: f64) {
        self.time_to_first_token.observe(ttft_secs);
    }

    /// Records prompt and completion tokens, plus speed throughput.
    pub fn record_tokens(&self, prompt_tokens: usize, generated_tokens: usize, tok_per_sec: f64) {
        self.prompt_tokens_total
            .fetch_add(prompt_tokens as u64, Ordering::Relaxed);
        self.generated_tokens_total
            .fetch_add(generated_tokens as u64, Ordering::Relaxed);

        if tok_per_sec > 0.0 && tok_per_sec.is_finite() {
            self.last_tokens_per_second
                .store((tok_per_sec * 1000.0) as u64, Ordering::Relaxed);
            self.tokens_per_second_rate.observe(tok_per_sec);
        }
    }

    /// Records cache hit.
    pub fn record_cache_hit(&self) {
        self.cache_hits_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Records cache miss.
    pub fn record_cache_miss(&self) {
        self.cache_misses_total.fetch_add(1, Ordering::Relaxed);
    }

    /// RAII guard that tracks in-flight active requests.
    pub fn start_request(self: &Arc<Self>) -> ActiveRequestGuard {
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        ActiveRequestGuard {
            metrics: Arc::clone(self),
            start_time: Instant::now(),
        }
    }

    /// RAII guard that tracks in-flight SSE streaming connections.
    pub fn start_streaming(self: &Arc<Self>) -> ActiveStreamingGuard {
        self.active_streaming_connections
            .fetch_add(1, Ordering::Relaxed);
        ActiveStreamingGuard {
            metrics: Arc::clone(self),
        }
    }

    /// Current resident set size (RSS) memory in bytes.
    pub fn resident_memory_bytes() -> u64 {
        #[cfg(target_os = "macos")]
        #[allow(deprecated)]
        unsafe {
            let mut info: libc::mach_task_basic_info = std::mem::zeroed();
            let mut count = (std::mem::size_of::<libc::mach_task_basic_info>()
                / std::mem::size_of::<libc::integer_t>())
                as libc::mach_msg_type_number_t;
            let kret = libc::task_info(
                libc::mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                &mut info as *mut libc::mach_task_basic_info as libc::task_info_t,
                &mut count,
            );
            if kret == libc::KERN_SUCCESS {
                info.resident_size
            } else {
                0
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            0
        }
    }

    /// Formats all metrics into standard Prometheus text exposition format.
    pub fn render_prometheus(&self) -> String {
        let mut out = String::with_capacity(4096);

        out.push_str("# HELP apfel_requests_total Total HTTP requests handled by apfel-rs\n");
        out.push_str("# TYPE apfel_requests_total counter\n");
        {
            let requests = self.requests.read().unwrap();
            let mut keys: Vec<&RequestKey> = requests.keys().collect();
            keys.sort_by(|a, b| (&a.endpoint, a.status).cmp(&(&b.endpoint, b.status)));
            for key in keys {
                let count = requests[key].load(Ordering::Relaxed);
                out.push_str(&format!(
                    "apfel_requests_total{{endpoint=\"{}\",status=\"{}\"}} {}\n",
                    key.endpoint, key.status, count
                ));
            }
            if requests.is_empty() {
                out.push_str("apfel_requests_total{endpoint=\"/health\",status=\"200\"} 0\n");
            }
        }
        out.push('\n');

        out.push_str("# HELP apfel_active_requests Number of requests currently being processed\n");
        out.push_str("# TYPE apfel_active_requests gauge\n");
        out.push_str(&format!(
            "apfel_active_requests {}\n\n",
            self.active_requests.load(Ordering::Relaxed)
        ));

        out.push_str(
            "# HELP apfel_active_streaming_connections Number of active SSE streaming connections\n",
        );
        out.push_str("# TYPE apfel_active_streaming_connections gauge\n");
        out.push_str(&format!(
            "apfel_active_streaming_connections {}\n\n",
            self.active_streaming_connections.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP apfel_prompt_tokens_total Total prompt tokens processed\n");
        out.push_str("# TYPE apfel_prompt_tokens_total counter\n");
        out.push_str(&format!(
            "apfel_prompt_tokens_total {}\n\n",
            self.prompt_tokens_total.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP apfel_generated_tokens_total Total tokens generated by the model\n");
        out.push_str("# TYPE apfel_generated_tokens_total counter\n");
        out.push_str(&format!(
            "apfel_generated_tokens_total {}\n\n",
            self.generated_tokens_total.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP apfel_errors_total Total errors encountered\n");
        out.push_str("# TYPE apfel_errors_total counter\n");
        out.push_str(&format!(
            "apfel_errors_total {}\n\n",
            self.errors_total.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP apfel_cache_hits_total Total semantic and context cache hits\n");
        out.push_str("# TYPE apfel_cache_hits_total counter\n");
        out.push_str(&format!(
            "apfel_cache_hits_total {}\n\n",
            self.cache_hits_total.load(Ordering::Relaxed)
        ));

        out.push_str("# HELP apfel_cache_misses_total Total cache misses\n");
        out.push_str("# TYPE apfel_cache_misses_total counter\n");
        out.push_str(&format!(
            "apfel_cache_misses_total {}\n\n",
            self.cache_misses_total.load(Ordering::Relaxed)
        ));

        let last_speed = (self.last_tokens_per_second.load(Ordering::Relaxed) as f64) / 1000.0;
        out.push_str("# HELP apfel_tokens_per_second Generation speed in tokens per second\n");
        out.push_str("# TYPE apfel_tokens_per_second gauge\n");
        out.push_str(&format!("apfel_tokens_per_second {:.2}\n\n", last_speed));

        let rss = Self::resident_memory_bytes();
        out.push_str("# HELP apfel_process_resident_memory_bytes Resident memory size in bytes\n");
        out.push_str("# TYPE apfel_process_resident_memory_bytes gauge\n");
        out.push_str(&format!("apfel_process_resident_memory_bytes {}\n\n", rss));

        let thermal = crate::core::thermal::PowerThermalMonitor::current_thermal_state();
        let thermal_val = match thermal {
            crate::core::thermal::ThermalState::Nominal => 0,
            crate::core::thermal::ThermalState::Fair => 1,
            crate::core::thermal::ThermalState::Serious => 2,
            crate::core::thermal::ThermalState::Critical => 3,
        };
        out.push_str("# HELP apfel_system_thermal_pressure System thermal pressure level (0=Nominal, 1=Fair, 2=Serious, 3=Critical)\n");
        out.push_str("# TYPE apfel_system_thermal_pressure gauge\n");
        out.push_str(&format!("apfel_system_thermal_pressure {}\n\n", thermal_val));

        self.time_to_first_token.render(
            "apfel_time_to_first_token_seconds",
            "Time to first token in seconds",
            &mut out,
        );

        self.request_duration.render(
            "apfel_request_duration_seconds",
            "HTTP request duration in seconds",
            &mut out,
        );

        self.tokens_per_second_rate.render(
            "apfel_tokens_per_second_rate",
            "Distribution of generation speed in tokens per second",
            &mut out,
        );

        out
    }
}

/// RAII Guard decrementing active requests when dropped.
pub struct ActiveRequestGuard {
    metrics: Arc<ApfelMetrics>,
    pub start_time: Instant,
}

impl ActiveRequestGuard {
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

impl Drop for ActiveRequestGuard {
    fn drop(&mut self) {
        self.metrics.active_requests.fetch_sub(1, Ordering::Relaxed);
    }
}

/// RAII Guard decrementing active streaming connections when dropped.
pub struct ActiveStreamingGuard {
    metrics: Arc<ApfelMetrics>,
}

impl Drop for ActiveStreamingGuard {
    fn drop(&mut self) {
        self.metrics
            .active_streaming_connections
            .fetch_sub(1, Ordering::Relaxed);
    }
}
