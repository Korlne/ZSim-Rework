import { t } from "../../i18n.js";
import { createDataTable } from "../components/datatable.js";
import { createForm } from "../components/form.js";
import { confirm } from "../components/confirm.js";
import {
  getWEngines, saveWEngine, deleteWEngine,
  getDriveDiscs, saveDriveDisc, deleteDriveDisc,
  getDiscSets, saveDiscSet, deleteDiscSet,
} from "../utils/api.js";
import { formatStat, formatFixed } from "../utils/format.js";

// ── Route config ────────────────────────────────────────

const TAB_CONFIG = {
  "w-engines": { labelKey: "editor.nav.wEngines", icon: "⚙" },
  "drive-discs": { labelKey: "editor.nav.driveDiscs", icon: "💿" },
  "disc-sets": { labelKey: "editor.nav.discSets", icon: "📦" },
};

const SLOTS = [1, 2, 3, 4, 5, 6];

const STAT_OPTIONS = [
  "HP", "ATK", "DEF", "Crit Rate", "Crit DMG",
  "PEN Ratio", "PEN", "Energy Regen", "Anomaly Mastery",
  "Anomaly Proficiency", "Impact",
];

const ELEMENT_OPTIONS = ["Physical", "Fire", "Ice", "Electric", "Ether"];

// ── Render ──────────────────────────────────────────────

export function renderPage(route) {
  const container = document.createElement("div");

  // Sub-tab navigation
  const tabBar = document.createElement("nav");
  tabBar.className = "editor-tabs";
  tabBar.style.marginBottom = "16px";

  for (const [key, cfg] of Object.entries(TAB_CONFIG)) {
    const tab = document.createElement("button");
    tab.className = "editor-tab" + (key === route ? " active" : "");
    tab.textContent = t(cfg.labelKey);
    tab.addEventListener("click", () => {
      window.location.hash = "#/equipment/" + key;
    });
    tabBar.appendChild(tab);
  }
  container.appendChild(tabBar);

  // Content area
  const content = document.createElement("div");
  content.id = "equipment-content";
  container.appendChild(content);

  // Render the appropriate tab
  switch (route) {
    case "w-engines":
      renderWEngines(content);
      break;
    case "drive-discs":
      renderDriveDiscs(content);
      break;
    case "disc-sets":
      renderDiscSets(content);
      break;
    default:
      content.innerHTML = `<p class="text-muted">${t("editor.notFound")}</p>`;
  }

  return container;
}

// ═══════════════════════════════════════════════════════════
//  W-ENGINES
// ═══════════════════════════════════════════════════════════

async function renderWEngines(container) {
  let data = [];
  try {
    data = await getWEngines();
  } catch (e) {
    container.innerHTML = `<p class="error">${t("editor.loadError")}</p>`;
    return;
  }

  const headerRow = document.createElement("div");
  headerRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;margin-bottom:12px";

  const title = document.createElement("h2");
  title.style.margin = "0";
  title.textContent = t("editor.nav.wEngines");
  headerRow.appendChild(title);

  const addBtn = document.createElement("button");
  addBtn.className = "btn btn-primary";
  addBtn.textContent = t("editor.equipment.addBtn");
  addBtn.addEventListener("click", () => showWEngineForm(null, container));
  headerRow.appendChild(addBtn);
  container.appendChild(headerRow);

  const table = createDataTable(
    [
      { key: "id", label: t("editor.equipment.colId") },
      { key: "name", label: t("editor.equipment.colName") },
      { key: "level", label: t("editor.equipment.colLevel") },
      { key: "atk", label: "ATK", format: formatStat },
      { key: "crit_rate", label: t("editor.equipment.colCritRate"), format: (v) => formatFixed(v, 3) },
      { key: "crit_dmg", label: t("editor.equipment.colCritDmg"), format: (v) => formatFixed(v, 3) },
    ],
    data,
    {
      onEdit: (row) => showWEngineForm(row.id, container),
      onDelete: (row) => handleDelete("w-engine", row.id, row.name, container, renderWEngines),
      emptyMessage: t("editor.equipment.empty"),
    }
  );
  container.appendChild(table);
}

