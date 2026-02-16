use super::traits::{Observer, ObserverEvent, ObserverMetric};
use tracing::info;

/// Log-based observer — uses tracing, zero external deps
pub struct LogObserver;

impl LogObserver {
    pub fn new() -> Self {
        Self
    }
}

impl Observer for LogObserver {
    fn record_event(&self, event: &ObserverEvent) {
        match event {
            ObserverEvent::AgentStart { provider, model } => {
                info!(provider = %provider, model = %model, "agent.start");
            }
            ObserverEvent::AgentEnd {
                duration,
                tokens_used,
            } => {
                let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
                info!(duration_ms = ms, tokens = ?tokens_used, "agent.end");
            }
            ObserverEvent::ToolCall {
                tool,
                duration,
                success,
                source,
            } => {
                let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
                let src = source.as_deref().unwrap_or("unknown");
                info!(tool = %tool, duration_ms = ms, success = success, source = %src, "tool.call");
            }
            ObserverEvent::ChannelMessage {
                channel,
                direction,
                actor,
            } => {
                let who = actor.as_deref().unwrap_or("unknown");
                info!(channel = %channel, direction = %direction, actor = %who, "channel.message");
            }
            ObserverEvent::SecurityEvent {
                event_type,
                detail,
                source,
            } => {
                let src = source.as_deref().unwrap_or("unknown");
                info!(event = %event_type, detail = %detail, source = %src, "security.event");
            }
            ObserverEvent::HeartbeatTick => {
                info!("heartbeat.tick");
            }
            ObserverEvent::Error { component, message } => {
                info!(component = %component, error = %message, "error");
            }
        }
    }

    fn record_metric(&self, metric: &ObserverMetric) {
        match metric {
            ObserverMetric::RequestLatency(d) => {
                let ms = u64::try_from(d.as_millis()).unwrap_or(u64::MAX);
                info!(latency_ms = ms, "metric.request_latency");
            }
            ObserverMetric::TokensUsed(t) => {
                info!(tokens = t, "metric.tokens_used");
            }
            ObserverMetric::ActiveSessions(s) => {
                info!(sessions = s, "metric.active_sessions");
            }
            ObserverMetric::QueueDepth(d) => {
                info!(depth = d, "metric.queue_depth");
            }
        }
    }

    fn name(&self) -> &str {
        "log"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn log_observer_name() {
        assert_eq!(LogObserver::new().name(), "log");
    }

    #[test]
    fn log_observer_all_events_no_panic() {
        let obs = LogObserver::new();
        obs.record_event(&ObserverEvent::AgentStart {
            provider: "openrouter".into(),
            model: "claude-sonnet".into(),
        });
        obs.record_event(&ObserverEvent::AgentEnd {
            duration: Duration::from_millis(500),
            tokens_used: Some(100),
        });
        obs.record_event(&ObserverEvent::AgentEnd {
            duration: Duration::ZERO,
            tokens_used: None,
        });
        obs.record_event(&ObserverEvent::ToolCall {
            tool: "shell".into(),
            duration: Duration::from_millis(10),
            success: false,
            source: Some("cli".into()),
        });
        obs.record_event(&ObserverEvent::ToolCall {
            tool: "file_read".into(),
            duration: Duration::from_millis(5),
            success: true,
            source: None,
        });
        obs.record_event(&ObserverEvent::ChannelMessage {
            channel: "telegram".into(),
            direction: "outbound".into(),
            actor: Some("user123".into()),
        });
        obs.record_event(&ObserverEvent::ChannelMessage {
            channel: "discord".into(),
            direction: "inbound".into(),
            actor: None,
        });
        obs.record_event(&ObserverEvent::SecurityEvent {
            event_type: "injection_detected".into(),
            detail: "Prompt injection in webhook message".into(),
            source: Some("webhook:1.2.3.4".into()),
        });
        obs.record_event(&ObserverEvent::HeartbeatTick);
        obs.record_event(&ObserverEvent::Error {
            component: "provider".into(),
            message: "timeout".into(),
        });
    }

    #[test]
    fn log_observer_all_metrics_no_panic() {
        let obs = LogObserver::new();
        obs.record_metric(&ObserverMetric::RequestLatency(Duration::from_secs(2)));
        obs.record_metric(&ObserverMetric::TokensUsed(0));
        obs.record_metric(&ObserverMetric::TokensUsed(u64::MAX));
        obs.record_metric(&ObserverMetric::ActiveSessions(1));
        obs.record_metric(&ObserverMetric::QueueDepth(999));
    }
}
