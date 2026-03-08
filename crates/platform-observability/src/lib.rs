//! Shared observability bootstrapping.

use platform_consensus::LeaseCoordinator;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing_subscriber::EnvFilter;

// ---------------------------------------------------------------------------
// Tracing init (original)
// ---------------------------------------------------------------------------

/// Initializes tracing with an env filter and logs startup metadata.
pub fn init(service_name: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .try_init();

    tracing::info!(service = service_name, "observability initialized");
}

/// Emit lease metrics/events from a consensus coordinator into tracing logs.
pub async fn emit_consensus_snapshot(
    service_name: &str,
    coordinator: &LeaseCoordinator,
    event_limit: usize,
) {
    let metrics = coordinator.metrics_snapshot().await;
    tracing::info!(
        service = service_name,
        acquired = metrics.acquired,
        acquire_failed = metrics.acquire_failed,
        renewed = metrics.renewed,
        released = metrics.released,
        release_rejected = metrics.release_rejected,
        expired = metrics.expired,
        "consensus lease metrics snapshot"
    );

    for event in coordinator.recent_events(event_limit).await {
        tracing::debug!(
            service = service_name,
            kind = ?event.kind,
            resource = %event.resource,
            owner = %event.owner,
            observed_at = ?event.observed_at,
            "consensus lease event"
        );
    }
}

// ---------------------------------------------------------------------------
// Metrics Collection
// ---------------------------------------------------------------------------

/// The kind of metric being recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// A concrete metric value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetricValue {
    CounterValue(u64),
    GaugeValue(f64),
    HistogramValue {
        count: u64,
        sum: f64,
        buckets: Vec<(f64, u64)>,
    },
}

/// A single named metric with labels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    pub name: String,
    pub metric_type: MetricType,
    pub value: MetricValue,
    pub labels: HashMap<String, String>,
    pub updated_at: String,
}

