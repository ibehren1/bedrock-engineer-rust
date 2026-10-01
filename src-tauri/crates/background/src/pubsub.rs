//! `src/main/lib/pubsub-manager.ts` and `src/main/handlers/pubsub-handlers.ts`.
//!
//! Subscribers are window labels. A message published on channel `c` is sent to each subscriber
//! of `c` as the event [`pubsub_event_name`]`(c)` (BRIDGE.md rule 5), with the published data as
//! the payload. Channels without subscribers are dropped.

use crate::events::EventSink;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// `pubsubEventName(channel)` from `tauriBridge.ts`: `pubsub:` + the channel with every UTF-16
/// unit outside `[A-Za-z0-9\-/:_]` replaced by `_`.
pub fn pubsub_event_name(channel: &str) -> String {
    let mut out = String::from("pubsub:");
    for c in channel.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | ':' | '_') {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push('_');
            }
        }
    }
    out
}

/// One entry of [`PubSubStats::channels`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelStats {
    pub channel: String,
    pub subscriber_count: usize,
}

/// `getStats()` (`pubsub:stats`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PubSubStats {
    pub total_channels: usize,
    pub total_subscribers: usize,
    pub channels: Vec<ChannelStats>,
}

/// `PubSubManager`.
pub struct PubSubManager {
    sink: Arc<dyn EventSink>,
    /// Channel → subscriber labels, in subscription order.
    subscribers: Mutex<Vec<(String, Vec<String>)>>,
}

impl PubSubManager {
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        PubSubManager {
            sink,
            subscribers: Mutex::new(Vec::new()),
        }
    }

    /// `pubsub:subscribe { channel }` from window `subscriber`.
    pub fn subscribe(&self, channel: &str, subscriber: &str) {
        let mut subs = self.subscribers.lock().unwrap();
        match subs.iter_mut().find(|(c, _)| c == channel) {
            Some((_, list)) => {
                if !list.iter().any(|s| s == subscriber) {
                    list.push(subscriber.to_string());
                }
            }
            None => subs.push((channel.to_string(), vec![subscriber.to_string()])),
        }
        tracing::debug!(channel, "Subscription added");
    }

    /// `pubsub:unsubscribe { channel }` from window `subscriber`.
    pub fn unsubscribe(&self, channel: &str, subscriber: &str) {
        let mut subs = self.subscribers.lock().unwrap();
        if let Some(i) = subs.iter().position(|(c, _)| c == channel) {
            subs[i].1.retain(|s| s != subscriber);
            if subs[i].1.is_empty() {
                subs.remove(i);
            }
        }
    }

    /// `unsubscribeAll(webContents)`; call when a window is destroyed.
    pub fn unsubscribe_all(&self, subscriber: &str) {
        let mut subs = self.subscribers.lock().unwrap();
        for (_, list) in subs.iter_mut() {
            list.retain(|s| s != subscriber);
        }
        subs.retain(|(_, list)| !list.is_empty());
    }

    /// `pubsub:publish { channel, data }`.
    pub fn publish(&self, channel: &str, data: Value) {
        let targets: Vec<String> = {
            let subs = self.subscribers.lock().unwrap();
            match subs.iter().find(|(c, _)| c == channel) {
                Some((_, list)) if !list.is_empty() => list.clone(),
                _ => {
                    tracing::debug!(channel, "No subscribers for channel");
                    return;
                }
            }
        };
        let event = pubsub_event_name(channel);
        let failed: Vec<String> = targets
            .into_iter()
            .filter(|t| {
                self.sink
                    .emit_to(t, &event, data.clone())
                    .inspect_err(|e| tracing::warn!(channel, error = %e, "Failed to send message to subscriber"))
                    .is_err()
            })
            .collect();
        if !failed.is_empty() {
            for f in &failed {
                self.unsubscribe(channel, f);
            }
        }
    }

    /// `pubsub:stats`.
    pub fn stats(&self) -> PubSubStats {
        let subs = self.subscribers.lock().unwrap();
        let channels: Vec<ChannelStats> = subs
            .iter()
            .map(|(c, l)| ChannelStats {
                channel: c.clone(),
                subscriber_count: l.len(),
            })
            .collect();
        PubSubStats {
            total_channels: channels.len(),
            total_subscribers: channels.iter().map(|c| c.subscriber_count).sum(),
            channels,
        }
    }

    /// `cleanup()`.
    pub fn cleanup(&self) {
        self.subscribers.lock().unwrap().clear();
    }
}
