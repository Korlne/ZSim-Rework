use serde::{Deserialize, Serialize};

/// APL 轨道中的单个动作条目，指定在何时执行哪个动作。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEntry {
    pub action_id: String,
    /// 该动作应开始执行的 Tick 数。
    pub at: u64,
}

/// APL 计划中的单个轨道（一个角色的 APL 序列）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub track_id: String,
    /// 此轨道分配到的角色 ID。
    pub char_id: String,
    pub actions: Vec<ActionEntry>,
}

/// 从 data/apl/ 加载的完整 APL（动作优先级列表）数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct APLData {
    pub tracks: Vec<Track>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_action_entry() {
        let json = r#"{"action_id": "Attack_Normal_1", "at": 0}"#;
        let entry: ActionEntry = serde_json::from_str(json).expect("deserialize");
        assert_eq!(entry.action_id, "Attack_Normal_1");
        assert_eq!(entry.at, 0);
    }

    #[test]
    fn test_deserialize_track() {
        let json = r#"{
            "track_id": "track_01",
            "char_id": "anby_demara",
            "actions": [
                {"action_id": "Attack_Normal_1", "at": 0},
                {"action_id": "Attack_Normal_2", "at": 30},
                {"action_id": "Skill_Ex_1", "at": 60}
            ]
        }"#;
        let track: Track = serde_json::from_str(json).expect("deserialize");
        assert_eq!(track.track_id, "track_01");
        assert_eq!(track.char_id, "anby_demara");
        assert_eq!(track.actions.len(), 3);
        assert_eq!(track.actions[1].action_id, "Attack_Normal_2");
        assert_eq!(track.actions[1].at, 30);
    }

    #[test]
    fn test_deserialize_apl_data() {
        let json = r#"{
            "tracks": [
                {
                    "track_id": "track_01",
                    "char_id": "anby_demara",
                    "actions": [
                        {"action_id": "Attack_Normal_1", "at": 0},
                        {"action_id": "Ultimate_1", "at": 120}
                    ]
                }
            ]
        }"#;
        let apl: APLData = serde_json::from_str(json).expect("deserialize");
        assert_eq!(apl.tracks.len(), 1);
        assert_eq!(apl.tracks[0].track_id, "track_01");
        assert_eq!(apl.tracks[0].actions.len(), 2);
    }

    #[test]
    fn test_serde_roundtrip() {
        let apl = APLData {
            tracks: vec![Track {
                track_id: "t1".into(),
                char_id: "char_a".into(),
                actions: vec![
                    ActionEntry {
                        action_id: "A1".into(),
                        at: 0,
                    },
                    ActionEntry {
                        action_id: "A2".into(),
                        at: 50,
                    },
                ],
            }],
        };
        let json = serde_json::to_string(&apl).expect("serialize");
        let back: APLData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.tracks.len(), 1);
        assert_eq!(back.tracks[0].actions.len(), 2);
    }
}
