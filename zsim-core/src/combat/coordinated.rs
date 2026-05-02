//! Coordinated action system — manages off-field character reactive attacks.
//!
//! Listens for combat events (damage dealt, anomaly triggered, chain attacks,
//! dodges, parries) and produces derived [`SkillAction`]s for off-field characters
//! to execute their coordinated attacks.
//!
//! Each listener has an independent cooldown to prevent excessive triggering.
//! Derived actions execute in the same tick as their parent action.

use std::fmt::Debug;

use crate::combat::apl::SkillAction;
use crate::combat::game_state::GameState;
use crate::events::signals::{EventType, GameEvent};

/// Trait for coordinated action listeners.
///
/// Implementations define:
/// - Which event types trigger this listener ([`event_types()`])
/// - What action(s) to produce when triggered ([`on_event()`])
pub trait CoordinatedListener: Debug {
    /// Called when a relevant event occurs. Returns derived skill actions.
    fn on_event(&self, event: &GameEvent, game_state: &GameState) -> Vec<SkillAction>;

    /// Returns the event types this listener responds to.
    fn event_types(&self) -> Vec<EventType>;
}

/// A default coordinated listener that triggers a fixed action on a specific event type.
#[derive(Debug)]
pub struct SimpleCoordinatedListener {
    pub source_id: String,
    pub action_id: String,
    pub trigger_event: EventType,
}

impl CoordinatedListener for SimpleCoordinatedListener {
    fn on_event(&self, event: &GameEvent, game_state: &GameState) -> Vec<SkillAction> {
        let target_id = event
            .target_id
            .clone()
            .unwrap_or_else(|| "enemy_default".to_string());
        vec![SkillAction {
            action_id: self.action_id.clone(),
            source_id: self.source_id.clone(),
            target_id,
            started_at_tick: game_state.current_tick,
        }]
    }

    fn event_types(&self) -> Vec<EventType> {
        vec![self.trigger_event.clone()]
    }
}

/// Internal slot holding a listener with cooldown metadata.
#[derive(Debug)]
struct ListenerSlot {
    listener: Box<dyn CoordinatedListener>,
    event_types: Vec<EventType>,
    cooldown_ticks: u64,
    last_trigger_tick: Option<u64>,
}

/// Manages coordinated action listeners and produces derived skill actions.
///
/// ## Event flow
/// 1. Simulation runner calls [`on_event()`](CoordinatedActionSystem::on_event) or
///    [`process_events()`](CoordinatedActionSystem::process_events) with the events
///    generated during the current tick.
/// 2. Each listener checks whether the event type matches and its cooldown has expired.
/// 3. Matching listeners produce [`SkillAction`]s queued in an internal buffer.
/// 4. Runner drains the buffer via
///    [`get_pending_actions()`](CoordinatedActionSystem::get_pending_actions)
///    (consuming queue).
///
/// Derived actions execute in the **same tick** as their parent action.
#[derive(Debug)]
pub struct CoordinatedActionSystem {
    slots: Vec<ListenerSlot>,
    pending_actions: Vec<SkillAction>,
}

