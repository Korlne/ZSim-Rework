import { getDeployedConfigs, getDeployedConfig, deleteDeployedConfig, duplicateDeployedConfig, saveDeployedConfig, getCharacters, getWEngines } from "../utils/api.js";
import { t } from "../../i18n.js";

export function renderPage(route) {
  const hash = window.location.hash.replace(/^#\//, "");
  const parts = hash.split("/");

  // Edit mode: #/deployed/{configId} or #/deployed/new
  if (parts.length > 1 && parts[1]) {
    return renderEditForm(parts[1]);
  }

  return renderListPage();
}

// ── List Page ────────────────────────────────────────────

function renderListPage() {
  const container = document.createElement("div");
  container.className = "page-deployed";

  const header = document.createElement("div");
  header.className = "deployed-header";
  header.style.display = "flex";
  header.style.alignItems = "center";
  header.style.justifyContent = "space-between";
  header.style.marginBottom = "16px";

  const title = document.createElement("h2");
  title.textContent = t("editor.deployed.title");
  header.appendChild(title);

  const newBtn = document.createElement("button");
  newBtn.className = "btn btn-primary";
  newBtn.textContent = "+ " + t("editor.deployed.newConfig");
  newBtn.addEventListener("click", async () => {
    newBtn.disabled = true;
    try {
      const chars = await getCharacters();
      if (!chars || chars.length === 0) {
        showNotification(t("editor.deployed.noCharacters"), "error");
        return;
      }
      const data = {
        config_id: "",
        name: t("editor.deployed.defaultName"),
        char_id: chars[0].char_id,
        char_level: 1,
        char_ascension: 0,
        cinemas: "[false,false,false,false,false,false]",
        potentials: "[false,false,false,false,false,false]",
        wengine_id: null,
        wengine_level: null,
        wengine_ascension: null,
        disc_configs: "[]",
      };
      const result = await saveDeployedConfig(data);
      window.location.hash = `#/deployed/${result.config_id}`;
    } catch (err) {
      showNotification(t("editor.deployed.createError") + ": " + err, "error");
    } finally {
      newBtn.disabled = false;
    }
  });
  header.appendChild(newBtn);
  container.appendChild(header);

  const grid = document.createElement("div");
  grid.className = "deployed-grid";
  grid.id = "deployed-grid";
  container.appendChild(grid);

  loadConfigList(grid);

  return container;
}

async function loadConfigList(grid) {
  grid.innerHTML = `<div class="page-loading">${t("editor.loading")}</div>`;

  try {
    const [configs, chars, wengines] = await Promise.all([
      getDeployedConfigs(),
      getCharacters(),
      getWEngines(),
    ]);

    const charMap = {};
    for (const c of (Array.isArray(chars) ? chars : [])) {
      charMap[c.char_id] = c.name || c.char_id;
    }
    const wengineMap = {};
    for (const w of (Array.isArray(wengines) ? wengines : [])) {
      wengineMap[w.id || w.wengine_id] = w.name || w.id || w.wengine_id;
    }

    if (!configs || configs.length === 0) {
      grid.innerHTML = `<div class="deployed-empty">${t("editor.deployed.empty")}</div>`;
      return;
    }

    grid.innerHTML = "";
    for (const cfg of configs) {
      grid.appendChild(renderConfigCard(cfg, charMap, wengineMap, grid));
    }
  } catch (err) {
    grid.innerHTML = `<div class="page-error"><p>${t("editor.deployed.loadError")}: ${err.message || err}</p></div>`;
  }
}

function renderConfigCard(cfg, charMap, wengineMap, grid) {
  const card = document.createElement("div");
  card.className = "deployed-card";

  // Character name
  const charName = charMap[cfg.char_id] || cfg.char_id;

  // Wengine name
  let wengineName = t("editor.deployed.noWengine");
  if (cfg.wengine_id && wengineMap[cfg.wengine_id]) {
    wengineName = wengineMap[cfg.wengine_id];
  }

  // Cinema count
  let cinemaCount = 0;
  try {
    const cins = JSON.parse(cfg.cinemas || "[]");
    cinemaCount = cins.filter(Boolean).length;
  } catch { /* ignore */ }

  // Format date
  let dateStr = "";
  if (cfg.created_at) {
    try {
      dateStr = new Date(cfg.created_at).toLocaleDateString();
    } catch { dateStr = cfg.created_at; }
  }

  card.innerHTML = `
    <div class="deployed-card-title">${escapeHtml(cfg.name)}</div>
    <div class="deployed-card-body">
      <div class="deployed-card-row">
        <span class="deployed-card-label">${t("editor.deployed.charLabel")}</span>
        <span class="deployed-card-value">${escapeHtml(charName)}</span>
      </div>
      <div class="deployed-card-row">
        <span class="deployed-card-label">${t("editor.deployed.wengineLabel")}</span>
        <span class="deployed-card-value">${escapeHtml(wengineName)}</span>
      </div>
      <div class="deployed-card-row">
        <span class="deployed-card-label">${t("editor.deployed.statsLabel")}</span>
        <span class="deployed-card-value">Lv.${cfg.char_level || 1} / A${cfg.char_ascension || 0} / C${cinemaCount}</span>
      </div>
      ${dateStr ? `
      <div class="deployed-card-row">
        <span class="deployed-card-label">${t("editor.deployed.createdLabel")}</span>
        <span class="deployed-card-value">${dateStr}</span>
      </div>` : ""}
    </div>
    <div class="deployed-card-actions">
      <button class="btn btn-sm btn-secondary" data-action="edit" data-id="${cfg.config_id}">${t("editor.action.edit")}</button>
      <button class="btn btn-sm btn-secondary" data-action="duplicate" data-id="${cfg.config_id}">${t("editor.deployed.duplicate")}</button>
      <button class="btn btn-sm btn-danger" data-action="delete" data-id="${cfg.config_id}">${t("editor.action.delete")}</button>
    </div>
  `;

  // Event delegation via card click
  card.querySelectorAll("button[data-action]").forEach(btn => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const action = btn.dataset.action;
      const configId = btn.dataset.id;

      if (action === "edit") {
        window.location.hash = `#/deployed/${configId}`;
      } else if (action === "duplicate") {
        handleDuplicate(configId, grid);
      } else if (action === "delete") {
        handleDelete(configId, cfg.name, grid);
      }
    });
  });

  return card;
}