function showWEngineForm(id, rootContainer) {
  const existing = document.getElementById("equip-edit-container");
  if (existing) existing.remove();

  let values;
  if (id) {
    // We already have the data from the table
  }
  // Since get_all_equipment has all data, we need to fetch
  // For simplicity, open with empty values for new, or pre-filled via the row
  // Let's get data from the API
  showInlineForm(rootContainer, "w-engine", id, getEmptyWEngine(id), async (formValues) => {
    await saveWEngine(formValues);
  });
}

function getEmptyWEngine(id) {
  return {
    id: id || "",
    name: "",
    level: 1,
    ascension: 0,
    atk: 0,
    crit_rate: 0,
    crit_dmg: 0,
    pen_ratio: 0,
    energy_regen: 0,
    impact: 0,
    anomaly_mastery: 0,
    passive_effects: "[]",
  };
}

// ═══════════════════════════════════════════════════════════
//  DRIVE DISCS
// ═══════════════════════════════════════════════════════════

async function renderDriveDiscs(container) {
  let data = [];
  try {
    data = await getDriveDiscs();
  } catch (e) {
    container.innerHTML = `<p class="error">${t("editor.loadError")}</p>`;
    return;
  }

  const headerRow = document.createElement("div");
  headerRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;margin-bottom:12px";

  const title = document.createElement("h2");
  title.style.margin = "0";
  title.textContent = t("editor.nav.driveDiscs");
  headerRow.appendChild(title);

  const addBtn = document.createElement("button");
  addBtn.className = "btn btn-primary";
  addBtn.textContent = t("editor.equipment.addBtn");
  addBtn.addEventListener("click", () => showDriveDiscForm(null, container));
  headerRow.appendChild(addBtn);
  container.appendChild(headerRow);

  const table = createDataTable(
    [
      { key: "id", label: t("editor.equipment.colId") },
      { key: "slot", label: t("editor.equipment.colSlot") },
      { key: "level", label: t("editor.equipment.colLevel") },
      { key: "set_id", label: t("editor.equipment.colSetId") },
      { key: "main_stat_name", label: t("editor.equipment.colMainStat") },
      { key: "main_stat_value", label: t("editor.equipment.colMainVal"), format: formatStat },
    ],
    data,
    {
      onEdit: (row) => showDriveDiscForm(row.id, container),
      onDelete: (row) => handleDelete("drive-disc", row.id, row.id, container, renderDriveDiscs),
      emptyMessage: t("editor.equipment.empty"),
    }
  );
  container.appendChild(table);
}

function showDriveDiscForm(id, rootContainer) {
  const existing = document.getElementById("equip-edit-container");
  if (existing) existing.remove();
  showInlineForm(rootContainer, "drive-disc", id, getEmptyDriveDisc(id), async (formValues) => {
    await saveDriveDisc(formValues);
  });
}

function getEmptyDriveDisc(id) {
  return {
    id: id || "",
    slot: 1,
    level: 1,
    set_id: "",
    main_stat_name: "ATK",
    main_stat_value: 0,
    sub_stat_1_name: null,
    sub_stat_1_value: null,
    sub_stat_2_name: null,
    sub_stat_2_value: null,
    sub_stat_3_name: null,
    sub_stat_3_value: null,
    sub_stat_4_name: null,
    sub_stat_4_value: null,
  };
}

// ═══════════════════════════════════════════════════════════
//  DISC SETS
// ═══════════════════════════════════════════════════════════