/// In-memory registry of metrics keyed by composite key (name + sorted labels).
#[derive(Debug, Clone, Default)]
pub struct MetricsRegistry {
    pub metrics: HashMap<String, Metric>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            metrics: HashMap::new(),
        }
    }

    /// Build a composite key from name + sorted labels.
    fn composite_key(name: &str, labels: &[(&str, &str)]) -> String {
        let mut sorted: Vec<(&str, &str)> = labels.to_vec();
        sorted.sort_by_key(|(k, _)| *k);
        let label_str: String = sorted
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",");
        if label_str.is_empty() {
            name.to_string()
        } else {
            format!("{name}{{{label_str}}}")
        }
    }

    fn labels_map(labels: &[(&str, &str)]) -> HashMap<String, String> {
        labels
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn now_string() -> String {
        "now".to_string()
    }

    /// Increment a counter metric by `amount`.
    pub fn increment_counter(&mut self, name: &str, labels: &[(&str, &str)], amount: u64) {
        let key = Self::composite_key(name, labels);
        let entry = self.metrics.entry(key).or_insert_with(|| Metric {
            name: name.to_string(),
            metric_type: MetricType::Counter,
            value: MetricValue::CounterValue(0),
            labels: Self::labels_map(labels),
            updated_at: Self::now_string(),
        });
        if let MetricValue::CounterValue(ref mut v) = entry.value {
            *v += amount;
        }
        entry.updated_at = Self::now_string();
    }

    /// Set a gauge metric to `value`.
    pub fn set_gauge(&mut self, name: &str, labels: &[(&str, &str)], value: f64) {
        let key = Self::composite_key(name, labels);
        let entry = self.metrics.entry(key).or_insert_with(|| Metric {
            name: name.to_string(),
            metric_type: MetricType::Gauge,
            value: MetricValue::GaugeValue(0.0),
            labels: Self::labels_map(labels),
            updated_at: Self::now_string(),
        });
        entry.value = MetricValue::GaugeValue(value);
        entry.updated_at = Self::now_string();
    }

    /// Observe a histogram sample.
    pub fn observe_histogram(&mut self, name: &str, labels: &[(&str, &str)], value: f64) {
        let default_buckets: Vec<(f64, u64)> = vec![
            (0.005, 0),
            (0.01, 0),
            (0.025, 0),
            (0.05, 0),
            (0.1, 0),
            (0.25, 0),
            (0.5, 0),
            (1.0, 0),
            (2.5, 0),
            (5.0, 0),
            (10.0, 0),
        ];
        let key = Self::composite_key(name, labels);
        let entry = self.metrics.entry(key).or_insert_with(|| Metric {
            name: name.to_string(),
            metric_type: MetricType::Histogram,
            value: MetricValue::HistogramValue {
                count: 0,
                sum: 0.0,
                buckets: default_buckets.clone(),
            },
            labels: Self::labels_map(labels),
            updated_at: Self::now_string(),
        });
        if let MetricValue::HistogramValue {
            ref mut count,
            ref mut sum,
            ref mut buckets,
        } = entry.value
        {
            *count += 1;
            *sum += value;
            for (bound, ref mut bucket_count) in buckets.iter_mut() {
                if value <= *bound {
                    *bucket_count += 1;
                }
            }
        }
        entry.updated_at = Self::now_string();
    }

    /// Look up a metric by its exact name (no labels).
    pub fn get_metric(&self, name: &str) -> Option<&Metric> {
        self.metrics.get(name)
    }

    /// Return references to every stored metric.
    pub fn get_all_metrics(&self) -> Vec<&Metric> {
        self.metrics.values().collect()
    }

    /// Render all metrics in Prometheus text exposition format.
    pub fn render_prometheus(&self) -> String {
        let mut lines = Vec::new();
        // Group by metric name for TYPE lines
        let mut seen_types: HashMap<&str, bool> = HashMap::new();

        // Sort keys for deterministic output
        let mut keys: Vec<&String> = self.metrics.keys().collect();
        keys.sort();

        for key in keys {
            let m = &self.metrics[key];
            if !seen_types.contains_key(m.name.as_str()) {
                let type_str = match m.metric_type {
                    MetricType::Counter => "counter",
                    MetricType::Gauge => "gauge",
                    MetricType::Histogram => "histogram",
                };
                lines.push(format!("# TYPE {} {}", m.name, type_str));
                seen_types.insert(&m.name, true);
            }

            let label_str = self.format_prom_labels(&m.labels);

            match &m.value {
                MetricValue::CounterValue(v) => {
                    lines.push(format!("{}{} {}", m.name, label_str, v));
                }
                MetricValue::GaugeValue(v) => {
                    lines.push(format!("{}{} {}", m.name, label_str, v));
                }
                MetricValue::HistogramValue {
                    count,
                    sum,
                    buckets,
                } => {
                    for (bound, bucket_count) in buckets {
                        let mut bucket_labels = m.labels.clone();
                        bucket_labels.insert("le".to_string(), format!("{}", bound));
                        let bl = self.format_prom_labels(&bucket_labels);
                        lines.push(format!("{}_bucket{} {}", m.name, bl, bucket_count));
                    }
                    // +Inf bucket
                    let mut inf_labels = m.labels.clone();
                    inf_labels.insert("le".to_string(), "+Inf".to_string());
                    let il = self.format_prom_labels(&inf_labels);
                    lines.push(format!("{}_bucket{} {}", m.name, il, count));

                    lines.push(format!("{}_sum{} {}", m.name, label_str, sum));
                    lines.push(format!("{}_count{} {}", m.name, label_str, count));
                }
            }
        }
        lines.join("\n")
    }

    fn format_prom_labels(&self, labels: &HashMap<String, String>) -> String {
        if labels.is_empty() {
            return String::new();
        }
        let mut sorted: Vec<(&String, &String)> = labels.iter().collect();
        sorted.sort_by_key(|(k, _)| k.to_string());
        let inner: Vec<String> = sorted
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, v))
            .collect();
        format!("{{{}}}", inner.join(","))
    }

    /// Clear all stored metrics.
    pub fn reset(&mut self) {
        self.metrics.clear();
    }
}

