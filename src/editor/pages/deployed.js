import { getDeployedConfigs, deleteDeployedConfig, duplicateDeployedConfig, saveDeployedConfig, getCharacters, getWEngines } from "../utils/api.js";
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

// ── Edit Form (placeholder for US-009) ───────────────────

function renderEditForm(configId) {
  const container = document.createElement("div");
  container.className = "page-deployed-edit";

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

  const body = document.createElement("div");
  body.className = "page-loading";
  body.textContent = t("editor.deployed.editComingSoon");
  container.appendChild(body);

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