impl CoordinatedActionSystem {
    /// Create an empty coordinated action system with no listeners.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            pending_actions: Vec::new(),
        }
    }

    /// Register a coordinated listener.
    ///
    /// - `listener`: the trait object that produces actions
    /// - `event_types`: which event types this listener reacts to
    /// - `cooldown_ticks`: minimum ticks between successive triggers (0 = no cooldown)
    pub fn register(
        &mut self,
        listener: Box<dyn CoordinatedListener>,
        event_types: Vec<EventType>,
        cooldown_ticks: u64,
    ) {
        self.slots.push(ListenerSlot {
            listener,
            event_types,
            cooldown_ticks,
            last_trigger_tick: None,
        });
    }

    /// Process a single event through all registered listeners.
    ///
    /// Matching listeners whose cooldown has expired will produce skill actions
    /// added to the internal queue.  If a listener returns an empty vec its
    /// cooldown is **not** updated, allowing conditional triggers to retry on
    /// a subsequent event.
    pub fn on_event(&mut self, event: &GameEvent, game_state: &GameState) {
        for slot in &mut self.slots {
            if !slot.event_types.contains(&event.event_type) {
                continue;
            }
            // Check cooldown — first trigger is always allowed (None)
            if let Some(last) = slot.last_trigger_tick {
                if game_state.current_tick < last + slot.cooldown_ticks {
                    continue;
                }
            }
            // Cooldown satisfied — invoke listener
            let actions = slot.listener.on_event(event, game_state);
            if !actions.is_empty() {
                slot.last_trigger_tick = Some(game_state.current_tick);
                self.pending_actions.extend(actions);
            }
        }
    }

    /// Process multiple events at once.
    ///
    /// Convenience method that calls [`on_event`](Self::on_event) for each event.
    pub fn process_events(&mut self, events: &[GameEvent], game_state: &GameState) {
        for event in events {
            self.on_event(event, game_state);
        }
    }

    /// Drain all pending skill actions (consuming queue pattern).
    ///
    /// Returns an empty vec if no actions are pending.
    pub fn get_pending_actions(&mut self) -> Vec<SkillAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Return the number of registered listeners.
    pub fn listener_count(&self) -> usize {
        self.slots.len()
    }

    /// Return the number of pending actions (before draining).
    pub fn pending_count(&self) -> usize {
        self.pending_actions.len()
    }
}