// ---------------------------------------------------------------------------
// Health Checks
// ---------------------------------------------------------------------------

/// Overall status of a health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// A single health check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_checked: String,
    pub duration_ms: u64,
}

/// Registry of named health checks.
#[derive(Debug, Clone, Default)]
pub struct HealthCheckRegistry {
    pub checks: HashMap<String, HealthCheck>,
}

impl HealthCheckRegistry {
    pub fn new() -> Self {
        Self {
            checks: HashMap::new(),
        }
    }

    /// Register a health check with default Healthy status.
    pub fn register(&mut self, name: &str) {
        self.checks.insert(
            name.to_string(),
            HealthCheck {
                name: name.to_string(),
                status: HealthStatus::Healthy,
                message: None,
                last_checked: "never".to_string(),
                duration_ms: 0,
            },
        );
    }

    /// Update the status of a previously registered health check.
    pub fn update(
        &mut self,
        name: &str,
        status: HealthStatus,
        message: Option<String>,
        duration_ms: u64,
    ) {
        if let Some(check) = self.checks.get_mut(name) {
            check.status = status;
            check.message = message;
            check.duration_ms = duration_ms;
            check.last_checked = "now".to_string();
        }
    }

    /// Look up a health check by name.
    pub fn get_check(&self, name: &str) -> Option<&HealthCheck> {
        self.checks.get(name)
    }

    /// Return all registered health checks.
    pub fn get_all_checks(&self) -> Vec<&HealthCheck> {
        self.checks.values().collect()
    }

    /// Compute the worst-case overall status across all checks.
    pub fn overall_status(&self) -> HealthStatus {
        let mut has_degraded = false;
        for check in self.checks.values() {
            match check.status {
                HealthStatus::Unhealthy => return HealthStatus::Unhealthy,
                HealthStatus::Degraded => has_degraded = true,
                HealthStatus::Healthy => {}
            }
        }
        if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }

    /// Render a JSON report of all health checks.
    pub fn render_json(&self) -> String {
        #[derive(Serialize)]
        struct Report<'a> {
            overall: &'a str,
            checks: Vec<&'a HealthCheck>,
        }
        let overall = match self.overall_status() {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
        };
        let mut checks: Vec<&HealthCheck> = self.checks.values().collect();
        checks.sort_by(|a, b| a.name.cmp(&b.name));
        let report = Report { overall, checks };
        serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
    }
}

// ---------------------------------------------------------------------------
// Structured Logging Helpers
// ---------------------------------------------------------------------------

/// Log severity level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// A structured log record (data-only representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredLog {
    pub level: LogLevel,
    pub message: String,
    pub service: String,
    pub timestamp: String,
    pub fields: HashMap<String, String>,
}

/// Emit a tracing event at the appropriate level with structured fields.
pub fn log_event(level: LogLevel, service: &str, message: &str, fields: &[(&str, &str)]) {
    let field_str: String = fields
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");

    match level {
        LogLevel::Trace => tracing::trace!(service = service, fields = %field_str, "{}", message),
        LogLevel::Debug => tracing::debug!(service = service, fields = %field_str, "{}", message),
        LogLevel::Info => tracing::info!(service = service, fields = %field_str, "{}", message),
        LogLevel::Warn => tracing::warn!(service = service, fields = %field_str, "{}", message),
        LogLevel::Error => tracing::error!(service = service, fields = %field_str, "{}", message),
    }
}

/// Emit a structured HTTP request log.
pub fn log_request(service: &str, method: &str, path: &str, status: u16, duration_ms: u64) {
    tracing::info!(
        service = service,
        method = method,
        path = path,
        status = status,
        duration_ms = duration_ms,
        "http request"
    );
}

/// Emit a structured error log with optional context.
pub fn log_error(service: &str, error: &str, context: &[(&str, &str)]) {
    let ctx_str: String = context
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ");

    tracing::error!(
        service = service,
        error = error,
        context = %ctx_str,
        "error occurred"
    );
}

// ---------------------------------------------------------------------------
// Service Info
// ---------------------------------------------------------------------------

/// Metadata about a running service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub start_time: String,
}