async function renderDiscSets(container) {
  let data = [];
  try {
    data = await getDiscSets();
  } catch (e) {
    container.innerHTML = `<p class="error">${t("editor.loadError")}</p>`;
    return;
  }

  const headerRow = document.createElement("div");
  headerRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;margin-bottom:12px";

  const title = document.createElement("h2");
  title.style.margin = "0";
  title.textContent = t("editor.nav.discSets");
  headerRow.appendChild(title);

  const addBtn = document.createElement("button");
  addBtn.className = "btn btn-primary";
  addBtn.textContent = t("editor.equipment.addBtn");
  addBtn.addEventListener("click", () => showDiscSetForm(null, container));
  headerRow.appendChild(addBtn);
  container.appendChild(headerRow);

  const table = createDataTable(
    [
      { key: "set_id", label: t("editor.equipment.colSetId") },
      { key: "name", label: t("editor.equipment.colName") },
      { key: "two_piece_description", label: t("editor.equipment.col2pc"), format: (v) => v || "-" },
      { key: "four_piece_description", label: t("editor.equipment.col4pc"), format: (v) => v || "-" },
    ],
    data,
    {
      onEdit: (row) => showDiscSetForm(row.set_id, container),
      onDelete: (row) => handleDelete("disc-set", row.set_id, row.name, container, renderDiscSets),
      emptyMessage: t("editor.equipment.empty"),
    }
  );
  container.appendChild(table);
}

function showDiscSetForm(setId, rootContainer) {
  const existing = document.getElementById("equip-edit-container");
  if (existing) existing.remove();
  showInlineForm(rootContainer, "disc-set", setId, getEmptyDiscSet(setId), async (formValues) => {
    await saveDiscSet(formValues);
  });
}

function getEmptyDiscSet(setId) {
  return {
    set_id: setId || "",
    name: "",
    two_piece_description: "",
    two_piece_buff_id: "",
    four_piece_description: "",
    four_piece_buff_id: "",
  };
}

// ═══════════════════════════════════════════════════════════
//  INLINE FORM GENERIC
// ═══════════════════════════════════════════════════════════

async function showInlineForm(rootContainer, type, id, defaults, saveFn) {
  const editContainer = document.createElement("div");
  editContainer.id = "equip-edit-container";
  editContainer.style.marginTop = "20px";
  rootContainer.appendChild(editContainer);

  let values = { ...defaults };
  let isEdit = !!id;
  let loadedData = null;

  if (isEdit) {
    try {
      if (type === "w-engine") {
        const all = await getWEngines();
        loadedData = all.find((w) => w.id === id);
      } else if (type === "drive-disc") {
        const all = await getDriveDiscs();
        loadedData = all.find((d) => d.id === id);
      } else if (type === "disc-set") {
        const all = await getDiscSets();
        loadedData = all.find((d) => d.set_id === id);
      }
      if (loadedData) values = { ...defaults, ...loadedData };
    } catch (e) {
      // Use defaults
    }
  }

  function renderForm() {
    editContainer.innerHTML = "";
    buildInlineFormContent(editContainer, rootContainer, type, values, isEdit, saveFn);
  }

  renderForm();
  editContainer._langHandler = () => renderForm();
  window.addEventListener("langchange", editContainer._langHandler);
}

function buildInlineFormContent(editContainer, rootContainer, type, values, isEdit, saveFn) {

  const formEl = document.createElement("div");
  formEl.className = "editor-form";

  const sectionTitle = document.createElement("h3");
  sectionTitle.textContent = isEdit
    ? t("editor.equipment.editTitle")
    : t("editor.equipment.addTitle");
  sectionTitle.style.marginBottom = "16px";
  formEl.appendChild(sectionTitle);

  // Build fields based on type
  let fields;
  if (type === "w-engine") {
    fields = buildWEngineFields(values, isEdit);
  } else if (type === "drive-disc") {
    fields = buildDriveDiscFields(values);
  } else if (type === "disc-set") {
    fields = buildDiscSetFields(values);
  }

  // For drive discs, we need custom sub-stat section
  if (type === "drive-disc") {
    formEl.appendChild(buildDriveDiscForm(values, editContainer, rootContainer, saveFn));
    editContainer.appendChild(formEl);
    editContainer.scrollIntoView({ behavior: "smooth", block: "start" });
    return;
  }

  // For w-engine, add passive_effects tag input
  if (type === "w-engine") {
    formEl.appendChild(buildWEngineForm(values, editContainer, rootContainer, saveFn));
    editContainer.appendChild(formEl);
    editContainer.scrollIntoView({ behavior: "smooth", block: "start" });
    return;
  }

  // For disc-sets, just use createForm
  const form = createForm(fields, values, {
    saveLabel: t("editor.save"),
    cancelLabel: t("editor.cancel"),
    onSave: async (state) => {
      try {
        await saveFn(state);
        editContainer.remove();
        showNotification(t("editor.equipment.saved"), "success");
        renderCurrentTab(rootContainer);
      } catch (e) {
        showNotification(t("editor.equipment.saveError") + ": " + e, "error");
      }
    },
    onCancel: () => editContainer.remove(),
  });
  formEl.appendChild(form);
  editContainer.appendChild(formEl);
  editContainer.scrollIntoView({ behavior: "smooth", block: "start" });
}

