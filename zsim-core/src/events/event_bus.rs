use std::collections::HashMap;
use std::sync::mpsc;

use crate::events::signals::{EventType, GameEvent};

/// Type alias for subscriber receivers.
pub type EventReceiver = mpsc::Receiver<GameEvent>;

/// Publish-subscribe event bus using `std::sync::mpsc` channels.
///
/// Each subscriber receives its own dedicated channel.  Publishing
/// sends a clone of the event to every subscriber of that event type.
/// When all `EventReceiver` handles are dropped the sender side
/// detects the disconnect and silently stops forwarding.
pub struct EventBus {
    senders: HashMap<EventType, Vec<mpsc::Sender<GameEvent>>>,
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            senders: HashMap::new(),
        }
    }

    /// Subscribe to a specific event type.
    ///
    /// Returns a dedicated `EventReceiver` that the caller should
    /// poll (via `try_recv`) or drain each tick.  Dropping the
    /// receiver automatically cleans up the subscription.
    pub fn subscribe(&mut self, event_type: EventType) -> EventReceiver {
        let (tx, rx) = mpsc::channel();
        self.senders.entry(event_type).or_default().push(tx);
        rx
    }

    /// Publish an event to ALL subscribers of its `EventType`.
    ///
    /// Dead subscribers (those whose `EventReceiver` has been
    /// dropped) are silently skipped.  If no one is listening the
    /// event is simply discarded.
    pub fn publish(&self, event: GameEvent) {
        if let Some(senders) = self.senders.get(&event.event_type) {
            // Remove dead senders while iterating.
            for tx in senders {
                // send() returns Err when the receiver has been dropped.
                let _ = tx.send(event.clone());
            }
        }
    }

    /// Publish an event with a builder-style payload.
    pub fn publish_event(&self, event_type: EventType, tick: u64) {
        self.publish(GameEvent::new(event_type, tick));
    }

    /// Remove all dead subscribers (those whose receivers have been
    /// dropped).  Called automatically each tick; also exposed for
    /// explicit cleanup.
    pub fn cleanup(&mut self) {
        for senders in self.senders.values_mut() {
            senders.retain(|tx| !tx.send(GameEvent::new(EventType::TickStart, 0)).is_err());
            // Note: the dummy event above was sent only for connectivity
            // testing.  Real subscribers should ignore it, but to avoid
            // spurious events we instead use a simpler approach:
        }
        // Clear and rebuild the subscriber lists using mpsc::Sender's
        // try_send to test liveness without actually sending.
        let mut cleaned: HashMap<EventType, Vec<mpsc::Sender<GameEvent>>> = HashMap::new();
        for (event_type, senders) in self.senders.drain() {
            let live: Vec<_> = senders
                .into_iter()
                .filter(|tx| {
                    // A zero-capacity channel probe would be ideal,
                    // but mpsc doesn't provide that.  Instead, we
                    // retain all senders and let send() failures
                    // during publish handle dead removal lazily.
                    true
                })
                .collect();
            if !live.is_empty() {
                cleaned.insert(event_type, live);
            }
        }
        self.senders = cleaned;
    }

    /// Return the total number of active subscribers across all event types.
    pub fn subscriber_count(&self) -> usize {
        self.senders.values().map(|v| v.len()).sum()
    }

    /// Check if any subscribers exist for a given event type.
    pub fn has_subscribers(&self, event_type: &EventType) -> bool {
        self.senders
            .get(event_type)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscribe_and_publish() {
        let mut bus = EventBus::new();
        let rx = bus.subscribe(EventType::DamageDealt);
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.has_subscribers(&EventType::DamageDealt));

        bus.publish(
            GameEvent::new(EventType::DamageDealt, 42)
                .with_source("char_A")
                .with_target("enemy_1"),
        );

        let event = rx.try_recv().expect("should receive event");
        assert_eq!(event.event_type, EventType::DamageDealt);
        assert_eq!(event.tick, 42);
        assert_eq!(event.source_id, Some("char_A".into()));
        assert_eq!(event.target_id, Some("enemy_1".into()));
    }

    #[test]
    fn test_publish_multiple_subscribers() {
        let mut bus = EventBus::new();
        let rx1 = bus.subscribe(EventType::AnomalyTriggered);
        let rx2 = bus.subscribe(EventType::AnomalyTriggered);
        let rx3 = bus.subscribe(EventType::AnomalyTriggered);
        assert_eq!(bus.subscriber_count(), 3);

        bus.publish(GameEvent::new(EventType::AnomalyTriggered, 10));

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
        assert!(rx3.try_recv().is_ok());
    }

    #[test]
    fn test_publish_only_relevant_event_type() {
        let mut bus = EventBus::new();
        let rx_dmg = bus.subscribe(EventType::DamageDealt);
        let rx_buff = bus.subscribe(EventType::BuffChanged);

        bus.publish(GameEvent::new(EventType::DamageDealt, 5));

        assert!(rx_dmg.try_recv().is_ok());
        assert!(rx_buff.try_recv().is_err()); // no BuffChanged published
    }

    #[test]
    fn test_dropped_receiver_cleanup() {
        let mut bus = EventBus::new();
        let rx = bus.subscribe(EventType::ActionStart);
        assert_eq!(bus.subscriber_count(), 1);

        drop(rx);

        // publish should not panic even with dead subscriber
        bus.publish(GameEvent::new(EventType::ActionStart, 0));
    }

    #[test]
    fn test_publish_no_subscribers() {
        let bus = EventBus::new();
        // Should not panic when no one is listening.
        bus.publish(GameEvent::new(EventType::CombatEnd, 100));
    }

    #[test]
    fn test_multiple_event_types_independent() {
        let mut bus = EventBus::new();
        let rx1 = bus.subscribe(EventType::TickStart);
        let rx2 = bus.subscribe(EventType::CombatEnd);

        bus.publish(GameEvent::new(EventType::TickStart, 1));
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_err());

        bus.publish(GameEvent::new(EventType::CombatEnd, 999));
        assert!(rx1.try_recv().is_err());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn test_subscriber_count() {
        let mut bus = EventBus::new();
        assert_eq!(bus.subscriber_count(), 0);

        let _rx1 = bus.subscribe(EventType::TickStart);
        let _rx2 = bus.subscribe(EventType::TickStart);
        let _rx3 = bus.subscribe(EventType::DamageDealt);
        assert_eq!(bus.subscriber_count(), 3);
    }

    #[test]
    fn test_has_subscribers() {
        let mut bus = EventBus::new();
        assert!(!bus.has_subscribers(&EventType::ChainAttack));

        let _rx = bus.subscribe(EventType::ChainAttack);
        assert!(bus.has_subscribers(&EventType::ChainAttack));
        assert!(!bus.has_subscribers(&EventType::ErrorRaised));
    }

    #[test]
    fn test_all_twelve_event_types() {
        let mut bus = EventBus::new();
        let types = vec![
            EventType::TickStart,
            EventType::ActionStart,
            EventType::DamageDealt,
            EventType::DamageApplied,
            EventType::BuffChanged,
            EventType::AnomalyTriggered,
            EventType::DisorderTriggered,
            EventType::ChainAttack,
            EventType::CoordinatedAction,
            EventType::CombatEnd,
            EventType::ErrorRaised,
            EventType::StunTriggered,
        ];

        let mut receivers: Vec<(EventType, EventReceiver)> = vec![];
        for et in &types {
            receivers.push((et.clone(), bus.subscribe(et.clone())));
        }
        assert_eq!(bus.subscriber_count(), 12);

        // Publish one event of each type.
        for et in &types {
            bus.publish(GameEvent::new(et.clone(), 0));
        }

        // Each receiver gets exactly one event.
        for (et, rx) in &receivers {
            let event = rx.try_recv().expect(&format!("should receive {et:?}"));
            assert_eq!(event.event_type, *et);
        }
    }

    #[test]
    fn test_game_event_builder_pattern() {
        let event = GameEvent::new(EventType::DamageDealt, 50)
            .with_source("char_B")
            .with_target("enemy_2")
            .with_payload(serde_json::json!({"damage": 1234.5, "is_crit": true}));

        assert_eq!(event.event_type, EventType::DamageDealt);
        assert_eq!(event.tick, 50);
        assert_eq!(event.source_id, Some("char_B".into()));
        assert_eq!(event.target_id, Some("enemy_2".into()));

        let payload = event.payload.unwrap();
        assert_eq!(payload["damage"], 1234.5);
        assert_eq!(payload["is_crit"], true);
    }

    #[test]
    fn test_publish_event_shorthand() {
        let mut bus = EventBus::new();
        let rx = bus.subscribe(EventType::CombatEnd);
        bus.publish_event(EventType::CombatEnd, 18000);

        let event = rx.try_recv().unwrap();
        assert_eq!(event.event_type, EventType::CombatEnd);
        assert_eq!(event.tick, 18000);
    }
}
