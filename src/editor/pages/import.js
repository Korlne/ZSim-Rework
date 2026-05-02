import { scanDataFiles, importFromJson, clearDataType, reimportAll } from "../utils/api.js";
import { t } from "../../i18n.js";

let scanResults = null;

export function renderPage() {
  const container = document.createElement("div");
  container.className = "page-import";

  // Header
  const header = document.createElement("h2");
  header.textContent = t("editor.nav.import");
  container.appendChild(header);

  // Mode selector
  const optionsDiv = document.createElement("div");
  optionsDiv.className = "import-options";

  const modeLabel = document.createElement("div");
  modeLabel.className = "section-title";
  modeLabel.textContent = t("editor.import.modeLabel");
  optionsDiv.appendChild(modeLabel);

  const overwriteRow = document.createElement("label");
  overwriteRow.className = "import-option-row";
  const overwriteRadio = document.createElement("input");
  overwriteRadio.type = "radio";
  overwriteRadio.name = "import-mode";
  overwriteRadio.value = "overwrite";
  overwriteRadio.checked = true;
  const overwriteSpan = document.createElement("span");
  overwriteSpan.innerHTML = `<strong>${t("editor.import.modeOverwrite")}</strong> — ${t("editor.import.modeOverwriteDesc")}`;
  overwriteRow.appendChild(overwriteRadio);
  overwriteRow.appendChild(overwriteSpan);
  optionsDiv.appendChild(overwriteRow);

  const appendRow = document.createElement("label");
  appendRow.className = "import-option-row";
  const appendRadio = document.createElement("input");
  appendRadio.type = "radio";
  appendRadio.name = "import-mode";
  appendRadio.value = "append";
  const appendSpan = document.createElement("span");
  appendSpan.innerHTML = `<strong>${t("editor.import.modeAppend")}</strong> — ${t("editor.import.modeAppendDesc")}`;
  appendRow.appendChild(appendRadio);
  appendRow.appendChild(appendSpan);
  optionsDiv.appendChild(appendRow);

  container.appendChild(optionsDiv);

  // Scan button
  const actionRow = document.createElement("div");
  actionRow.className = "button-row";
  actionRow.style.marginBottom = "16px";

  const scanBtn = document.createElement("button");
  scanBtn.className = "btn btn-primary";
  scanBtn.textContent = t("editor.import.scanBtn");
  scanBtn.addEventListener("click", async () => {
    scanBtn.disabled = true;
    scanBtn.textContent = t("editor.loading");
    try {
      const raw = await scanDataFiles();
      scanResults = typeof raw === "string" ? JSON.parse(raw) : raw;
      renderFileList(fileListContainer);
    } catch (err) {
      showNotif(t("editor.import.scanError") + ": " + err, "error");
    } finally {
      scanBtn.disabled = false;
      scanBtn.textContent = t("editor.import.scanBtn");
    }
  });
  actionRow.appendChild(scanBtn);

  // Import All button
  const importAllBtn = document.createElement("button");
  importAllBtn.className = "btn btn-secondary";
  importAllBtn.textContent = t("editor.import.importAllBtn");
  importAllBtn.addEventListener("click", async () => {
    const mode = document.querySelector('input[name="import-mode"]:checked').value;
    await importAllData(mode, fileListContainer, importAllBtn);
  });
  actionRow.appendChild(importAllBtn);

  container.appendChild(actionRow);

  // File list container
  const fileListContainer = document.createElement("div");
  fileListContainer.id = "import-file-list";

  // Initial message
  const initialMsg = document.createElement("p");
  initialMsg.className = "text-muted";
  initialMsg.textContent = t("editor.import.noFiles");
  fileListContainer.appendChild(initialMsg);

  container.appendChild(fileListContainer);

  return container;
}

function getMode() {
  return document.querySelector('input[name="import-mode"]:checked').value;
}