function buildWEngineFields(values, isEdit) {
  return [
    { key: "id", label: t("editor.equipment.fieldId"), type: "text", readonly: isEdit },
    { key: "name", label: t("editor.equipment.fieldName"), type: "text" },
    { key: "level", label: t("editor.equipment.fieldLevel"), type: "number", min: 1, max: 60 },
    { key: "ascension", label: t("editor.equipment.fieldAscension"), type: "number", min: 0, max: 6 },
    { key: "atk", label: "ATK", type: "number", step: 0.1 },
    { key: "crit_rate", label: t("editor.equipment.fieldCritRate"), type: "number", step: 0.001 },
    { key: "crit_dmg", label: t("editor.equipment.fieldCritDmg"), type: "number", step: 0.001 },
    { key: "pen_ratio", label: t("editor.equipment.fieldPenRatio"), type: "number", step: 0.001 },
    { key: "energy_regen", label: t("editor.equipment.fieldEnergyRegen"), type: "number", step: 0.01 },
    { key: "impact", label: t("editor.equipment.fieldImpact"), type: "number", step: 0.1 },
    { key: "anomaly_mastery", label: t("editor.equipment.fieldAnomalyMastery"), type: "number", step: 0.1 },
  ];
}

function buildDriveDiscFields(values) {
  const slotOptions = SLOTS.map((s) => ({ value: String(s), label: `Slot ${s}` }));
  const statOptions = STAT_OPTIONS.map((s) => ({ value: s, label: s }));
  return [
    { key: "id", label: t("editor.equipment.fieldId"), type: "text" },
    { key: "slot", label: t("editor.equipment.fieldSlot"), type: "select", options: slotOptions },
    { key: "level", label: t("editor.equipment.fieldLevel"), type: "number", min: 0, max: 15 },
    { key: "set_id", label: t("editor.equipment.fieldSetId"), type: "text" },
    { key: "main_stat_name", label: t("editor.equipment.fieldMainStat"), type: "select", options: statOptions },
    { key: "main_stat_value", label: t("editor.equipment.fieldMainVal"), type: "number", step: 0.1 },
  ];
}

function buildDiscSetFields(values) {
  return [
    { key: "set_id", label: t("editor.equipment.fieldSetId"), type: "text", readonly: !!values.set_id },
    { key: "name", label: t("editor.equipment.fieldName"), type: "text" },
    { key: "two_piece_description", label: t("editor.equipment.field2pcDesc"), type: "textarea" },
    { key: "two_piece_buff_id", label: t("editor.equipment.field2pcBuff"), type: "text" },
    { key: "four_piece_description", label: t("editor.equipment.field4pcDesc"), type: "textarea" },
    { key: "four_piece_buff_id", label: t("editor.equipment.field4pcBuff"), type: "text" },
  ];
}

// ── W-Engine custom form ────────────────────────────────

