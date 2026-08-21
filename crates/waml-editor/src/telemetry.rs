//! In-process telemetry: the editor's `tracing` seam (review O-3).
//!
//! [`init`] installs the global `tracing` subscriber once, from app startup on
//! both targets. Every event lands in a bounded in-memory ring buffer
//! ([`recent_events`] snapshots it for a future in-editor log panel), and is
//! mirrored somewhere a person can see it today:
//!
//! * native -- a `tracing_subscriber::fmt` layer on stderr, `info` by default
//!   and overridable via `RUST_LOG`;
//! * wasm -- makepad's `log!` path into the browser console (no `web_sys`,
//!   no extra deps: the same channel every existing `log!` call uses).
//!
//! Field names follow OTEL semantic conventions where natural (e.g.
//! `error.message`), so a later OTLP export can map events without a rename
//! sweep. No OpenTelemetry dependency lives here on purpose: OTEL is an
//! export target, not a facade.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

/// Ring capacity: enough to hold a session's worth of failures without ever
/// growing (drop-oldest past this). ~8k events of a few hundred bytes keeps
/// the worst case in low single-digit MB.
const CAPACITY: usize = 8192;

/// One captured `tracing` event, decoupled from the borrow-laden originals so
/// a snapshot can outlive the dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TelemetryEvent {
    pub level: tracing::Level,
    /// The emitting module path (tracing's `target`).
    pub target: String,
    /// Native: microseconds since the Unix epoch. wasm: a monotonic per-event
    /// counter -- `SystemTime::now()` PANICS on `wasm32-unknown-unknown` (see
    /// `waml::bundle_envelope::production_nonce`), and ordering is what a log
    /// panel actually needs.
    pub timestamp_us: u64,
    /// The event's `message` field, formatted.
    pub message: String,
    /// Every other field, formatted, in recording order. Names follow OTEL
    /// semantic conventions where one exists (`error.message`, ...).
    pub fields: Vec<(String, String)>,
}

/// Bounded drop-oldest buffer of captured events.
struct RingBuffer {
    capacity: usize,
    events: VecDeque<TelemetryEvent>,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            events: VecDeque::new(),
        }
    }

    fn push(&mut self, event: TelemetryEvent) {
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Test seam: reached through `recent_events`, whose own consumer (an
    /// in-editor log panel) has not landed.
    #[cfg(test)]
    fn snapshot(&self) -> Vec<TelemetryEvent> {
        self.events.iter().cloned().collect()
    }
}

/// Shared handle to a ring: the layer writes through it, [`recent_events`]
/// reads through it, and a test can hold a private one.
type SharedRing = Arc<Mutex<RingBuffer>>;

fn global_ring() -> &'static SharedRing {
    static RING: OnceLock<SharedRing> = OnceLock::new();
    RING.get_or_init(|| Arc::new(Mutex::new(RingBuffer::new(CAPACITY))))
}

#[cfg(not(target_arch = "wasm32"))]
fn now_micros() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

/// `SystemTime::now()` PANICS on `wasm32-unknown-unknown`; a monotonic counter
/// preserves event order, which is what the buffer needs the stamp for.
#[cfg(target_arch = "wasm32")]
fn now_micros() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static TICK: AtomicU64 = AtomicU64::new(0);
    TICK.fetch_add(1, Ordering::Relaxed)
}

/// Splits an event into `message` vs named fields, formatting values.
#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl FieldVisitor {
    fn record(&mut self, name: &str, value: String) {
        if name == "message" {
            self.message = value;
        } else {
            self.fields.push((name.to_string(), value));
        }
    }
}

impl tracing::field::Visit for FieldVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.record(field.name(), value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.record(field.name(), format!("{value:?}"));
    }
}

