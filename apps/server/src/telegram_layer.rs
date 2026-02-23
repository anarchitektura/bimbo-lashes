//! Custom tracing layer that sends ERROR-level events to Telegram.
//!
//! Features:
//! - Rate limiting: at most 1 message per `MIN_INTERVAL` (10 s default)
//! - Deduplication: identical error messages are suppressed for `DEDUP_WINDOW` (60 s)
//! - Non-blocking: Telegram HTTP calls are spawned onto the Tokio runtime

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Minimum interval between Telegram messages (prevents spam on cascading errors).
const MIN_INTERVAL: Duration = Duration::from_secs(10);
/// Window during which identical error hashes are suppressed.
const DEDUP_WINDOW: Duration = Duration::from_secs(60);

// ── Layer ──

/// A `tracing` layer that forwards ERROR events to a Telegram chat.
pub struct TelegramLayer {
    bot_token: String,
    chat_id: i64,
    http: reqwest::Client,
    /// Tracks when we last sent a Telegram message (rate limit).
    state: Mutex<LayerState>,
}

struct LayerState {
    last_sent: Instant,
    /// (hash, inserted_at) of recently sent error messages.
    recent: Vec<(u64, Instant)>,
}

impl TelegramLayer {
    /// Create a new layer. Messages will be sent to `chat_id` via `bot_token`.
    pub fn new(bot_token: String, chat_id: i64) -> Self {
        Self {
            bot_token,
            chat_id,
            http: reqwest::Client::new(),
            state: Mutex::new(LayerState {
                last_sent: Instant::now() - MIN_INTERVAL, // allow first message immediately
                recent: Vec::new(),
            }),
        }
    }
}

impl<S: Subscriber> Layer<S> for TelegramLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        // Only process ERROR events
        if *event.metadata().level() != Level::ERROR {
            return;
        }

        // Extract message fields
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let message = visitor.message();

        // Build formatted text
        let target = event.metadata().target();
        let file = event.metadata().file().unwrap_or("?");
        let line = event
            .metadata()
            .line()
            .map(|l| l.to_string())
            .unwrap_or_else(|| "?".into());

        let now_utc = chrono::Utc::now().format("%H:%M:%S UTC");
        let text = format!(
            "\u{1f6a8} <b>Server Error</b>\n\
             ━━━━━━━━━━━━━━━\n\
             <code>{message}</code>\n\
             ━━━━━━━━━━━━━━━\n\
             \u{1f4cd} {target} ({file}:{line})\n\
             \u{1f550} {now_utc}"
        );

        // ── Rate limit + dedup ──
        let hash = {
            let mut h = DefaultHasher::new();
            message.hash(&mut h);
            h.finish()
        };

        let should_send = {
            let mut state = self.state.lock().unwrap();
            let now = Instant::now();

            // Evict expired dedup entries
            state.recent.retain(|(_, ts)| now.duration_since(*ts) < DEDUP_WINDOW);

            // Check dedup + rate limit
            let is_dup = state.recent.iter().any(|(h, _)| *h == hash);
            let too_soon = now.duration_since(state.last_sent) < MIN_INTERVAL;

            if is_dup || too_soon {
                false
            } else {
                state.last_sent = now;
                state.recent.push((hash, now));
                true
            }
        };

        if !should_send {
            return;
        }

        // ── Spawn async send (non-blocking) ──
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );
        let client = self.http.clone();
        let chat_id = self.chat_id;

        tokio::spawn(async move {
            let _ = client
                .post(&url)
                .json(&serde_json::json!({
                    "chat_id": chat_id,
                    "text": text,
                    "parse_mode": "HTML"
                }))
                .send()
                .await;
        });
    }
}

// ── Field visitor ──

/// Collects `message` (or unnamed) field from a tracing event.
#[derive(Default)]
struct MessageVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl MessageVisitor {
    /// Combined message: the main message plus any structured fields.
    fn message(&self) -> String {
        if self.fields.is_empty() {
            return self.message.clone();
        }
        let extras: Vec<String> = self
            .fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        if self.message.is_empty() {
            extras.join(", ")
        } else {
            format!("{} ({})", self.message, extras.join(", "))
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let val = format!("{:?}", value);
        if field.name() == "message" {
            self.message = val;
        } else {
            self.fields.push((field.name().to_string(), val));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.fields.push((field.name().to_string(), value.to_string()));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .push((field.name().to_string(), value.to_string()));
    }
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    fn make_layer() -> TelegramLayer {
        TelegramLayer::new("fake:token".into(), 12345)
    }

    /// Helper: simulate the rate-limit + dedup logic.
    fn check_should_send(state: &Mutex<LayerState>, hash: u64) -> bool {
        let mut s = state.lock().unwrap();
        let now = Instant::now();
        s.recent
            .retain(|(_, ts)| now.duration_since(*ts) < DEDUP_WINDOW);

        let is_dup = s.recent.iter().any(|(h, _)| *h == hash);
        let too_soon = now.duration_since(s.last_sent) < MIN_INTERVAL;

        if is_dup || too_soon {
            return false;
        }
        s.last_sent = now;
        s.recent.push((hash, now));
        true
    }

    #[test]
    fn test_first_message_allowed() {
        let layer = make_layer();
        assert!(check_should_send(&layer.state, 111));
    }

    #[test]
    fn test_rate_limit_suppresses_second() {
        let layer = make_layer();
        assert!(check_should_send(&layer.state, 111));
        // Different hash but within rate limit window → suppressed
        assert!(!check_should_send(&layer.state, 222));
    }

    #[test]
    fn test_dedup_same_message() {
        let layer = make_layer();
        assert!(check_should_send(&layer.state, 111));

        // Fast-forward past rate limit
        {
            let mut s = layer.state.lock().unwrap();
            s.last_sent = Instant::now() - MIN_INTERVAL;
        }

        // Same hash → suppressed by dedup
        assert!(!check_should_send(&layer.state, 111));
    }

    #[test]
    fn test_different_errors_sent_after_interval() {
        let layer = make_layer();
        assert!(check_should_send(&layer.state, 111));

        // Fast-forward past rate limit
        {
            let mut s = layer.state.lock().unwrap();
            s.last_sent = Instant::now() - MIN_INTERVAL;
        }

        // Different hash → allowed
        assert!(check_should_send(&layer.state, 222));
    }

    #[test]
    fn test_dedup_expires_after_window() {
        let layer = make_layer();
        assert!(check_should_send(&layer.state, 111));

        // Fast-forward past both rate limit and dedup window
        {
            let mut s = layer.state.lock().unwrap();
            s.last_sent = Instant::now() - MIN_INTERVAL;
            // Fake the dedup entry as old
            s.recent.clear();
            s.recent
                .push((111, Instant::now() - DEDUP_WINDOW - Duration::from_secs(1)));
        }

        // Same hash but dedup expired → allowed
        assert!(check_should_send(&layer.state, 111));
    }

    #[test]
    fn test_format_message_basic() {
        let mut v = MessageVisitor::default();
        v.message = "Something failed".into();
        assert_eq!(v.message(), "Something failed");
    }

    #[test]
    fn test_format_message_with_fields() {
        let mut v = MessageVisitor::default();
        v.message = "DB error".into();
        v.fields
            .push(("booking_id".into(), "42".into()));
        assert_eq!(v.message(), "DB error (booking_id=42)");
    }

    #[test]
    fn test_format_message_fields_only() {
        let v = MessageVisitor {
            message: String::new(),
            fields: vec![("error".into(), "timeout".into())],
        };
        assert_eq!(v.message(), "error=timeout");
    }
}