impl Default for CoordinatedActionSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::team::TeamManager;
    use crate::entities::character::Character;
    use crate::entities::enums::{ElementTag, FactionTag, SimMode, SpecialtyTag};
    use crate::entities::models::BaseStats;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn make_character(char_id: &str) -> Character {
        let mut c = Character::new(
            char_id,
            FactionTag::GentleHouse,
            SpecialtyTag::Support,
            ElementTag::Ether,
            BaseStats::default().with_hp(8000.0),
        );
        c.resources.energy = 100.0;
        c
    }

    fn make_game_state() -> GameState {
        GameState::new(
            TeamManager::new(vec![
                make_character("char_on_field"),
                make_character("char_off_field"),
            ]),
            SimMode::Single,
        )
    }

    fn coord_listener(
        source_id: &str,
        action_id: &str,
        trigger: EventType,
    ) -> Box<dyn CoordinatedListener> {
        Box::new(SimpleCoordinatedListener {
            source_id: source_id.into(),
            action_id: action_id.into(),
            trigger_event: trigger,
        })
    }

    fn damage_event(tick: u64, target: &str) -> GameEvent {
        GameEvent::new(EventType::DamageDealt, tick)
            .with_source("char_on_field")
            .with_target(target)
    }

    fn anomaly_event(tick: u64) -> GameEvent {
        GameEvent::new(EventType::AnomalyTriggered, tick)
            .with_source("char_on_field")
            .with_target("enemy_1")
    }

    fn chain_event(tick: u64) -> GameEvent {
        GameEvent::new(EventType::ChainAttack, tick)
            .with_source("char_on_field")
            .with_target("enemy_1")
    }

    // ------------------------------------------------------------------
    // Creation & queries
    // ------------------------------------------------------------------

    #[test]
    fn test_new_empty() {
        let mut system = CoordinatedActionSystem::new();
        assert_eq!(system.listener_count(), 0);
        assert_eq!(system.pending_count(), 0);
        assert!(system.get_pending_actions().is_empty());
    }

    #[test]
    fn test_listener_count() {
        let mut system = CoordinatedActionSystem::new();
        assert_eq!(system.listener_count(), 0);

        system.register(
            coord_listener("char_1", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            30,
        );
        assert_eq!(system.listener_count(), 1);

        system.register(
            coord_listener("char_2", "coord_atk", EventType::AnomalyTriggered),
            vec![EventType::AnomalyTriggered],
            60,
        );
        assert_eq!(system.listener_count(), 2);
    }

    #[test]
    fn test_pending_count() {
        let mut system = CoordinatedActionSystem::new();
        assert_eq!(system.pending_count(), 0);

        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 1);
    }

    #[test]
    fn test_default_impl() {
        let mut system = CoordinatedActionSystem::default();
        assert_eq!(system.listener_count(), 0);
        assert_eq!(system.pending_count(), 0);
        assert!(system.get_pending_actions().is_empty());
    }

    // ------------------------------------------------------------------
    // Register & fire
    // ------------------------------------------------------------------

    #[test]
    fn test_matching_event_triggers_action() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "coord_atk");
        assert_eq!(actions[0].source_id, "char_off");
    }

    #[test]
    fn test_nonmatching_event_no_action() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        let event = GameEvent::new(EventType::BuffChanged, 0);
        system.on_event(&event, &state);

        assert_eq!(system.pending_count(), 0);
    }

    #[test]
    fn test_simple_listener_action_fields() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_support", "support_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let mut state = make_game_state();
        state.current_tick = 42;
        let event = damage_event(42, "enemy_boss");
        system.on_event(&event, &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "support_atk");
        assert_eq!(actions[0].source_id, "char_support");
        assert_eq!(actions[0].target_id, "enemy_boss");
        assert_eq!(actions[0].started_at_tick, 42);
    }

    #[test]
    fn test_multiple_listeners_same_event() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_a", "atk_a", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );
        system.register(
            coord_listener("char_b", "atk_b", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 2);

        let source_ids: Vec<&str> = actions.iter().map(|a| a.source_id.as_str()).collect();
        assert!(source_ids.contains(&"char_a"));
        assert!(source_ids.contains(&"char_b"));
    }

    #[test]
    fn test_multiple_events_same_tick() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let mut state = make_game_state();
        state.current_tick = 0;

        let e1 = damage_event(0, "enemy_1");
        let e2 = damage_event(0, "enemy_2");
        system.on_event(&e1, &state);
        system.on_event(&e2, &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn test_process_events_batch() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        let events = vec![
            damage_event(0, "enemy_1"),
            GameEvent::new(EventType::BuffChanged, 0),
            damage_event(0, "enemy_2"),
        ];
        system.process_events(&events, &state);

        // Only the damage events should trigger (2 events, but same-tick cooldown
        // with cooldown=0 means both fire)
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn test_listener_returns_empty_skips_cooldown_update() {
        // A listener that conditionally returns empty actions
        #[derive(Debug)]
        struct ConditionalListener {
            only_on_tick: u64,
        }

        impl CoordinatedListener for ConditionalListener {
            fn on_event(&self, _event: &GameEvent, state: &GameState) -> Vec<SkillAction> {
                if state.current_tick == self.only_on_tick {
                    vec![SkillAction {
                        action_id: "special".into(),
                        source_id: "char_cond".into(),
                        target_id: "enemy_1".into(),
                        started_at_tick: state.current_tick,
                    }]
                } else {
                    vec![]
                }
            }

            fn event_types(&self) -> Vec<EventType> {
                vec![EventType::DamageDealt]
            }
        }

        let mut system = CoordinatedActionSystem::new();
        system.register(
            Box::new(ConditionalListener { only_on_tick: 10 }),
            vec![EventType::DamageDealt],
            30,
        );

        let mut state = make_game_state();

        // Tick 5 → returns empty → cooldown NOT updated
        state.current_tick = 5;
        let event = damage_event(5, "enemy_1");
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 0);

        // Tick 6 → still not triggered, cooldown was NOT updated at tick 5
        state.current_tick = 6;
        let event = damage_event(6, "enemy_1");
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 0);

        // Tick 10 → should trigger
        state.current_tick = 10;
        let event = damage_event(10, "enemy_1");
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 1);
    }

    #[test]
    fn test_listener_returns_multiple_actions() {
        #[derive(Debug)]
        struct DoubleHitListener;

        impl CoordinatedListener for DoubleHitListener {
            fn on_event(&self, _event: &GameEvent, state: &GameState) -> Vec<SkillAction> {
                vec![
                    SkillAction {
                        action_id: "coord_hit_1".into(),
                        source_id: "char_double".into(),
                        target_id: "enemy_1".into(),
                        started_at_tick: state.current_tick,
                    },
                    SkillAction {
                        action_id: "coord_hit_2".into(),
                        source_id: "char_double".into(),
                        target_id: "enemy_1".into(),
                        started_at_tick: state.current_tick,
                    },
                ]
            }

            fn event_types(&self) -> Vec<EventType> {
                vec![EventType::DamageDealt]
            }
        }

        let mut system = CoordinatedActionSystem::new();
        system.register(
            Box::new(DoubleHitListener),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].action_id, "coord_hit_1");
        assert_eq!(actions[1].action_id, "coord_hit_2");
    }

    // ------------------------------------------------------------------
    // Cooldown
    // ------------------------------------------------------------------

    #[test]
    fn test_cooldown_blocks_immediate_retrigger() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            30,
        );

        let mut state = make_game_state();
        state.current_tick = 0;

        // First trigger at tick 0
        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 1);
        let _ = system.get_pending_actions();

        // Same tick: blocked by cooldown (0 < 0 + 30)
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 0);
    }

    #[test]
    fn test_cooldown_expires_allows_retrigger() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            30,
        );

        let mut state = make_game_state();
        state.current_tick = 0;

        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);
        let _ = system.get_pending_actions();

        // Tick 30: cooldown expired (30 < 0 + 30 → false → not blocked)
        state.current_tick = 30;
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 1);
    }

    #[test]
    fn test_cooldown_still_active_at_tick_29() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            30,
        );

        let mut state = make_game_state();
        state.current_tick = 0;

        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);
        let _ = system.get_pending_actions();

        // Tick 29: still blocked (29 < 0 + 30)
        state.current_tick = 29;
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 0);
    }

    #[test]
    fn test_zero_cooldown_fires_every_time() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let mut state = make_game_state();
        state.current_tick = 0;

        // Fire many times in a row
        for _ in 0..5 {
            let event = damage_event(state.current_tick, "enemy_1");
            system.on_event(&event, &state);
        }

        assert_eq!(system.pending_count(), 5);
    }

    #[test]
    fn test_cooldown_per_listener_independence() {
        let mut system = CoordinatedActionSystem::new();
        // Listener A: 10-tick cooldown
        system.register(
            coord_listener("char_a", "atk_a", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            10,
        );
        // Listener B: 50-tick cooldown
        system.register(
            coord_listener("char_b", "atk_b", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            50,
        );

        let mut state = make_game_state();
        state.current_tick = 0;

        let event = damage_event(0, "enemy");
        system.on_event(&event, &state);
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 2); // Both fire at tick 0

        // Tick 15: A's cooldown expired (15 ≥ 10), B's still active (15 < 50)
        state.current_tick = 15;
        system.on_event(&event, &state);
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].source_id, "char_a");

        // Tick 50: B's cooldown expired
        state.current_tick = 50;
        let _ = system.get_pending_actions(); // drain nothing
        system.on_event(&event, &state);
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 2);
    }

    #[test]
    fn test_cooldown_not_affected_by_non_matching_events() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            30,
        );

        let mut state = make_game_state();
        state.current_tick = 0;

        // Trigger at tick 0
        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);
        let _ = system.get_pending_actions();

        // Non-matching event at tick 5 → should NOT advance cooldown timeline
        state.current_tick = 5;
        let bad_event = GameEvent::new(EventType::BuffChanged, 5);
        system.on_event(&bad_event, &state);
        assert_eq!(system.pending_count(), 0);

        // Tick 29: still blocked (cooldown from tick 0)
        state.current_tick = 29;
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 0);

        // Tick 30: cooldown expired (30 ≥ 30)
        state.current_tick = 30;
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 1);
    }

    #[test]
    fn test_on_event_updates_last_trigger_tick() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            20,
        );

        let mut state = make_game_state();

        // Trigger at tick 5
        state.current_tick = 5;
        let event = damage_event(5, "enemy_1");
        system.on_event(&event, &state);
        let _ = system.get_pending_actions();

        // Tick 24: blocked (24 < 5 + 20)
        state.current_tick = 24;
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 0);

        // Tick 25: now allowed (25 >= 5 + 20)
        state.current_tick = 25;
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 1);
    }

    // ------------------------------------------------------------------
    // Consuming queue
    // ------------------------------------------------------------------

    #[test]
    fn test_get_pending_actions_consuming() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 1);

        let first = system.get_pending_actions();
        assert_eq!(first.len(), 1);
        assert_eq!(system.pending_count(), 0);

        // Second drain is empty
        let second = system.get_pending_actions();
        assert!(second.is_empty());
    }

    #[test]
    fn test_get_pending_actions_empty() {
        let mut system = CoordinatedActionSystem::new();
        let actions = system.get_pending_actions();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_pending_count_increments_and_drains() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        assert_eq!(system.pending_count(), 0);

        let state = make_game_state();
        system.on_event(&damage_event(0, "enemy_1"), &state);
        assert_eq!(system.pending_count(), 1);

        system.on_event(&damage_event(0, "enemy_2"), &state);
        assert_eq!(system.pending_count(), 2);

        let _ = system.get_pending_actions();
        assert_eq!(system.pending_count(), 0);
    }

    // ------------------------------------------------------------------
    // Event type filtering
    // ------------------------------------------------------------------

    #[test]
    fn test_trigger_on_damage_dealt() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        system.on_event(&damage_event(0, "enemy_1"), &state);
        assert_eq!(system.pending_count(), 1);
    }

    #[test]
    fn test_trigger_on_anomaly_triggered() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "anomaly_followup", EventType::AnomalyTriggered),
            vec![EventType::AnomalyTriggered],
            0,
        );

        let state = make_game_state();
        system.on_event(&anomaly_event(0), &state);
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "anomaly_followup");
    }

    #[test]
    fn test_trigger_on_chain_attack() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "chain_followup", EventType::ChainAttack),
            vec![EventType::ChainAttack],
            0,
        );

        let state = make_game_state();
        system.on_event(&chain_event(0), &state);
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "chain_followup");
    }

    #[test]
    fn test_multiple_event_types_per_listener() {
        #[derive(Debug)]
        struct MultiEventListener;

        impl CoordinatedListener for MultiEventListener {
            fn on_event(&self, event: &GameEvent, state: &GameState) -> Vec<SkillAction> {
                let action_id = match event.event_type {
                    EventType::DamageDealt => "on_damage",
                    EventType::AnomalyTriggered => "on_anomaly",
                    EventType::ChainAttack => "on_chain",
                    _ => "other",
                };
                vec![SkillAction {
                    action_id: action_id.into(),
                    source_id: "char_multi".into(),
                    target_id: "enemy_1".into(),
                    started_at_tick: state.current_tick,
                }]
            }

            fn event_types(&self) -> Vec<EventType> {
                vec![
                    EventType::DamageDealt,
                    EventType::AnomalyTriggered,
                    EventType::ChainAttack,
                ]
            }
        }

        let mut system = CoordinatedActionSystem::new();
        system.register(
            Box::new(MultiEventListener),
            vec![
                EventType::DamageDealt,
                EventType::AnomalyTriggered,
                EventType::ChainAttack,
            ],
            0,
        );

        let state = make_game_state();

        // Fire all three event types
        system.on_event(&damage_event(0, "enemy_1"), &state);
        system.on_event(&anomaly_event(0), &state);
        system.on_event(&chain_event(0), &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].action_id, "on_damage");
        assert_eq!(actions[1].action_id, "on_anomaly");
        assert_eq!(actions[2].action_id, "on_chain");
    }

    // ------------------------------------------------------------------
    // Custom listener
    // ------------------------------------------------------------------

    #[test]
    fn test_custom_listener_implementation() {
        #[derive(Debug)]
        struct CustomListener;

        impl CoordinatedListener for CustomListener {
            fn on_event(&self, _event: &GameEvent, _state: &GameState) -> Vec<SkillAction> {
                vec![SkillAction {
                    action_id: "custom_action".into(),
                    source_id: "custom_char".into(),
                    target_id: "custom_target".into(),
                    started_at_tick: 999,
                }]
            }

            fn event_types(&self) -> Vec<EventType> {
                vec![EventType::ActionStart]
            }
        }

        let mut system = CoordinatedActionSystem::new();
        system.register(
            Box::new(CustomListener),
            vec![EventType::ActionStart],
            0,
        );

        let state = make_game_state();
        let event = GameEvent::new(EventType::ActionStart, 5);
        system.on_event(&event, &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "custom_action");
        assert_eq!(actions[0].source_id, "custom_char");
        assert_eq!(actions[0].target_id, "custom_target");
        assert_eq!(actions[0].started_at_tick, 999);
    }

    #[test]
    fn test_custom_listener_uses_event_payload() {
        #[derive(Debug)]
        struct PayloadAwareListener;

        impl CoordinatedListener for PayloadAwareListener {
            fn on_event(&self, event: &GameEvent, state: &GameState) -> Vec<SkillAction> {
                let is_crit = event
                    .payload
                    .as_ref()
                    .and_then(|p| p.get("is_crit"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if is_crit {
                    vec![SkillAction {
                        action_id: "crit_followup".into(),
                        source_id: "char_payload".into(),
                        target_id: event
                            .target_id
                            .clone()
                            .unwrap_or_else(|| "enemy".into()),
                        started_at_tick: state.current_tick,
                    }]
                } else {
                    vec![]
                }
            }

            fn event_types(&self) -> Vec<EventType> {
                vec![EventType::DamageDealt]
            }
        }

        let mut system = CoordinatedActionSystem::new();
        system.register(
            Box::new(PayloadAwareListener),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();

        // Non-crit event → no action
        let non_crit = GameEvent::new(EventType::DamageDealt, 0)
            .with_source("char")
            .with_target("enemy_1")
            .with_payload(serde_json::json!({"damage": 100.0, "is_crit": false}));
        system.on_event(&non_crit, &state);
        assert_eq!(system.pending_count(), 0);

        // Crit event → action
        let crit = GameEvent::new(EventType::DamageDealt, 0)
            .with_source("char")
            .with_target("enemy_boss")
            .with_payload(serde_json::json!({"damage": 300.0, "is_crit": true}));
        system.on_event(&crit, &state);
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "crit_followup");
        assert_eq!(actions[0].target_id, "enemy_boss");
    }

    // ------------------------------------------------------------------
    // Edge cases & integration
    // ------------------------------------------------------------------

    #[test]
    fn test_simple_listener_uses_event_target() {
        let mut system = CoordinatedActionSystem::new();
        let listener = SimpleCoordinatedListener {
            source_id: "char_off".into(),
            action_id: "coord_atk".into(),
            trigger_event: EventType::DamageDealt,
        };
        system.register(Box::new(listener), vec![EventType::DamageDealt], 0);

        let state = make_game_state();
        let event = damage_event(0, "enemy_specific");
        system.on_event(&event, &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].target_id, "enemy_specific");
    }

    #[test]
    fn test_simple_listener_fallback_target() {
        let mut system = CoordinatedActionSystem::new();
        let listener = SimpleCoordinatedListener {
            source_id: "char_off".into(),
            action_id: "coord_atk".into(),
            trigger_event: EventType::DamageDealt,
        };
        system.register(Box::new(listener), vec![EventType::DamageDealt], 0);

        let state = make_game_state();
        // Event with no target_id
        let event = GameEvent::new(EventType::DamageDealt, 0).with_source("char");
        system.on_event(&event, &state);

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].target_id, "enemy_default");
    }

    #[test]
    fn test_action_started_at_tick_correct() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let mut state = make_game_state();

        for tick in &[10u64, 20, 30] {
            state.current_tick = *tick;
            let event = damage_event(*tick, "enemy_1");
            system.on_event(&event, &state);
        }

        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 3);
        assert_eq!(actions[0].started_at_tick, 10);
        assert_eq!(actions[1].started_at_tick, 20);
        assert_eq!(actions[2].started_at_tick, 30);
    }

    #[test]
    fn test_no_action_on_registered_but_wrong_event_type() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_off", "atk", EventType::ChainAttack),
            vec![EventType::ChainAttack],
            0,
        );

        let state = make_game_state();
        // Wrong event type
        system.on_event(&damage_event(0, "enemy_1"), &state);
        system.on_event(&anomaly_event(0), &state);
        system.on_event(
            &GameEvent::new(EventType::BuffChanged, 0),
            &state,
        );

        assert_eq!(system.pending_count(), 0);
    }

    #[test]
    fn test_register_multiple_then_fire_in_sequence() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_1", "dmg_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            10,
        );
        system.register(
            coord_listener("char_2", "anomaly_atk", EventType::AnomalyTriggered),
            vec![EventType::AnomalyTriggered],
            20,
        );

        let mut state = make_game_state();

        // Tick 0: fire both event types
        state.current_tick = 0;
        system.on_event(&damage_event(0, "e1"), &state);
        system.on_event(&anomaly_event(0), &state);
        assert_eq!(system.pending_count(), 2);
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 2);

        // Tick 5: damage cooldown still active (5 < 10), anomaly cooldown active (5 < 20)
        state.current_tick = 5;
        system.on_event(&damage_event(5, "e1"), &state);
        system.on_event(&anomaly_event(5), &state);
        assert_eq!(system.pending_count(), 0);

        // Tick 15: damage cooldown expired (15 ≥ 10), anomaly still active (15 < 20)
        state.current_tick = 15;
        system.on_event(&damage_event(15, "e1"), &state);
        assert_eq!(system.pending_count(), 1);
        let actions = system.get_pending_actions();
        assert_eq!(actions[0].action_id, "dmg_atk");

        // Tick 20: anomaly cooldown expired
        state.current_tick = 20;
        system.on_event(&anomaly_event(20), &state);
        let actions = system.get_pending_actions();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_id, "anomaly_atk");
    }

    #[test]
    fn test_pending_actions_includes_all_sources() {
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_a", "atk_a", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );
        system.register(
            coord_listener("char_b", "atk_b", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            0,
        );

        let state = make_game_state();
        system.on_event(&damage_event(0, "enemy_X"), &state);

        let actions = system.get_pending_actions();
        let ids: Vec<&str> = actions.iter().map(|a| a.action_id.as_str()).collect();
        assert_eq!(actions.len(), 2);
        assert!(ids.contains(&"atk_a"));
        assert!(ids.contains(&"atk_b"));
    }

    #[test]
    fn test_full_lifecycle() {
        // Full lifecycle: register → fire → drain → fire → drain → verify
        let mut system = CoordinatedActionSystem::new();
        system.register(
            coord_listener("char_support", "coord_atk", EventType::DamageDealt),
            vec![EventType::DamageDealt],
            30,
        );

        let mut state = make_game_state();
        state.current_tick = 0;

        // Phase 1: first trigger at tick 0
        let event = damage_event(0, "enemy_1");
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 1);
        let phase1 = system.get_pending_actions();
        assert_eq!(phase1.len(), 1);
        assert_eq!(phase1[0].action_id, "coord_atk");
        assert_eq!(phase1[0].source_id, "char_support");

        // Phase 2: cooldown active at tick 15
        state.current_tick = 15;
        let event = damage_event(15, "enemy_1");
        system.on_event(&event, &state);
        assert_eq!(system.pending_count(), 0);

        // Phase 3: cooldown expired at tick 30
        state.current_tick = 30;
        let event = damage_event(30, "enemy_2");
        system.on_event(&event, &state);
        let phase3 = system.get_pending_actions();
        assert_eq!(phase3.len(), 1);
        assert_eq!(phase3[0].target_id, "enemy_2");
        assert_eq!(phase3[0].started_at_tick, 30);

        // Phase 4: listener still registered, verify count
        assert_eq!(system.listener_count(), 1);
        assert_eq!(system.pending_count(), 0);
    }
}
