use serde::{Deserialize, Serialize};

/// A single action entry in an APL track, specifying when to execute which action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEntry {
    pub action_id: String,
    /// Tick at which this action should start executing.
    pub at: u64,
}

/// A single track (one character's APL sequence) in an APL plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub track_id: String,
    /// The character ID this track is assigned to.
    pub char_id: String,
    pub actions: Vec<ActionEntry>,
}

/// Full APL (Action Priority List) data loaded from data/apl/.
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