// ── Actions ──────────────────────────────────────────────

async function handleDuplicate(configId, grid) {
  try {
    await duplicateDeployedConfig(configId);
    showNotification(t("editor.deployed.duplicated"), "success");
    loadConfigList(grid);
  } catch (err) {
    showNotification(t("editor.deployed.duplicateError") + ": " + err, "error");
  }
}

async function handleDelete(configId, name, grid) {
  const confirmed = await showConfirmDialog(
    t("editor.deployed.confirmDelete", { name: name })
  );
  if (!confirmed) return;

  try {
    await deleteDeployedConfig(configId);
    showNotification(t("editor.deployed.deleted"), "success");
    loadConfigList(grid);
  } catch (err) {
    showNotification(t("editor.deployed.deleteError") + ": " + err, "error");
  }
}

// ── Edit Form ───────────────────────────────────────────

function parseBoolArray(raw, len = 6) {
  if (!raw) return new Array(len).fill(false);
  try {
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return new Array(len).fill(false);
    return arr.map(Boolean);
  } catch {
    return new Array(len).fill(false);
  }
}

function createToggleSection(sectionLabel, stateArray, btnPrefix) {
  const section = document.createElement("div");
  section.style.marginTop = "12px";

  const label = document.createElement("div");
  label.className = "form-label";
  label.textContent = sectionLabel;
  section.appendChild(label);

  const row = document.createElement("div");
  row.style.cssText = "display:flex;gap:8px;margin-top:6px";
  const btnRefs = [];

  for (let i = 0; i < stateArray.length; i++) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.textContent = btnPrefix + (i + 1);
    const updateBtnStyle = () => {
      btn.style.background = stateArray[i] ? "var(--primary)" : "var(--surface)";
      btn.style.color = stateArray[i] ? "#fff" : "var(--text)";
    };
    btn.style.cssText = `
      padding:8px 16px;border-radius:6px;border:1px solid var(--border);
      cursor:pointer;font-family:var(--font);font-size:13px;font-weight:600;
      transition:background 0.15s;
    `;
    updateBtnStyle();

    btn.addEventListener("click", () => {
      if (!stateArray[i]) {
        for (let j = 0; j <= i; j++) stateArray[j] = true;
      } else {
        for (let j = i; j < stateArray.length; j++) stateArray[j] = false;
      }
      for (let k = 0; k < btnRefs.length; k++) {
        btnRefs[k].style.background = stateArray[k] ? "var(--primary)" : "var(--surface)";
        btnRefs[k].style.color = stateArray[k] ? "#fff" : "var(--text)";
      }
    });

    row.appendChild(btn);
    btnRefs.push(btn);
  }
  section.appendChild(row);
  return { section, btnRefs };
}

