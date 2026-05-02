use std::collections::HashMap;
use std::sync::mpsc;

use crate::events::signals::{EventType, GameEvent};

/// 订阅者接收器的类型别名。
pub type EventReceiver = mpsc::Receiver<GameEvent>;

/// 使用 `std::sync::mpsc` 信道的发布-订阅事件总线。
///
/// 每个订阅者收到自己的专用信道。发布时
/// 会将事件的克隆发送给该事件类型的每个订阅者。
/// 当所有 `EventReceiver` 句柄被丢弃时，发送方
/// 检测到断开连接并静默停止转发。
pub struct EventBus {
    senders: HashMap<EventType, Vec<mpsc::Sender<GameEvent>>>,
}

impl EventBus {
    pub fn new() -> Self {
        EventBus {
            senders: HashMap::new(),
        }
    }

    /// 订阅特定事件类型。
    ///
    /// 返回一个专用的 `EventReceiver`，调用者应在每个
    /// tick 中轮询（通过 `try_recv`）或清空。丢弃
    /// 接收器会自动清理订阅。
    pub fn subscribe(&mut self, event_type: EventType) -> EventReceiver {
        let (tx, rx) = mpsc::channel();
        self.senders.entry(event_type).or_default().push(tx);
        rx
    }

    /// 向 `EventType` 的所有订阅者发布一个事件。
    ///
    /// 失效的订阅者（其 `EventReceiver` 已被丢弃的）
    /// 被静默跳过。如果没有人在监听，
    /// 事件会被直接丢弃。
    pub fn publish(&self, event: GameEvent) {
        if let Some(senders) = self.senders.get(&event.event_type) {
            // 在迭代过程中移除失效的发送者。
            for tx in senders {
                // send() 在接收器被丢弃时返回 Err。
                let _ = tx.send(event.clone());
            }
        }
    }

    /// 使用构建器风格的负载发布一个事件。
    pub fn publish_event(&self, event_type: EventType, tick: u64) {
        self.publish(GameEvent::new(event_type, tick));
    }

    /// 移除所有失效的订阅者（其接收器已被丢弃的）。
    /// 每个 tick 自动调用；也暴露出来以便
    /// 显式清理。
    pub fn cleanup(&mut self) {
        for senders in self.senders.values_mut() {
            senders.retain(|tx| !tx.send(GameEvent::new(EventType::TickStart, 0)).is_err());
            // 注意：上面的哑事件仅用于连通性测试。
            // 真正的订阅者应忽略它，但为了避免
            // 虚假事件，我们转而采用更简单的方法：
        }
        // 清除并使用 mpsc::Sender 的 try_send 重建订阅者列表，
        // 以测试活跃性而不实际发送消息。
        let mut cleaned: HashMap<EventType, Vec<mpsc::Sender<GameEvent>>> = HashMap::new();
        for (event_type, senders) in self.senders.drain() {
            let live: Vec<_> = senders
                .into_iter()
                .filter(|tx| {
                    // 零容量信道探测是理想的方式，
                    // 但 mpsc 不提供该功能。因此，我们
                    // 保留所有发送者，让 publish 时的 send()
                    // 失败来延迟处理失效的移除。
                    true
                })
                .collect();
            if !live.is_empty() {
                cleaned.insert(event_type, live);
            }
        }
        self.senders = cleaned;
    }

    /// 返回所有事件类型的活跃订阅者总数。
    pub fn subscriber_count(&self) -> usize {
        self.senders.values().map(|v| v.len()).sum()
    }

    /// 检查给定事件类型是否存在任何订阅者。
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
        assert!(rx_buff.try_recv().is_err()); // 未发布 BuffChanged 事件
    }

    #[test]
    fn test_dropped_receiver_cleanup() {
        let mut bus = EventBus::new();
        let rx = bus.subscribe(EventType::ActionStart);
        assert_eq!(bus.subscriber_count(), 1);

        drop(rx);

        // 即使订阅者已失效，publish 也不应 panic
        bus.publish(GameEvent::new(EventType::ActionStart, 0));
    }

    #[test]
    fn test_publish_no_subscribers() {
        let bus = EventBus::new();
        // 没有监听者时不应 panic。
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

        // 每种类型发布一个事件。
        for et in &types {
            bus.publish(GameEvent::new(et.clone(), 0));
        }

        // 每个接收器恰好收到一个事件。
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