function renderFileList(container) {
  if (!scanResults || Object.keys(scanResults).length === 0) {
    container.innerHTML = `<p class="text-muted">${t("editor.import.noFiles")}</p>`;
    return;
  }

  const typeLabels = {
    characters: t("editor.import.typeCharacters"),
    skills: t("editor.import.typeSkills"),
    equipment: t("editor.import.typeEquipment"),
    enemies: t("editor.import.typeEnemies"),
    apl: t("editor.import.typeApl"),
  };

  const dataTypeKeys = {
    characters: "characters",
    skills: "skills",
    equipment: "equipment",
    enemies: "enemies",
    apl: "apl",
  };

  container.innerHTML = "";

  for (const [type, files] of Object.entries(scanResults)) {
    if (!files || files.length === 0) continue;

    const section = document.createElement("div");
    section.className = "form-section";

    const titleRow = document.createElement("div");
    titleRow.style.display = "flex";
    titleRow.style.alignItems = "center";
    titleRow.style.justifyContent = "space-between";
    titleRow.style.marginBottom = "8px";

    const title = document.createElement("h4");
    title.className = "section-title";
    title.style.margin = "0";
    title.textContent = `${typeLabels[type] || type} (${t("editor.import.fileCount", { count: files.length })})`;
    titleRow.appendChild(title);

    const importBtn = document.createElement("button");
    importBtn.className = "btn btn-sm btn-secondary";
    importBtn.textContent = t("editor.import.importBtn");
    importBtn.addEventListener("click", async () => {
      await importDataType(type, getMode(), importBtn);
    });
    titleRow.appendChild(importBtn);

    section.appendChild(titleRow);

    // File names
    const fileList = document.createElement("div");
    fileList.style.display = "flex";
    fileList.style.flexWrap = "wrap";
    fileList.style.gap = "4px 8px";
    fileList.style.fontSize = "12px";
    fileList.style.color = "var(--text-muted)";

    for (const file of files) {
      const fileTag = document.createElement("span");
      fileTag.className = "tag-item";
      fileTag.textContent = file;
      fileList.appendChild(fileTag);
    }
    section.appendChild(fileList);

    container.appendChild(section);
  }
}

async function importDataType(dataType, mode, btn) {
  btn.disabled = true;
  const originalText = btn.textContent;
  btn.textContent = t("editor.import.importing");

  try {
    if (mode === "overwrite") {
      await clearDataType(dataType);
    }
    const raw = await importFromJson(dataType, null);
    const result = typeof raw === "string" ? JSON.parse(raw) : raw;
    const success = result.success || 0;
    const errors = result.errors || [];
    const typeLabel = {
      characters: t("editor.import.typeCharacters"),
      skills: t("editor.import.typeSkills"),
      equipment: t("editor.import.typeEquipment"),
      enemies: t("editor.import.typeEnemies"),
      apl: t("editor.import.typeApl"),
    }[dataType] || dataType;
    showNotif(
      t("editor.import.resultSuccess", { type: typeLabel, count: success }),
      errors.length > 0 ? "warning" : "success"
    );
    if (errors.length > 0) {
      console.warn("Import errors:", errors);
      showNotif(t("editor.import.resultErrors", { type: typeLabel, count: errors.length }), "error");
    }
  } catch (err) {
    showNotif(t("editor.import.scanError") + ": " + err, "error");
  } finally {
    btn.disabled = false;
    btn.textContent = originalText;
  }
}

async function importAllData(mode, fileListContainer, btn) {
  btn.disabled = true;
  const originalText = btn.textContent;
  btn.textContent = t("editor.import.importing");

  try {
    if (mode === "overwrite") {
      const raw = await reimportAll();
      const result = typeof raw === "string" ? JSON.parse(raw) : raw;
      const imported = result.imported || {};
      let totalSuccess = 0;
      let totalErrors = 0;
      for (const [type, res] of Object.entries(imported)) {
        totalSuccess += res.success || 0;
        totalErrors += (res.errors || []).length;
      }
      showNotif(
        t("editor.notif.imported", { success: totalSuccess, errors: totalErrors }),
        totalErrors > 0 ? "warning" : "success"
      );
    } else {
      // Append mode: import each type individually
      const types = ["characters", "skills", "equipment", "enemies", "apl"];
      for (const dataType of types) {
        if (scanResults && scanResults[dataType] && scanResults[dataType].length > 0) {
          const raw = await importFromJson(dataType, null);
          const result = typeof raw === "string" ? JSON.parse(raw) : raw;
          const success = result.success || 0;
          const errors = result.errors || [];
          if (errors.length > 0) {
            showNotif(
              `${dataType}: ${success} ok, ${errors.length} errors`,
              "warning"
            );
          }
        }
      }
      showNotif(t("editor.import.importAllDone"), "success");
    }
  } catch (err) {
    showNotif(t("editor.import.scanError") + ": " + err, "error");
  } finally {
    btn.disabled = false;
    btn.textContent = originalText;
  }
}

function showNotif(message, type = "info") {
  const existing = document.querySelector(".editor-notification");
  if (existing) existing.remove();

  const el = document.createElement("div");
  el.className = `editor-notification ${type}`;
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}