/// Build a [`ServiceInfo`] snapshot.
pub fn build_service_info(name: &str, version: &str) -> ServiceInfo {
    ServiceInfo {
        name: name.to_string(),
        version: version.to_string(),
        uptime_seconds: 0,
        start_time: "now".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Request Tracing / Span Collection
// ---------------------------------------------------------------------------

/// A single span in a distributed trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation: String,
    pub start_time: String,
    pub duration_ms: Option<u64>,
    pub status: Option<String>,
}

/// In-memory collector for request spans.
#[derive(Debug, Clone, Default)]
pub struct SpanCollector {
    pub spans: HashMap<String, RequestSpan>,
    next_id: u64,
}

impl SpanCollector {
    pub fn new() -> Self {
        Self {
            spans: HashMap::new(),
            next_id: 1,
        }
    }

    fn generate_id(&mut self) -> String {
        let id = format!("span-{:016x}", self.next_id);
        self.next_id += 1;
        id
    }

    fn generate_trace_id(&mut self) -> String {
        let id = format!("trace-{:016x}", self.next_id);
        self.next_id += 1;
        id
    }

    /// Start a new span, optionally parented to an existing span.
    pub fn start_span(&mut self, operation: &str, parent: Option<&str>) -> RequestSpan {
        let trace_id = if let Some(parent_id) = parent {
            // Inherit trace_id from parent
            self.spans
                .get(parent_id)
                .map(|s| s.trace_id.clone())
                .unwrap_or_else(|| self.generate_trace_id())
        } else {
            self.generate_trace_id()
        };

        let span_id = self.generate_id();
        let span = RequestSpan {
            trace_id,
            span_id: span_id.clone(),
            parent_span_id: parent.map(|s| s.to_string()),
            operation: operation.to_string(),
            start_time: "now".to_string(),
            duration_ms: None,
            status: None,
        };
        self.spans.insert(span_id.clone(), span.clone());
        span
    }

    /// Mark a span as finished.
    pub fn finish_span(&mut self, span_id: &str, status: &str, duration_ms: u64) {
        if let Some(span) = self.spans.get_mut(span_id) {
            span.status = Some(status.to_string());
            span.duration_ms = Some(duration_ms);
        }
    }

    /// Look up a span by ID.
    pub fn get_span(&self, span_id: &str) -> Option<&RequestSpan> {
        self.spans.get(span_id)
    }

    /// Return all spans belonging to a given trace.
    pub fn get_trace(&self, trace_id: &str) -> Vec<&RequestSpan> {
        self.spans
            .values()
            .filter(|s| s.trace_id == trace_id)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // MetricsRegistry tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_increment_counter_creates_metric() {
        let mut reg = MetricsRegistry::new();
        reg.increment_counter("requests_total", &[], 1);
        let m = reg.get_metric("requests_total").unwrap();
        assert_eq!(m.metric_type, MetricType::Counter);
        assert_eq!(m.value, MetricValue::CounterValue(1));
    }

    #[test]
    fn test_increment_counter_accumulates() {
        let mut reg = MetricsRegistry::new();
        reg.increment_counter("requests_total", &[], 5);
        reg.increment_counter("requests_total", &[], 3);
        let m = reg.get_metric("requests_total").unwrap();
        assert_eq!(m.value, MetricValue::CounterValue(8));
    }

    #[test]
    fn test_counter_with_labels() {
        let mut reg = MetricsRegistry::new();
        reg.increment_counter("http_requests", &[("method", "GET")], 1);
        reg.increment_counter("http_requests", &[("method", "POST")], 2);
        assert_eq!(reg.metrics.len(), 2);
        let m = reg.get_metric("http_requests{method=GET}").unwrap();
        assert_eq!(m.value, MetricValue::CounterValue(1));
    }

    #[test]
    fn test_labels_sorted_in_key() {
        let mut reg = MetricsRegistry::new();
        reg.increment_counter("x", &[("b", "2"), ("a", "1")], 1);
        reg.increment_counter("x", &[("a", "1"), ("b", "2")], 1);
        // Both should hit the same key
        assert_eq!(reg.metrics.len(), 1);
        let m = reg.get_metric("x{a=1,b=2}").unwrap();
        assert_eq!(m.value, MetricValue::CounterValue(2));
    }

    #[test]
    fn test_set_gauge() {
        let mut reg = MetricsRegistry::new();
        reg.set_gauge("temperature", &[], 36.6);
        let m = reg.get_metric("temperature").unwrap();
        assert_eq!(m.value, MetricValue::GaugeValue(36.6));
    }

    #[test]
    fn test_gauge_overwrites() {
        let mut reg = MetricsRegistry::new();
        reg.set_gauge("temperature", &[], 36.6);
        reg.set_gauge("temperature", &[], 37.0);
        let m = reg.get_metric("temperature").unwrap();
        assert_eq!(m.value, MetricValue::GaugeValue(37.0));
    }

    #[test]
    fn test_gauge_with_labels() {
        let mut reg = MetricsRegistry::new();
        reg.set_gauge("cpu", &[("core", "0")], 0.5);
        reg.set_gauge("cpu", &[("core", "1")], 0.8);
        assert_eq!(reg.metrics.len(), 2);
    }

    #[test]
    fn test_observe_histogram() {
        let mut reg = MetricsRegistry::new();
        reg.observe_histogram("latency", &[], 0.05);
        let m = reg.get_metric("latency").unwrap();
        match &m.value {
            MetricValue::HistogramValue { count, sum, .. } => {
                assert_eq!(*count, 1);
                assert!((sum - 0.05).abs() < f64::EPSILON);
            }
            _ => panic!("expected histogram value"),
        }
    }

    #[test]
    fn test_histogram_multiple_observations() {
        let mut reg = MetricsRegistry::new();
        reg.observe_histogram("latency", &[], 0.05);
        reg.observe_histogram("latency", &[], 0.5);
        reg.observe_histogram("latency", &[], 5.0);
        let m = reg.get_metric("latency").unwrap();
        match &m.value {
            MetricValue::HistogramValue {
                count,
                sum,
                buckets,
            } => {
                assert_eq!(*count, 3);
                assert!((sum - 5.55).abs() < 1e-9);
                // 0.05 bucket: should have 1 (only the 0.05 observation fits)
                let b_005 = buckets
                    .iter()
                    .find(|(b, _)| (*b - 0.05).abs() < 1e-9)
                    .unwrap();
                assert_eq!(b_005.1, 1);
                // 0.5 bucket: should have 2
                let b_05 = buckets
                    .iter()
                    .find(|(b, _)| (*b - 0.5).abs() < 1e-9)
                    .unwrap();
                assert_eq!(b_05.1, 2);
                // 5.0 bucket: should have 3
                let b_5 = buckets
                    .iter()
                    .find(|(b, _)| (*b - 5.0).abs() < 1e-9)
                    .unwrap();
                assert_eq!(b_5.1, 3);
            }
            _ => panic!("expected histogram value"),
        }
    }

    #[test]
    fn test_get_all_metrics() {
        let mut reg = MetricsRegistry::new();
        reg.increment_counter("a", &[], 1);
        reg.set_gauge("b", &[], 2.0);
        assert_eq!(reg.get_all_metrics().len(), 2);
    }

    #[test]
    fn test_get_metric_not_found() {
        let reg = MetricsRegistry::new();
        assert!(reg.get_metric("nonexistent").is_none());
    }

    #[test]
    fn test_reset_clears_all() {
        let mut reg = MetricsRegistry::new();
        reg.increment_counter("a", &[], 1);
        reg.set_gauge("b", &[], 1.0);
        reg.reset();
        assert!(reg.get_all_metrics().is_empty());
    }

    #[test]
    fn test_render_prometheus_counter() {
        let mut reg = MetricsRegistry::new();
        reg.increment_counter("requests_total", &[], 42);
        let output = reg.render_prometheus();
        assert!(output.contains("# TYPE requests_total counter"));
        assert!(output.contains("requests_total 42"));
    }

    #[test]
    fn test_render_prometheus_gauge() {
        let mut reg = MetricsRegistry::new();
        reg.set_gauge("temp", &[("room", "lab")], 22.5);
        let output = reg.render_prometheus();
        assert!(output.contains("# TYPE temp gauge"));
        assert!(output.contains("temp{room=\"lab\"} 22.5"));
    }

    #[test]
    fn test_render_prometheus_histogram() {
        let mut reg = MetricsRegistry::new();
        reg.observe_histogram("dur", &[], 0.3);
        let output = reg.render_prometheus();
        assert!(output.contains("# TYPE dur histogram"));
        assert!(output.contains("dur_sum 0.3"));
        assert!(output.contains("dur_count 1"));
        assert!(output.contains("dur_bucket"));
    }

    #[test]
    fn test_metric_labels_stored() {
        let mut reg = MetricsRegistry::new();
        reg.increment_counter("rpc", &[("service", "auth"), ("method", "login")], 1);
        let m = reg.get_metric("rpc{method=login,service=auth}").unwrap();
        assert_eq!(m.labels.get("service").unwrap(), "auth");
        assert_eq!(m.labels.get("method").unwrap(), "login");
    }

    #[test]
    fn test_composite_key_empty_labels() {
        let key = MetricsRegistry::composite_key("foo", &[]);
        assert_eq!(key, "foo");
    }

    #[test]
    fn test_composite_key_with_labels() {
        let key = MetricsRegistry::composite_key("foo", &[("z", "1"), ("a", "2")]);
        assert_eq!(key, "foo{a=2,z=1}");
    }

    // -----------------------------------------------------------------------
    // HealthCheckRegistry tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_health_check() {
        let mut reg = HealthCheckRegistry::new();
        reg.register("db");
        let check = reg.get_check("db").unwrap();
        assert_eq!(check.status, HealthStatus::Healthy);
        assert_eq!(check.name, "db");
    }

    #[test]
    fn test_update_health_check() {
        let mut reg = HealthCheckRegistry::new();
        reg.register("db");
        reg.update(
            "db",
            HealthStatus::Unhealthy,
            Some("connection lost".to_string()),
            150,
        );
        let check = reg.get_check("db").unwrap();
        assert_eq!(check.status, HealthStatus::Unhealthy);
        assert_eq!(check.message, Some("connection lost".to_string()));
        assert_eq!(check.duration_ms, 150);
    }

    #[test]
    fn test_update_nonexistent_check_is_noop() {
        let mut reg = HealthCheckRegistry::new();
        reg.update("nope", HealthStatus::Unhealthy, None, 0);
        assert!(reg.get_check("nope").is_none());
    }

    #[test]
    fn test_overall_status_healthy() {
        let mut reg = HealthCheckRegistry::new();
        reg.register("a");
        reg.register("b");
        assert_eq!(reg.overall_status(), HealthStatus::Healthy);
    }

    #[test]
    fn test_overall_status_degraded() {
        let mut reg = HealthCheckRegistry::new();
        reg.register("a");
        reg.register("b");
        reg.update("a", HealthStatus::Degraded, None, 0);
        assert_eq!(reg.overall_status(), HealthStatus::Degraded);
    }

    #[test]
    fn test_overall_status_unhealthy() {
        let mut reg = HealthCheckRegistry::new();
        reg.register("a");
        reg.register("b");
        reg.update("a", HealthStatus::Degraded, None, 0);
        reg.update("b", HealthStatus::Unhealthy, None, 0);
        assert_eq!(reg.overall_status(), HealthStatus::Unhealthy);
    }

    #[test]
    fn test_overall_status_empty() {
        let reg = HealthCheckRegistry::new();
        assert_eq!(reg.overall_status(), HealthStatus::Healthy);
    }

    #[test]
    fn test_get_all_checks() {
        let mut reg = HealthCheckRegistry::new();
        reg.register("a");
        reg.register("b");
        reg.register("c");
        assert_eq!(reg.get_all_checks().len(), 3);
    }

    #[test]
    fn test_render_json_structure() {
        let mut reg = HealthCheckRegistry::new();
        reg.register("db");
        reg.update("db", HealthStatus::Healthy, Some("ok".to_string()), 5);
        let json = reg.render_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["overall"], "healthy");
        assert!(parsed["checks"].is_array());
        assert_eq!(parsed["checks"][0]["name"], "db");
    }

    #[test]
    fn test_render_json_degraded() {
        let mut reg = HealthCheckRegistry::new();
        reg.register("cache");
        reg.update("cache", HealthStatus::Degraded, None, 10);
        let json = reg.render_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["overall"], "degraded");
    }

    // -----------------------------------------------------------------------
    // Structured Logging tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_structured_log_creation() {
        let log = StructuredLog {
            level: LogLevel::Info,
            message: "test message".to_string(),
            service: "auth".to_string(),
            timestamp: "now".to_string(),
            fields: HashMap::new(),
        };
        assert_eq!(log.level, LogLevel::Info);
        assert_eq!(log.service, "auth");
    }

    #[test]
    fn test_structured_log_with_fields() {
        let mut fields = HashMap::new();
        fields.insert("user_id".to_string(), "123".to_string());
        let log = StructuredLog {
            level: LogLevel::Warn,
            message: "rate limited".to_string(),
            service: "api".to_string(),
            timestamp: "now".to_string(),
            fields,
        };
        assert_eq!(log.fields.get("user_id").unwrap(), "123");
    }

    #[test]
    fn test_log_event_does_not_panic() {
        // Ensure the function can be called without panic at each level
        log_event(LogLevel::Trace, "svc", "trace msg", &[]);
        log_event(LogLevel::Debug, "svc", "debug msg", &[("key", "val")]);
        log_event(LogLevel::Info, "svc", "info msg", &[]);
        log_event(LogLevel::Warn, "svc", "warn msg", &[]);
        log_event(LogLevel::Error, "svc", "error msg", &[]);
    }

    #[test]
    fn test_log_request_does_not_panic() {
        log_request("api", "GET", "/health", 200, 12);
        log_request("api", "POST", "/login", 401, 45);
    }

    #[test]
    fn test_log_error_does_not_panic() {
        log_error("worker", "timeout", &[("queue", "high")]);
        log_error("worker", "crash", &[]);
    }

    #[test]
    fn test_log_level_variants() {
        let levels = vec![
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];
        assert_eq!(levels.len(), 5);
        assert_ne!(LogLevel::Trace, LogLevel::Error);
    }

    // -----------------------------------------------------------------------
    // ServiceInfo tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_service_info() {
        let info = build_service_info("gateway", "1.2.3");
        assert_eq!(info.name, "gateway");
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.uptime_seconds, 0);
    }

    #[test]
    fn test_service_info_serialization() {
        let info = build_service_info("api", "0.1.0");
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"name\":\"api\""));
        assert!(json.contains("\"version\":\"0.1.0\""));
    }

    #[test]
    fn test_service_info_deserialization() {
        let json = r#"{"name":"x","version":"1.0","uptime_seconds":100,"start_time":"t0"}"#;
        let info: ServiceInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "x");
        assert_eq!(info.uptime_seconds, 100);
    }

    // -----------------------------------------------------------------------
    // SpanCollector tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_start_span_no_parent() {
        let mut collector = SpanCollector::new();
        let span = collector.start_span("GET /api", None);
        assert!(!span.trace_id.is_empty());
        assert!(!span.span_id.is_empty());
        assert!(span.parent_span_id.is_none());
        assert_eq!(span.operation, "GET /api");
        assert!(span.duration_ms.is_none());
        assert!(span.status.is_none());
    }

    #[test]
    fn test_start_span_with_parent() {
        let mut collector = SpanCollector::new();
        let parent = collector.start_span("request", None);
        let child = collector.start_span("db_query", Some(&parent.span_id));
        assert_eq!(child.trace_id, parent.trace_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
        assert_ne!(child.span_id, parent.span_id);
    }

    #[test]
    fn test_finish_span() {
        let mut collector = SpanCollector::new();
        let span = collector.start_span("op", None);
        collector.finish_span(&span.span_id, "ok", 42);
        let finished = collector.get_span(&span.span_id).unwrap();
        assert_eq!(finished.status, Some("ok".to_string()));
        assert_eq!(finished.duration_ms, Some(42));
    }

    #[test]
    fn test_finish_nonexistent_span_is_noop() {
        let mut collector = SpanCollector::new();
        collector.finish_span("nonexistent", "ok", 0);
        assert!(collector.get_span("nonexistent").is_none());
    }

    #[test]
    fn test_get_trace() {
        let mut collector = SpanCollector::new();
        let root = collector.start_span("root", None);
        let _child1 = collector.start_span("child1", Some(&root.span_id));
        let _child2 = collector.start_span("child2", Some(&root.span_id));
        // A separate trace
        let _other = collector.start_span("other", None);

        let trace = collector.get_trace(&root.trace_id);
        assert_eq!(trace.len(), 3); // root + 2 children
    }

    #[test]
    fn test_get_trace_empty() {
        let collector = SpanCollector::new();
        assert!(collector.get_trace("nonexistent").is_empty());
    }

    #[test]
    fn test_span_ids_unique() {
        let mut collector = SpanCollector::new();
        let s1 = collector.start_span("a", None);
        let s2 = collector.start_span("b", None);
        let s3 = collector.start_span("c", None);
        assert_ne!(s1.span_id, s2.span_id);
        assert_ne!(s2.span_id, s3.span_id);
        assert_ne!(s1.trace_id, s2.trace_id);
    }

    #[test]
    fn test_span_collector_stores_spans() {
        let mut collector = SpanCollector::new();
        let span = collector.start_span("test_op", None);
        assert!(collector.get_span(&span.span_id).is_some());
        assert_eq!(collector.spans.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Serialization round-trip tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_metric_type_serde() {
        let json = serde_json::to_string(&MetricType::Counter).unwrap();
        let back: MetricType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, MetricType::Counter);
    }

    #[test]
    fn test_health_status_serde() {
        for status in &[
            HealthStatus::Healthy,
            HealthStatus::Degraded,
            HealthStatus::Unhealthy,
        ] {
            let json = serde_json::to_string(status).unwrap();
            let back: HealthStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, status);
        }
    }

    #[test]
    fn test_health_check_serde() {
        let check = HealthCheck {
            name: "db".to_string(),
            status: HealthStatus::Healthy,
            message: Some("all good".to_string()),
            last_checked: "now".to_string(),
            duration_ms: 5,
        };
        let json = serde_json::to_string(&check).unwrap();
        let back: HealthCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(back, check);
    }

    #[test]
    fn test_request_span_serde() {
        let span = RequestSpan {
            trace_id: "t1".to_string(),
            span_id: "s1".to_string(),
            parent_span_id: None,
            operation: "op".to_string(),
            start_time: "now".to_string(),
            duration_ms: Some(10),
            status: Some("ok".to_string()),
        };
        let json = serde_json::to_string(&span).unwrap();
        let back: RequestSpan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, span);
    }

    #[test]
    fn test_metric_value_serde_counter() {
        let val = MetricValue::CounterValue(42);
        let json = serde_json::to_string(&val).unwrap();
        let back: MetricValue = serde_json::from_str(&json).unwrap();
        assert_eq!(back, val);
    }

    #[test]
    fn test_metric_value_serde_gauge() {
        let val = MetricValue::GaugeValue(3.14);
        let json = serde_json::to_string(&val).unwrap();
        let back: MetricValue = serde_json::from_str(&json).unwrap();
        assert_eq!(back, val);
    }

    #[test]
    fn test_metric_value_serde_histogram() {
        let val = MetricValue::HistogramValue {
            count: 10,
            sum: 25.5,
            buckets: vec![(1.0, 5), (5.0, 9)],
        };
        let json = serde_json::to_string(&val).unwrap();
        let back: MetricValue = serde_json::from_str(&json).unwrap();
        assert_eq!(back, val);
    }
}