async function renderEditForm(configId) {
  const container = document.createElement("div");
  container.className = "page-deployed-edit";

  // Loading state
  container.innerHTML = `<div class="page-loading">${t("editor.loading")}</div>`;

  let configData = null;
  let characters = [];
  let wengines = [];

  try {
    const promises = [getCharacters(), getWEngines()];
    if (configId !== "new") {
      promises.push(getDeployedConfig(configId));
    }
    const results = await Promise.all(promises);
    characters = Array.isArray(results[0]) ? results[0] : [];
    wengines = Array.isArray(results[1]) ? results[1] : [];
    if (configId !== "new") {
      configData = results[2];
    }
  } catch (err) {
    container.innerHTML = `<div class="page-error"><p>${t("editor.deployed.loadError")}: ${err.message || err}</p></div>`;
    return container;
  }

  // Initialize form data
  let formData;
  if (configData) {
    formData = { ...configData };
  } else {
    formData = {
      config_id: "",
      name: t("editor.deployed.defaultName"),
      char_id: characters.length > 0 ? characters[0].char_id : "",
      char_level: 1,
      char_ascension: 0,
      cinemas: "[false,false,false,false,false,false]",
      potentials: "[false,false,false,false,false,false]",
      wengine_id: null,
      wengine_level: null,
      wengine_ascension: null,
      disc_configs: "[]",
    };
  }

  const cinemas = parseBoolArray(formData.cinemas);
  const potentials = parseBoolArray(formData.potentials);

  container.innerHTML = "";

  // Header with back button and title
  const header = document.createElement("div");
  header.style.display = "flex";
  header.style.alignItems = "center";
  header.style.gap = "12px";
  header.style.marginBottom = "16px";

  const backBtn = document.createElement("button");
  backBtn.className = "btn btn-secondary btn-sm";
  backBtn.textContent = "← " + t("editor.deployed.back");
  backBtn.addEventListener("click", () => {
    window.location.hash = "#/deployed";
  });
  header.appendChild(backBtn);

  const title = document.createElement("h2");
  title.textContent = configId === "new"
    ? t("editor.deployed.newConfig")
    : t("editor.deployed.editConfig");
  title.style.margin = "0";
  header.appendChild(title);
  container.appendChild(header);

  // Build form
  const formEl = document.createElement("div");
  formEl.className = "editor-form";

  // ── Config Name ──────────────────────────────────────
  const nameGroup = document.createElement("div");
  nameGroup.className = "form-field";
  nameGroup.style.marginBottom = "16px";

  const nameLabel = document.createElement("label");
  nameLabel.className = "form-label";
  nameLabel.textContent = t("editor.deployed.configName");
  nameGroup.appendChild(nameLabel);

  const nameInput = document.createElement("input");
  nameInput.type = "text";
  nameInput.className = "form-input";
  nameInput.value = formData.name || "";
  nameInput.addEventListener("input", () => { formData.name = nameInput.value; });
  nameGroup.appendChild(nameInput);
  formEl.appendChild(nameGroup);

  // ── Character Section ────────────────────────────────
  const charSection = document.createElement("div");
  charSection.className = "form-section";

  const charSectionTitle = document.createElement("div");
  charSectionTitle.className = "section-title";
  charSectionTitle.textContent = t("editor.deployed.sectionCharacter");
  charSection.appendChild(charSectionTitle);

  // Character select
  const charSelectGroup = document.createElement("div");
  charSelectGroup.className = "form-field";
  charSelectGroup.style.marginBottom = "12px";

  const charSelectLabel = document.createElement("label");
  charSelectLabel.className = "form-label";
  charSelectLabel.textContent = t("editor.deployed.charLabel");
  charSelectGroup.appendChild(charSelectLabel);

  const charSelect = document.createElement("select");
  charSelect.className = "form-input form-select";

  const charPlaceholder = document.createElement("option");
  charPlaceholder.value = "";
  charPlaceholder.disabled = true;
  charPlaceholder.hidden = true;
  charPlaceholder.textContent = t("editor.deployed.selectCharacter");
  charSelect.appendChild(charPlaceholder);

  for (const c of characters) {
    const opt = document.createElement("option");
    opt.value = c.char_id;
    opt.textContent = c.name || c.char_id;
    if (c.char_id === formData.char_id) opt.selected = true;
    charSelect.appendChild(opt);
  }
  charSelect.addEventListener("change", () => {
    formData.char_id = charSelect.value;
  });
  charSelectGroup.appendChild(charSelect);
  charSection.appendChild(charSelectGroup);

  // Character level & ascension
  const charLevelsRow = document.createElement("div");
  charLevelsRow.className = "form-fields";
  charLevelsRow.style.marginBottom = "8px";

  const charLevelGroup = document.createElement("div");
  charLevelGroup.className = "form-field";
  const charLevelLabel = document.createElement("label");
  charLevelLabel.className = "form-label";
  charLevelLabel.textContent = t("editor.deployed.charLevel");
  charLevelGroup.appendChild(charLevelLabel);
  const charLevelInput = document.createElement("input");
  charLevelInput.type = "number";
  charLevelInput.className = "form-input";
  charLevelInput.min = 1;
  charLevelInput.max = 60;
  charLevelInput.value = formData.char_level || 1;
  charLevelInput.addEventListener("input", () => {
    formData.char_level = charLevelInput.value === "" ? 1 : parseInt(charLevelInput.value, 10);
  });
  charLevelGroup.appendChild(charLevelInput);
  charLevelsRow.appendChild(charLevelGroup);

  const charAscGroup = document.createElement("div");
  charAscGroup.className = "form-field";
  const charAscLabel = document.createElement("label");
  charAscLabel.className = "form-label";
  charAscLabel.textContent = t("editor.deployed.charAscension");
  charAscGroup.appendChild(charAscLabel);
  const charAscInput = document.createElement("input");
  charAscInput.type = "number";
  charAscInput.className = "form-input";
  charAscInput.min = 0;
  charAscInput.max = 6;
  charAscInput.value = formData.char_ascension || 0;
  charAscInput.addEventListener("input", () => {
    formData.char_ascension = charAscInput.value === "" ? 0 : parseInt(charAscInput.value, 10);
  });
  charAscGroup.appendChild(charAscInput);
  charLevelsRow.appendChild(charAscGroup);
  charSection.appendChild(charLevelsRow);

  // Cinema toggles
  charSection.appendChild(createToggleSection(
    t("editor.characters.sectionCinema"), cinemas, "C"
  ).section);

  // Potential toggles
  charSection.appendChild(createToggleSection(
    t("editor.characters.sectionPotential"), potentials, "P"
  ).section);

  formEl.appendChild(charSection);

  // ── W-Engine Section ─────────────────────────────────
  const wengineSection = document.createElement("div");
  wengineSection.className = "form-section";

  const wengineSectionTitle = document.createElement("div");
  wengineSectionTitle.className = "section-title";
  wengineSectionTitle.textContent = t("editor.deployed.sectionWengine");
  wengineSection.appendChild(wengineSectionTitle);

  // Wengine select
  const wengineSelectGroup = document.createElement("div");
  wengineSelectGroup.className = "form-field";
  wengineSelectGroup.style.marginBottom = "12px";

  const wengineSelectLabel = document.createElement("label");
  wengineSelectLabel.className = "form-label";
  wengineSelectLabel.textContent = t("editor.deployed.wengineLabel");
  wengineSelectGroup.appendChild(wengineSelectLabel);

  const wengineSelect = document.createElement("select");
  wengineSelect.className = "form-input form-select";

  // "Unequip" option
  const unequipOpt = document.createElement("option");
  unequipOpt.value = "";
  unequipOpt.textContent = t("editor.deployed.unequip");
  if (!formData.wengine_id) unequipOpt.selected = true;
  wengineSelect.appendChild(unequipOpt);

  const wengineIdField = (w) => w.id || w.wengine_id;
  for (const w of wengines) {
    const opt = document.createElement("option");
    opt.value = wengineIdField(w);
    opt.textContent = w.name || wengineIdField(w);
    if (wengineIdField(w) === formData.wengine_id) opt.selected = true;
    wengineSelect.appendChild(opt);
  }
  wengineSelect.addEventListener("change", () => {
    formData.wengine_id = wengineSelect.value || null;
    // Show/hide level/ascension fields based on selection
    wengineLevelsRow.style.display = wengineSelect.value ? "" : "none";
  });
  wengineSelectGroup.appendChild(wengineSelect);
  wengineSection.appendChild(wengineSelectGroup);

  // Wengine level & ascension
  const wengineLevelsRow = document.createElement("div");
  wengineLevelsRow.className = "form-fields";
  wengineLevelsRow.style.display = formData.wengine_id ? "" : "none";

  const wengineLevelGroup = document.createElement("div");
  wengineLevelGroup.className = "form-field";
  const wengineLevelLabel = document.createElement("label");
  wengineLevelLabel.className = "form-label";
  wengineLevelLabel.textContent = t("editor.deployed.wengineLevel");
  wengineLevelGroup.appendChild(wengineLevelLabel);
  const wengineLevelInput = document.createElement("input");
  wengineLevelInput.type = "number";
  wengineLevelInput.className = "form-input";
  wengineLevelInput.min = 1;
  wengineLevelInput.max = 60;
  wengineLevelInput.value = formData.wengine_level || 1;
  wengineLevelInput.addEventListener("input", () => {
    formData.wengine_level = wengineLevelInput.value === "" ? null : parseInt(wengineLevelInput.value, 10);
  });
  wengineLevelGroup.appendChild(wengineLevelInput);
  wengineLevelsRow.appendChild(wengineLevelGroup);

  const wengineAscGroup = document.createElement("div");
  wengineAscGroup.className = "form-field";
  const wengineAscLabel = document.createElement("label");
  wengineAscLabel.className = "form-label";
  wengineAscLabel.textContent = t("editor.deployed.wengineAscension");
  wengineAscGroup.appendChild(wengineAscLabel);
  const wengineAscInput = document.createElement("input");
  wengineAscInput.type = "number";
  wengineAscInput.className = "form-input";
  wengineAscInput.min = 0;
  wengineAscInput.max = 6;
  wengineAscInput.value = formData.wengine_ascension || 0;
  wengineAscInput.addEventListener("input", () => {
    formData.wengine_ascension = wengineAscInput.value === "" ? null : parseInt(wengineAscInput.value, 10);
  });
  wengineAscGroup.appendChild(wengineAscInput);
  wengineLevelsRow.appendChild(wengineAscGroup);
  wengineSection.appendChild(wengineLevelsRow);

  formEl.appendChild(wengineSection);

  // ── Buttons ──────────────────────────────────────────
  const btnRow = document.createElement("div");
  btnRow.className = "form-buttons";

  const saveBtn = document.createElement("button");
  saveBtn.className = "btn btn-primary";
  saveBtn.textContent = t("editor.action.save");
  saveBtn.addEventListener("click", async () => {
    saveBtn.disabled = true;
    try {
      const payload = {
        ...formData,
        cinemas: JSON.stringify(cinemas),
        potentials: JSON.stringify(potentials),
      };
      const result = await saveDeployedConfig(payload);
      // Update config_id if new
      if (!formData.config_id && result.config_id) {
        formData.config_id = result.config_id;
        window.location.hash = `#/deployed/${result.config_id}`;
      }
      showNotification(t("editor.deployed.saveSuccess"), "success");
    } catch (err) {
      showNotification(t("editor.deployed.saveError") + ": " + (err.message || err), "error");
    } finally {
      saveBtn.disabled = false;
    }
  });

  const cancelBtn = document.createElement("button");
  cancelBtn.className = "btn btn-secondary";
  cancelBtn.textContent = t("editor.action.cancel");
  cancelBtn.addEventListener("click", () => {
    window.location.hash = "#/deployed";
  });

  btnRow.appendChild(saveBtn);
  btnRow.appendChild(cancelBtn);
  formEl.appendChild(btnRow);

  container.appendChild(formEl);

  return container;
}

// ── Helpers ──────────────────────────────────────────────

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

function showNotification(message, type = "info") {
  const existing = document.querySelector(".editor-notification");
  if (existing) existing.remove();

  const el = document.createElement("div");
  el.className = `editor-notification ${type}`;
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}

function showConfirmDialog(message) {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "confirm-overlay";

    const dialog = document.createElement("div");
    dialog.className = "confirm-dialog";
    dialog.innerHTML = `
      <div class="confirm-message">${message}</div>
      <div class="confirm-buttons">
        <button class="btn btn-secondary" id="confirm-cancel">${t("editor.confirm.cancel")}</button>
        <button class="btn btn-danger" id="confirm-ok">${t("editor.confirm.delete")}</button>
      </div>
    `;

    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    const cleanup = (result) => {
      overlay.remove();
      resolve(result);
    };

    dialog.querySelector("#confirm-cancel").addEventListener("click", () => cleanup(false));
    dialog.querySelector("#confirm-ok").addEventListener("click", () => cleanup(true));
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) cleanup(false);
    });
  });
}
