use std::time::Duration;

/// Events the observer can record.
///
/// Events include optional attribution fields (`source`, `actor`) to support
/// audit logging — a key identity governance requirement for agent security.
#[derive(Debug, Clone)]
pub enum ObserverEvent {
    AgentStart {
        provider: String,
        model: String,
    },
    AgentEnd {
        duration: Duration,
        tokens_used: Option<u64>,
    },
    ToolCall {
        tool: String,
        duration: Duration,
        success: bool,
        /// Who/what triggered this tool call (e.g. "webhook", "discord:user123", "cli")
        source: Option<String>,
    },
    ChannelMessage {
        channel: String,
        direction: String,
        /// The sender/actor for inbound messages
        actor: Option<String>,
    },
    /// A security-relevant event (injection detected, rate limit hit, etc.)
    SecurityEvent {
        event_type: String,
        detail: String,
        source: Option<String>,
    },
    HeartbeatTick,
    Error {
        component: String,
        message: String,
    },
}

/// Numeric metrics
#[derive(Debug, Clone)]
pub enum ObserverMetric {
    RequestLatency(Duration),
    TokensUsed(u64),
    ActiveSessions(u64),
    QueueDepth(u64),
}

/// Core observability trait — implement for any backend
pub trait Observer: Send + Sync {
    /// Record a discrete event
    fn record_event(&self, event: &ObserverEvent);

    /// Record a numeric metric
    fn record_metric(&self, metric: &ObserverMetric);

    /// Flush any buffered data (no-op for most backends)
    fn flush(&self) {}

    /// Human-readable name of this observer
    fn name(&self) -> &str;
}