function buildWEngineForm(values, editContainer, rootContainer, saveFn) {
  const fields = buildWEngineFields(values, !!values.id);
  const form = createForm(fields, values, {});

  // Remove form buttons (we add our own)
  const formBtns = form.querySelector(".form-buttons");
  if (formBtns) formBtns.remove();

  // Passive effects section
  const passiveSection = document.createElement("div");
  passiveSection.style.marginTop = "16px";

  const passiveLabel = document.createElement("div");
  passiveLabel.className = "form-label";
  passiveLabel.textContent = t("editor.equipment.fieldPassiveEffects");
  passiveSection.appendChild(passiveLabel);

  const tagWrapper = document.createElement("div");
  tagWrapper.className = "tag-input-wrapper";
  tagWrapper.style.marginTop = "8px";

  const tagContainer = document.createElement("div");
  tagContainer.className = "tag-container";

  let passiveEffects = [];
  try {
    const parsed = JSON.parse(values.passive_effects || "[]");
    passiveEffects = Array.isArray(parsed) ? parsed : [];
  } catch (e) {
    passiveEffects = [];
  }

  function renderTags() {
    tagContainer.innerHTML = "";
    for (const tag of passiveEffects) {
      const tagEl = document.createElement("span");
      tagEl.className = "tag-item";
      tagEl.textContent = tag;
      const removeBtn = document.createElement("button");
      removeBtn.type = "button";
      removeBtn.className = "tag-remove";
      removeBtn.textContent = "×";
      removeBtn.addEventListener("click", () => {
        const idx = passiveEffects.indexOf(tag);
        if (idx !== -1) passiveEffects.splice(idx, 1);
        renderTags();
      });
      tagEl.appendChild(removeBtn);
      tagContainer.appendChild(tagEl);
    }
  }

  const tagInput = document.createElement("input");
  tagInput.type = "text";
  tagInput.className = "form-input tag-text-input";
  tagInput.placeholder = t("editor.equipment.passivePlaceholder");
  tagInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && tagInput.value.trim()) {
      e.preventDefault();
      passiveEffects.push(tagInput.value.trim());
      tagInput.value = "";
      renderTags();
    }
  });

  renderTags();
  tagWrapper.appendChild(tagContainer);
  tagWrapper.appendChild(tagInput);
  passiveSection.appendChild(tagWrapper);

  // Buttons
  const btnRow = document.createElement("div");
  btnRow.className = "form-buttons";

  const btnSave = document.createElement("button");
  btnSave.className = "btn btn-primary";
  btnSave.textContent = t("editor.save");
  btnSave.addEventListener("click", async () => {
    // Read form state
    const state = { ...values };
    form.querySelectorAll(".form-input").forEach((input) => {
      const labelEl = input.closest(".form-field")?.querySelector(".form-label");
      if (!labelEl) return;
      const label = labelEl.textContent;
      for (const f of fields) {
        const fieldLabel = form.querySelector(`.form-label`)?.textContent;
        // Simple match by known keys
        if (f.label === label) {
          state[f.key] = input.type === "number" ? parseFloat(input.value) || 0 : input.value;
          break;
        }
      }
    });
    state.passive_effects = JSON.stringify(passiveEffects);
    try {
      await saveFn(state);
      editContainer.remove();
      showNotification(t("editor.equipment.saved"), "success");
      renderCurrentTab(rootContainer);
    } catch (e) {
      showNotification(t("editor.equipment.saveError") + ": " + e, "error");
    }
  });
  btnRow.appendChild(btnSave);

  const btnCancel = document.createElement("button");
  btnCancel.className = "btn btn-secondary";
  btnCancel.textContent = t("editor.cancel");
  btnCancel.addEventListener("click", () => editContainer.remove());
  btnRow.appendChild(btnCancel);

  passiveSection.appendChild(btnRow);
  form.appendChild(passiveSection);
  return form;
}

// ── Drive Disc custom form ──────────────────────────────

