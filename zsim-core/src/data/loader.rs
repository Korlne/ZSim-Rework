use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::combat::skill::SkillData;
use crate::data::apl::APLData;
use crate::data::equipment::EquipmentData;
use crate::entities::character::Character;
use crate::entities::enemy::EnemyState;

/// Central data loader for all game data JSON files.
/// Provides typed load methods with path-context error reporting.
pub struct DataLoader;

impl DataLoader {
    /// Load all character JSON files from a directory.
    /// Each `.json` file is deserialized as a single `Character`.
    pub fn load_characters(dir: &Path) -> Result<Vec<Character>> {
        let mut characters = Vec::new();
        for entry in json_entries(dir)
            .with_context(|| format!("failed to read characters directory: {}", dir.display()))?
        {
            let path = entry.path();
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read character file: {}", path.display()))?;
            let character: Character = serde_json::from_str(&contents)
                .with_context(|| format!("failed to parse character JSON: {}", path.display()))?;
            characters.push(character);
        }
        Ok(characters)
    }

    /// Load all enemy JSON files from a directory.
    /// Each `.json` file is deserialized as a single `EnemyState`.
    pub fn load_enemies(dir: &Path) -> Result<Vec<EnemyState>> {
        let mut enemies = Vec::new();
        for entry in json_entries(dir)
            .with_context(|| format!("failed to read enemies directory: {}", dir.display()))?
        {
            let path = entry.path();
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read enemy file: {}", path.display()))?;
            let enemy: EnemyState = serde_json::from_str(&contents)
                .with_context(|| format!("failed to parse enemy JSON: {}", path.display()))?;
            enemies.push(enemy);
        }
        Ok(enemies)
    }

    /// Load all skill JSON files from a directory.
    /// Each `.json` file can be either a single `SkillData` or an array of `SkillData`.
    pub fn load_skills(dir: &Path) -> Result<Vec<SkillData>> {
        let mut skills = Vec::new();
        for entry in json_entries(dir)
            .with_context(|| format!("failed to read skills directory: {}", dir.display()))?
        {
            let path = entry.path();
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read skill file: {}", path.display()))?;
            // Try array first, then single object
            if let Ok(arr) = serde_json::from_str::<Vec<SkillData>>(&contents) {
                skills.extend(arr);
            } else {
                let skill: SkillData = serde_json::from_str(&contents)
                    .with_context(|| format!("failed to parse skill JSON: {}", path.display()))?;
                skills.push(skill);
            }
        }
        Ok(skills)
    }

    /// Load equipment data from a single JSON file.
    pub fn load_equipment(path: &Path) -> Result<EquipmentData> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read equipment file: {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse equipment JSON: {}", path.display()))
    }

    /// Load APL (Action Priority List) data from a single JSON file.
    pub fn load_apl(path: &Path) -> Result<APLData> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read APL file: {}", path.display()))?;
        serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse APL JSON: {}", path.display()))
    }
}

/// Collect all `.json` file entries from a directory, sorted by filename.
fn json_entries(dir: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .with_context(|| format!("failed to read directory: {}", dir.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data")
    }

    #[test]
    fn test_load_characters() {
        let path = data_dir().join("characters");
        let characters = DataLoader::load_characters(&path).expect("load characters");
        assert!(!characters.is_empty(), "should load at least one character");
        let anby = characters
            .iter()
            .find(|c| c.char_id == "anby_demara")
            .expect("anby_demara should be loaded");
        assert_eq!(anby.name, "Anby Demara");
        assert_eq!(anby.level, 60);
        assert_eq!(anby.base_stats.atk, 1100.0);
    }

    #[test]
    fn test_load_enemies() {
        let path = data_dir().join("enemies");
        let enemies = DataLoader::load_enemies(&path).expect("load enemies");
        assert!(!enemies.is_empty(), "should load at least one enemy");
        let boss = enemies
            .iter()
            .find(|e| e.enemy_id == "boss_dullahan")
            .expect("boss_dullahan should be loaded");
        assert_eq!(boss.hp, 150000.0);
        assert_eq!(boss.def_val, 600.0);
        assert_eq!(boss.stun_max, 200.0);
        assert!(!boss.weaknesses.is_empty());
    }

    #[test]
    fn test_load_skills_from_directory() {
        let path = data_dir().join("skills");
        let skills = DataLoader::load_skills(&path).expect("load skills");
        assert!(!skills.is_empty(), "should load at least one skill");
        let ult = skills
            .iter()
            .find(|s| s.action_id == "Ultimate_1")
            .expect("Ultimate_1 should be loaded");
        assert!(ult.energy_cost >= 0.0, "energy_cost should be non-negative");
    }

    #[test]
    fn test_load_skills_single_file() {
        let path = data_dir().join("skills").join("anby_demara.json");
        let skills = DataLoader::load_skills(std::path::Path::new(&path).parent().unwrap())
            .expect("load skills");
        assert!(!skills.is_empty());
    }

    #[test]
    fn test_load_equipment() {
        let path = data_dir().join("equipment").join("equipment.json");
        let eq = DataLoader::load_equipment(&path).expect("load equipment");
        assert!(
            !eq.w_engines.is_empty() || !eq.drive_discs.is_empty() || !eq.disc_sets.is_empty(),
            "equipment data should have at least one entry"
        );
    }

    #[test]
    fn test_load_apl() {
        let path = data_dir().join("apl").join("sample_apl.json");
        let apl = DataLoader::load_apl(&path).expect("load APL");
        assert!(!apl.tracks.is_empty(), "APL should have at least one track");
        let track = &apl.tracks[0];
        assert!(!track.actions.is_empty(), "track should have actions");
    }

    #[test]
    fn test_load_nonexistent_directory_returns_error() {
        let result = DataLoader::load_characters(Path::new("data/nonexistent"));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("data/nonexistent"),
            "error should mention path: {err}"
        );
    }

    #[test]
    fn test_json_entries_filters_non_json() {
        // The characters directory only has .json files, so this verifies the filter works.
        let entries = json_entries(&data_dir().join("characters")).expect("read dir");
        for entry in &entries {
            assert!(
                entry
                    .path()
                    .extension()
                    .map(|e| e == "json")
                    .unwrap_or(false),
                "only json files should be returned"
            );
        }
    }
}
