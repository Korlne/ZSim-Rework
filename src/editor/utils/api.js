import { invoke } from "@tauri-apps/api/core";

// ── Database Init & Import ─────────────────────────────

export async function initDatabase() {
  const raw = await invoke("init_database");
  return JSON.parse(raw);
}

export async function importFromCsv(dataType, filePath) {
  const raw = await invoke("import_from_csv", { dataType, filePath });
  return JSON.parse(raw);
}

export async function importFromJson(dataType, dataPath) {
  const raw = await invoke("import_from_json", { dataType, dataPath: dataPath || null });
  return JSON.parse(raw);
}

export async function reimportAll() {
  const raw = await invoke("reimport_all");
  return JSON.parse(raw);
}

// ── Characters ─────────────────────────────────────────

export async function getCharacters() {
  const raw = await invoke("get_characters");
  return JSON.parse(raw);
}

export async function getCharacter(charId) {
  const raw = await invoke("get_character", { charId });
  return JSON.parse(raw);
}

export async function saveCharacter(data) {
  const raw = await invoke("save_character", { data: JSON.stringify(data) });
  return JSON.parse(raw);
}

export async function deleteCharacter(charId) {
  const raw = await invoke("delete_character", { charId });
  return JSON.parse(raw);
}

// ── Skills ─────────────────────────────────────────────

export async function getSkills(charId) {
  const raw = await invoke("get_skills", { charId });
  return JSON.parse(raw);
}

export async function saveSkill(data) {
  const raw = await invoke("save_skill", { data: JSON.stringify(data) });
  return JSON.parse(raw);
}

export async function deleteSkill(skillId) {
  const raw = await invoke("delete_skill", { skillId });
  return JSON.parse(raw);
}

// ── Equipment (W-Engines) ──────────────────────────────

export async function getAllEquipment() {
  const raw = await invoke("get_all_equipment");
  return JSON.parse(raw);
}

export async function getWEngines() {
  const raw = await invoke("get_w_engines");
  return JSON.parse(raw);
}

export async function saveWEngine(data) {
  const raw = await invoke("save_w_engine", { data: JSON.stringify(data) });
  return JSON.parse(raw);
}

export async function deleteWEngine(id) {
  const raw = await invoke("delete_w_engine", { id });
  return JSON.parse(raw);
}

// ── Equipment (Drive Discs) ────────────────────────────

export async function getDriveDiscs() {
  const raw = await invoke("get_drive_discs");
  return JSON.parse(raw);
}

export async function getDriveDiscsBySetId(setId) {
  const raw = await invoke("get_drive_discs_by_set_id", { setId });
  return JSON.parse(raw);
}

export async function saveDriveDisc(data) {
  const raw = await invoke("save_drive_disc", { data: JSON.stringify(data) });
  return JSON.parse(raw);
}

export async function deleteDriveDisc(id) {
  const raw = await invoke("delete_drive_disc", { id });
  return JSON.parse(raw);
}

// ── Equipment (Disc Sets) ──────────────────────────────

export async function getDiscSets() {
  const raw = await invoke("get_disc_sets");
  return JSON.parse(raw);
}

export async function saveDiscSet(data) {
  const raw = await invoke("save_disc_set", { data: JSON.stringify(data) });
  return JSON.parse(raw);
}

export async function deleteDiscSet(setId) {
  const raw = await invoke("delete_disc_set", { setId });
  return JSON.parse(raw);
}

// ── Enemies ────────────────────────────────────────────

export async function getEnemies() {
  const raw = await invoke("get_enemies");
  return JSON.parse(raw);
}

export async function getEnemy(enemyId) {
  const raw = await invoke("get_enemy", { enemyId });
  return JSON.parse(raw);
}

export async function saveEnemy(data) {
  const raw = await invoke("save_enemy", { data: JSON.stringify(data) });
  return JSON.parse(raw);
}

export async function deleteEnemy(enemyId) {
  const raw = await invoke("delete_enemy", { enemyId });
  return JSON.parse(raw);
}

// ── Data Directory Scan ──────────────────────────────────

export async function scanDataFiles() {
  const raw = await invoke("scan_data_files");
  return JSON.parse(raw);
}

export async function clearDataType(dataType) {
  const raw = await invoke("clear_data_type", { dataType });
  return JSON.parse(raw);
}

export async function exportToJson() {
  const raw = await invoke("export_to_json");
  return JSON.parse(raw);
}

// ── Deployed Configs ────────────────────────────────────

export async function getDeployedConfigs() {
  const raw = await invoke("list_deployed_configs");
  return JSON.parse(raw);
}

export async function getDeployedConfig(configId) {
  const raw = await invoke("get_deployed_config", { configId });
  return JSON.parse(raw);
}

export async function saveDeployedConfig(data) {
  const raw = await invoke("save_deployed_config", { data: JSON.stringify(data) });
  return JSON.parse(raw);
}

export async function deleteDeployedConfig(configId) {
  const raw = await invoke("delete_deployed_config", { configId });
  return JSON.parse(raw);
}

export async function duplicateDeployedConfig(configId) {
  const raw = await invoke("duplicate_deployed_config", { configId });
  return JSON.parse(raw);
}

// ── Data Queries ───────────────────────────────────────

export async function getDataSummary() {
  const raw = await invoke("get_data_summary");
  return JSON.parse(raw);
}

export async function searchData(query, dataType = "all") {
  const raw = await invoke("search_data", { query, dataType });
  return JSON.parse(raw);
}