function buildDriveDiscForm(values, editContainer, rootContainer, saveFn) {
  const container = document.createElement("div");
  container.className = "editor-form";

  const fields = buildDriveDiscFields(values);
  const form = createForm(fields, values, {});
  const formBtns = form.querySelector(".form-buttons");
  if (formBtns) formBtns.remove();

  container.appendChild(form);

  // Sub stats section
  const subSection = document.createElement("div");
  subSection.style.marginTop = "16px";
  subSection.style.paddingTop = "16px";
  subSection.style.borderTop = "1px solid var(--border)";

  const subTitle = document.createElement("div");
  subTitle.className = "form-label";
  subTitle.textContent = t("editor.equipment.fieldSubStats");
  subSection.appendChild(subTitle);

  const statOptions = STAT_OPTIONS.map((s) => ({ value: s, label: s }));

  for (let i = 1; i <= 4; i++) {
    const row = document.createElement("div");
    row.style.cssText = "display:flex;gap:8px;margin-top:8px;align-items:center";

    const nameSelect = document.createElement("select");
    nameSelect.className = "form-input form-select";
    nameSelect.style.flex = "1";
    const blankOpt = document.createElement("option");
    blankOpt.value = "";
    blankOpt.textContent = "-";
    nameSelect.appendChild(blankOpt);
    for (const opt of statOptions) {
      const el = document.createElement("option");
      el.value = opt.value;
      el.textContent = opt.label;
      const key = `sub_stat_${i}_name`;
      if (values[key] === opt.value) el.selected = true;
      nameSelect.appendChild(el);
    }
    nameSelect.addEventListener("change", () => {
      values[`sub_stat_${i}_name`] = nameSelect.value || null;
    });
    row.appendChild(nameSelect);

    const valInput = document.createElement("input");
    valInput.type = "number";
    valInput.className = "form-input";
    valInput.step = 0.1;
    valInput.style.width = "120px";
    valInput.placeholder = "Value";
    const valKey = `sub_stat_${i}_value`;
    valInput.value = values[valKey] ?? "";
    valInput.addEventListener("input", () => {
      values[valKey] = valInput.value === "" ? null : parseFloat(valInput.value);
    });
    row.appendChild(valInput);

    subSection.appendChild(row);
  }

  container.appendChild(subSection);

  // Buttons
  const btnRow = document.createElement("div");
  btnRow.className = "form-buttons";

  const btnSave = document.createElement("button");
  btnSave.className = "btn btn-primary";
  btnSave.textContent = t("editor.save");
  btnSave.addEventListener("click", async () => {
    try {
      await saveFn(values);
      editContainer.remove();
      showNotification(t("editor.equipment.saved"), "success");
      renderCurrentTab(rootContainer);
    } catch (e) {
      showNotification(t("editor.equipment.saveError") + ": " + e, "error");
    }
  });
  btnRow.appendChild(btnSave);

  const btnCancel = document.createElement("button");
  btnCancel.className = "btn btn-secondary";
  btnCancel.textContent = t("editor.cancel");
  btnCancel.addEventListener("click", () => editContainer.remove());
  btnRow.appendChild(btnCancel);

  container.appendChild(btnRow);
  return container;
}

// ═══════════════════════════════════════════════════════════
//  HELPERS
// ═══════════════════════════════════════════════════════════

function getCurrentRoute() {
  const hash = window.location.hash.replace(/^#\//, "");
  const parts = hash.split("/");
  return parts[parts.length - 1] || "w-engines";
}

function renderCurrentTab(container) {
  const route = getCurrentRoute();
  container.innerHTML = "";
  switch (route) {
    case "w-engines": renderWEngines(container); break;
    case "drive-discs": renderDriveDiscs(container); break;
    case "disc-sets": renderDiscSets(container); break;
  }
}

async function handleDelete(type, id, name, rootContainer, refreshFn) {
  const ok = await confirm(t("editor.equipment.confirmDelete", { name }));
  if (!ok) return;
  try {
    if (type === "w-engine") await deleteWEngine(id);
    else if (type === "drive-disc") await deleteDriveDisc(id);
    else if (type === "disc-set") await deleteDiscSet(id);
    showNotification(t("editor.equipment.deleted"), "success");
    rootContainer.innerHTML = "";
    refreshFn(rootContainer);
  } catch (e) {
    showNotification(t("editor.equipment.deleteError") + ": " + e, "error");
  }
}

function showNotification(msg, type = "info") {
  const existing = document.querySelector(".editor-notification");
  if (existing) existing.remove();
  const el = document.createElement("div");
  el.className = "editor-notification " + type;
  el.textContent = msg;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}