/// The capture layer: every event into the ring, unfiltered (the ring is the
/// bound). On wasm it also mirrors the formatted line to the browser console,
/// standing in for the native stderr layer.
struct RingLayer {
    ring: SharedRing,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for RingLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let meta = event.metadata();
        let captured = TelemetryEvent {
            level: *meta.level(),
            target: meta.target().to_string(),
            timestamp_us: now_micros(),
            message: visitor.message,
            fields: visitor.fields,
        };
        #[cfg(target_arch = "wasm32")]
        {
            // Mirror through makepad's console path -- the exact channel the
            // pre-tracing `log!` calls used -- so the browser build stays as
            // loud as native.
            let fields: String = captured
                .fields
                .iter()
                .map(|(name, value)| format!(" {name}={value}"))
                .collect();
            makepad_widgets::log!(
                "{} {}: {}{}",
                captured.level,
                captured.target,
                captured.message,
                fields
            );
        }
        let mut ring = match self.ring.lock() {
            Ok(ring) => ring,
            Err(poisoned) => poisoned.into_inner(),
        };
        ring.push(captured);
    }
}

/// Install the global subscriber. Idempotent: extra calls (or a subscriber
/// some test already installed) are a no-op, never a panic.
pub fn init() {
    use tracing_subscriber::layer::SubscriberExt;
    let ring_layer = RingLayer {
        ring: Arc::clone(global_ring()),
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        use tracing_subscriber::Layer;
        // `info` unless RUST_LOG says otherwise. The filter gates only the
        // stderr mirror: the ring captures everything below it, so a log
        // panel can show what the console was too quiet to print.
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
        let subscriber = tracing_subscriber::registry().with(ring_layer).with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_filter(filter),
        );
        let _ = tracing::subscriber::set_global_default(subscriber);
    }
    #[cfg(target_arch = "wasm32")]
    {
        // No env, no stderr in the browser: the ring layer alone, which also
        // mirrors to the console (see `RingLayer::on_event`).
        let subscriber = tracing_subscriber::registry().with(ring_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(n: u64) -> TelemetryEvent {
        TelemetryEvent {
            level: tracing::Level::INFO,
            target: "test".to_string(),
            timestamp_us: n,
            message: format!("event {n}"),
            fields: Vec::new(),
        }
    }

    #[test]
    fn ring_holds_up_to_capacity() {
        let mut ring = RingBuffer::new(3);
        ring.push(event(1));
        ring.push(event(2));
        let stamps: Vec<u64> = ring.snapshot().iter().map(|e| e.timestamp_us).collect();
        assert_eq!(stamps, vec![1, 2]);
    }

    #[test]
    fn ring_drops_oldest_past_capacity() {
        let mut ring = RingBuffer::new(3);
        for n in 1..=5 {
            ring.push(event(n));
        }
        let stamps: Vec<u64> = ring.snapshot().iter().map(|e| e.timestamp_us).collect();
        assert_eq!(stamps, vec![3, 4, 5], "oldest two must have been dropped");
    }

    #[test]
    fn snapshot_is_a_copy_not_a_drain() {
        let mut ring = RingBuffer::new(3);
        ring.push(event(1));
        assert_eq!(ring.snapshot().len(), 1);
        assert_eq!(ring.snapshot().len(), 1, "snapshot must not consume");
    }

    /// Dispatch through a real subscriber into a private ring: level, target,
    /// message, and non-message fields (OTEL-style dotted names included)
    /// must all survive capture.
    #[test]
    fn layer_captures_dispatched_events() {
        use tracing_subscriber::layer::SubscriberExt;
        let ring: SharedRing = Arc::new(Mutex::new(RingBuffer::new(8)));
        let subscriber = tracing_subscriber::registry().with(RingLayer {
            ring: Arc::clone(&ring),
        });
        tracing::subscriber::with_default(subscriber, || {
            // NB: a dotted field name directly after `target:` trips a macro
            // ambiguity in tracing 0.1, so this exercises the default target
            // (the module path), which is what the call sites use anyway.
            tracing::warn!(error.message = "disk full", "save failed");
            tracing::info!("opened");
        });
        let events = ring.lock().unwrap().snapshot();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].level, tracing::Level::WARN);
        assert_eq!(events[0].target, module_path!());
        assert_eq!(events[0].message, "save failed");
        assert_eq!(
            events[0].fields,
            vec![("error.message".to_string(), "disk full".to_string())]
        );
        assert_eq!(events[1].message, "opened");
        assert!(events[1].fields.is_empty());
    }
}
