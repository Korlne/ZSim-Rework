/// The 12 event types dispatched by the simulation engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    TickStart,
    ActionStart,
    DamageDealt,
    DamageApplied,
    BuffChanged,
    AnomalyTriggered,
    DisorderTriggered,
    ChainAttack,
    CoordinatedAction,
    CombatEnd,
    ErrorRaised,
    StunTriggered,
}

/// A typed game event with payload carried as a JSON value.
#[derive(Debug, Clone)]
pub struct GameEvent {
    pub event_type: EventType,
    /// Tick at which the event was generated.
    pub tick: u64,
    /// Source entity ID (character or enemy).
    pub source_id: Option<String>,
    /// Target entity ID.
    pub target_id: Option<String>,
    /// Flexible JSON payload with event-specific data.
    pub payload: Option<serde_json::Value>,
}

impl GameEvent {
    pub fn new(event_type: EventType, tick: u64) -> Self {
        GameEvent {
            event_type,
            tick,
            source_id: None,
            target_id: None,
            payload: None,
        }
    }

    pub fn with_source(mut self, id: impl Into<String>) -> Self {
        self.source_id = Some(id.into());
        self
    }

    pub fn with_target(mut self, id: impl Into<String>) -> Self {
        self.target_id = Some(id.into());
        self
    }

    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }
}
