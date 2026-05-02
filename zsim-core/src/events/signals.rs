/// 模拟引擎分发的 12 种事件类型。
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

/// 一个携带 JSON 值作为负载的类型化游戏事件。
#[derive(Debug, Clone)]
pub struct GameEvent {
    pub event_type: EventType,
    /// 事件产生时的 Tick 数。
    pub tick: u64,
    /// 来源实体 ID（角色或敌人）。
    pub source_id: Option<String>,
    /// 目标实体 ID。
    pub target_id: Option<String>,
    /// 包含事件特定数据的灵活 JSON 负载。
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
