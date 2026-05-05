import { t } from "../../i18n.js";
import { createDataTable } from "../components/datatable.js";
import { confirm } from "../components/confirm.js";
import {
  getCharacters,
  getCharacter,
  saveCharacter,
  deleteCharacter,
} from "../utils/api.js";
import { formatStat } from "../utils/format.js";
import {
  validateCharId, validateLevel, validateAscension,
  validateCritRate, validateNonNegative, validate
} from "../utils/validation.js";

// ── Field options ───────────────────────────────────────

const FACTION_OPTIONS = [
  "Belobog Industries",
  "Cunning_Hares",
  "Gentle_House",
  "Section_6",
  "NEPS",
  "Obol_Squad",
  "Soldiers_11th",
  "Stars_of_Lyra",
  "Mockingbird",
  "Criminal_Investigation_Special_Response_Team",
  "Sons_of_Calydon",
];

const SPECIALTY_OPTIONS = ["Attack", "Stun", "Support", "Defense", "Anomaly"];
const ELEMENT_OPTIONS = ["Physical", "Fire", "Ice", "Electric", "Ether"];

// ── State ───────────────────────────────────────────────

let allCharacters = [];
let filterFaction = "";
let filterSpecialty = "";
let filterElement = "";

// ── Helpers ─────────────────────────────────────────────

function parseCinemas(raw) {
  if (!raw) return [false, false, false, false, false, false];
  try {
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return [false, false, false, false, false, false];
    if (typeof arr[0] === "string") {
      const keys = ["cinema1","cinema2","cinema3","cinema4","cinema5","cinema6"];
      return keys.map((k) => arr.includes(k));
    }
    return arr.map(Boolean);
  } catch {
    return [false, false, false, false, false, false];
  }
}

function parsePotentials(raw) {
  if (!raw) return [false, false, false, false, false, false];
  try {
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return [false, false, false, false, false, false];
    return arr.map(Boolean);
  } catch {
    return [false, false, false, false, false, false];
  }
}

function stringifyCinemas(bools) {
  const keys = ["cinema1","cinema2","cinema3","cinema4","cinema5","cinema6"];
  const active = keys.filter((_, i) => bools[i]);
  return JSON.stringify(active);
}

function stringifyPotentials(bools) {
  return JSON.stringify(bools);
}

function parseActionDict(raw) {
  if (!raw) return [];
  try {
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? arr : [];
  } catch {
    return [];
  }
}

// ── Render ──────────────────────────────────────────────

export function renderPage() {
  const container = document.createElement("div");

  // Title + Add button row
  const headerRow = document.createElement("div");
  headerRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;margin-bottom:16px;";

  const heading = document.createElement("h2");
  heading.style.margin = "0";
  heading.textContent = t("editor.nav.characters");
  headerRow.appendChild(heading);

  const addBtn = document.createElement("button");
  addBtn.className = "btn btn-primary";
  addBtn.textContent = t("editor.characters.addBtn");
  addBtn.addEventListener("click", () => showEditForm(null, container));
  headerRow.appendChild(addBtn);
  container.appendChild(headerRow);

  // Filter bar
  const filterBar = document.createElement("div");
  filterBar.className = "filter-bar";

  const factionSelect = createFilterSelect(FACTION_OPTIONS, "editor.characters.filterFaction", (v) => {
    filterFaction = v; refreshTable(container);
  });
  const specialtySelect = createFilterSelect(SPECIALTY_OPTIONS, "editor.characters.filterSpecialty", (v) => {
    filterSpecialty = v; refreshTable(container);
  });
  const elementSelect = createFilterSelect(ELEMENT_OPTIONS, "editor.characters.filterElement", (v) => {
    filterElement = v; refreshTable(container);
  });

  filterBar.appendChild(factionSelect);
  filterBar.appendChild(specialtySelect);
  filterBar.appendChild(elementSelect);
  container.appendChild(filterBar);

  // Table
  const tableContainer = document.createElement("div");
  tableContainer.id = "char-table-container";
  container.appendChild(tableContainer);

  loadData(tableContainer, container);
  return container;
}

function createFilterSelect(options, labelKey, onChange) {
  const select = document.createElement("select");
  const allOpt = document.createElement("option");
  allOpt.value = "";
  allOpt.textContent = t(labelKey);
  select.appendChild(allOpt);
  for (const opt of options) {
    const el = document.createElement("option");
    el.value = opt;
    el.textContent = opt;
    select.appendChild(el);
  }
  select.addEventListener("change", () => onChange(select.value));
  return select;
}

async function loadData(tableContainer, rootContainer) {
  try {
    allCharacters = await getCharacters();
  } catch (e) {
    allCharacters = [];
  }
  refreshTable(tableContainer, rootContainer);
}

function refreshTable(tableContainer, rootContainer) {
  const filtered = allCharacters.filter((c) => {
    if (filterFaction && c.faction !== filterFaction) return false;
    if (filterSpecialty && c.specialty !== filterSpecialty) return false;
    if (filterElement && c.element !== filterElement) return false;
    return true;
  });

  const columns = [
    { key: "char_id", label: t("editor.characters.colId") },
    { key: "name", label: t("editor.characters.colName") },
    { key: "faction", label: t("editor.characters.colFaction") },
    { key: "specialty", label: t("editor.characters.colSpecialty") },
    { key: "element", label: t("editor.characters.colElement") },
    { key: "level", label: t("editor.characters.colLevel") },
    { key: "atk", label: "ATK", format: formatStat },
    { key: "hp", label: "HP", format: formatStat },
    { key: "def", label: "DEF", format: formatStat },
  ];

  const table = createDataTable(columns, filtered, {
    onEdit: (row) => showEditForm(row.char_id, rootContainer),
    onDelete: (row) => handleDelete(row.char_id, row.name, rootContainer),
  });

  tableContainer.innerHTML = "";
  tableContainer.appendChild(table);
}

async function handleDelete(charId, name, rootContainer) {
  const ok = await confirm(t("editor.characters.confirmDelete", { name }));
  if (!ok) return;
  try {
    await deleteCharacter(charId);
    showNotification(t("editor.characters.deleted", { name }), "success");
    allCharacters = await getCharacters();
    const tc = document.getElementById("char-table-container");
    if (tc) refreshTable(tc, rootContainer);
  } catch (e) {
    showNotification(t("editor.characters.deleteError") + ": " + e, "error");
  }
}

// ── Edit Form ───────────────────────────────────────────

async function showEditForm(charId, rootContainer) {
  // Remove existing edit panel
  const existing = document.getElementById("char-edit-container");
  if (existing) existing.remove();

  let data;
  if (charId) {
    try {
      data = await getCharacter(charId);
    } catch (e) {
      showNotification(t("editor.characters.loadError") + ": " + e, "error");
      return;
    }
  } else {
    data = getEmptyRecord();
  }

  const editContainer = document.createElement("div");
  editContainer.id = "char-edit-container";
  editContainer.style.marginTop = "20px";
  rootContainer.appendChild(editContainer);

  const constellations = parseCinemas(data.constellations);
  const potentials = parsePotentials(data.potentials);
  const actionDict = parseActionDict(data.action_dict);

  function renderForm() {
    editContainer.innerHTML = "";
    buildFormContent(editContainer, data, charId, constellations, potentials, actionDict, rootContainer);
  }

  renderForm();
  editContainer._langHandler = () => renderForm();
  window.addEventListener("langchange", editContainer._langHandler);
}

function buildFormContent(editContainer, data, charId, constellations, potentials, actionDict, rootContainer) {

  // ── Build form manually for full control ──────────────

  const formEl = document.createElement("div");
  formEl.className = "editor-form";

  const sectionTitle = document.createElement("h3");
  sectionTitle.textContent = charId
    ? t("editor.characters.editTitle")
    : t("editor.characters.addTitle");
  sectionTitle.style.marginBottom = "16px";
  formEl.appendChild(sectionTitle);

  const fieldsWrapper = document.createElement("div");
  formEl.appendChild(fieldsWrapper);

  const allInputs = {};
  const fields = [];

  function addField(key, label, type, extra = {}) {
    const group = document.createElement("div");
    group.className = "form-field";

    const lbl = document.createElement("label");
    lbl.className = "form-label";
    lbl.textContent = label;
    group.appendChild(lbl);

    let input;
    if (type === "select") {
      input = document.createElement("select");
      input.className = "form-input form-select";
      if (extra.options) {
        for (const opt of extra.options) {
          const el = document.createElement("option");
          el.value = opt;
          el.textContent = opt;
          if (opt === data[key] || opt === data[key]) el.selected = true;
          input.appendChild(el);
        }
      }
      input.addEventListener("change", () => { data[key] = input.value; });
    } else {
      input = document.createElement("input");
      input.type = type === "number" ? "number" : "text";
      input.className = "form-input";
      if (type === "number") {
        if (extra.step !== undefined) input.step = extra.step;
        if (extra.min !== undefined) input.min = extra.min;
        if (extra.max !== undefined) input.max = extra.max;
      }
      if (extra.readonly) input.readOnly = true;

      if (type === "number") {
        input.value = data[key] ?? 0;
        input.addEventListener("input", () => {
          data[key] = input.value === "" ? 0 : parseFloat(input.value);
        });
      } else {
        input.value = data[key] ?? "";
        input.addEventListener("input", () => { data[key] = input.value; });
      }
    }

    group.appendChild(input);
    allInputs[key] = input;

    // Track for state collection
    fields.push({ key, type, extra });
    return group;
  }

  // Section 1: Basic Info
  const basicGrid = document.createElement("div");
  basicGrid.className = "form-fields";

  basicGrid.appendChild(addField("char_id", t("editor.characters.fieldCharId"), "text", { readonly: !!charId }));
  basicGrid.appendChild(addField("name", t("editor.characters.fieldName"), "text"));
  basicGrid.appendChild(addField("faction", t("editor.characters.fieldFaction"), "select", { options: FACTION_OPTIONS }));
  basicGrid.appendChild(addField("specialty", t("editor.characters.fieldSpecialty"), "select", { options: SPECIALTY_OPTIONS }));
  basicGrid.appendChild(addField("element", t("editor.characters.fieldElement"), "select", { options: ELEMENT_OPTIONS }));
  basicGrid.appendChild(addField("level", t("editor.characters.fieldLevel"), "number", { min: 1, max: 60 }));
  basicGrid.appendChild(addField("ascension", t("editor.characters.fieldAscension"), "number", { min: 0, max: 6 }));

  fieldsWrapper.appendChild(basicGrid);

  // Section 2: Base Stats
  const statGrid = document.createElement("div");
  statGrid.className = "form-fields";
  statGrid.style.marginTop = "16px";

  const statFields = [
    ["hp", "HP", 1], ["atk", "ATK", 1], ["def", "DEF", 1],
    ["impact", "Impact", 1],
    ["crit_rate", "Crit Rate", 0.01], ["crit_dmg", t("editor.characters.fieldExtraCritDmg"), 0.01],
    ["pen_ratio", "PEN Ratio", 0.01], ["pen_fixed", "PEN Fixed", 1],
    ["anomaly_mastery", "Anomaly Mastery", 1],
    ["anomaly_proficiency", "Anomaly Proficiency", 1],
    ["energy_regen", "Energy Regen", "any"],
    ["energy_gen_rate", "Energy Gen Rate", "any"],
  ];
  for (const [key, label, step] of statFields) {
    statGrid.appendChild(addField(key, label, "number", { step }));
  }

  fieldsWrapper.appendChild(statGrid);

  // Section 3: Cinema toggles (sequential)
  function createToggleSection(sectionLabel, stateArray, btnPrefix, btnRefsArray) {
    const section = document.createElement("div");
    section.style.marginTop = "16px";
    const label = document.createElement("div");
    label.className = "form-label";
    label.textContent = sectionLabel;
    section.appendChild(label);

    const row = document.createElement("div");
    row.style.cssText = "display:flex;gap:8px;margin-top:8px";
    for (let i = 0; i < 6; i++) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.textContent = btnPrefix + (i + 1);
      btn.style.cssText = `
        padding:8px 16px;border-radius:6px;border:1px solid var(--border);
        cursor:pointer;font-family:var(--font);font-size:13px;font-weight:600;
        background:${stateArray[i] ? "var(--primary)" : "var(--surface)"};
        color:${stateArray[i] ? "#fff" : "var(--text)"};
        transition:background 0.15s;
      `;
      btn.addEventListener("click", () => {
        const wasActive = stateArray[i];
        if (!wasActive) {
          // Enforce sequential: can only enable if all previous are enabled
          if (i > 0 && !stateArray[i - 1]) return;
        } else {
          // Cascade disable: turn off this and all subsequent
          for (let j = i; j < 6; j++) stateArray[j] = false;
        }
        if (!wasActive) stateArray[i] = true;
        // Refresh button states
        for (let k = 0; k < 6; k++) {
          btnRefsArray[k].style.background = stateArray[k] ? "var(--primary)" : "var(--surface)";
          btnRefsArray[k].style.color = stateArray[k] ? "#fff" : "var(--text)";
        }
      });
      row.appendChild(btn);
      btnRefsArray[i] = btn;
    }
    section.appendChild(row);
    return section;
  }

  const cinemaBtns = [];
  fieldsWrapper.appendChild(createToggleSection(
    t("editor.characters.sectionCinema"), constellations, "C", cinemaBtns
  ));

  // Section 4: Potential toggles (sequential)
  const potentialBtns = [];
  fieldsWrapper.appendChild(createToggleSection(
    t("editor.characters.sectionPotential"), potentials, "P", potentialBtns
  ));

  // Section 5: Action dict tag input
  const actionSection = document.createElement("div");
  actionSection.style.marginTop = "16px";

  const actionLabel = document.createElement("div");
  actionLabel.className = "form-label";
  actionLabel.textContent = t("editor.characters.sectionActions");
  actionSection.appendChild(actionLabel);

  const tagWrapper = document.createElement("div");
  tagWrapper.className = "tag-input-wrapper";
  tagWrapper.style.marginTop = "8px";

  const tagContainer = document.createElement("div");
  tagContainer.className = "tag-container";

  function renderActionTags() {
    tagContainer.innerHTML = "";
    for (const tag of actionDict) {
      const tagEl = document.createElement("span");
      tagEl.className = "tag-item";
      tagEl.textContent = tag;
      const removeBtn = document.createElement("button");
      removeBtn.type = "button";
      removeBtn.className = "tag-remove";
      removeBtn.textContent = "×";
      removeBtn.addEventListener("click", () => {
        const idx = actionDict.indexOf(tag);
        if (idx !== -1) actionDict.splice(idx, 1);
        renderActionTags();
      });
      tagEl.appendChild(removeBtn);
      tagContainer.appendChild(tagEl);
    }
  }

  const actionInput = document.createElement("input");
  actionInput.type = "text";
  actionInput.className = "form-input tag-text-input";
  actionInput.placeholder = t("editor.characters.actionPlaceholder");
  actionInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && actionInput.value.trim()) {
      e.preventDefault();
      actionDict.push(actionInput.value.trim());
      actionInput.value = "";
      renderActionTags();
    }
  });

  renderActionTags();
  tagWrapper.appendChild(tagContainer);
  tagWrapper.appendChild(actionInput);
  actionSection.appendChild(tagWrapper);
  fieldsWrapper.appendChild(actionSection);

  // Buttons
  const btnRow = document.createElement("div");
  btnRow.className = "form-buttons";

  const saveBtn = document.createElement("button");
  saveBtn.className = "btn btn-primary";
  saveBtn.textContent = t("editor.action.save");
  saveBtn.addEventListener("click", async () => {
    // Validate required fields
    const errors = [];
    if (!charId && validate(data.char_id, [validateCharId])) {
      errors.push("Character ID is required");
    }
    if (!data.name) errors.push("Name is required");
    if (!data.faction) errors.push("Faction is required");
    if (!data.specialty) errors.push("Specialty is required");
    if (!data.element) errors.push("Element is required");

    // Validate numeric fields
    const lvlErr = validate(data.level, [validateLevel]);
    if (lvlErr) errors.push("Level: " + lvlErr);

    const ascErr = validate(data.ascension, [validateAscension]);
    if (ascErr) errors.push("Ascension: " + ascErr);

    const crErr = validate(data.crit_rate, [validateCritRate]);
    if (crErr) errors.push("Crit Rate: " + crErr);

    const cdErr = validate(data.crit_dmg, [validateCritRate]);
    if (cdErr) errors.push("Crit DMG: " + cdErr);

    const prErr = validate(data.pen_ratio, [validateCritRate]);
    if (prErr) errors.push("PEN Ratio: " + prErr);

    for (const field of ["hp", "atk", "def", "impact", "pen_fixed", "anomaly_mastery", "anomaly_proficiency", "energy_regen", "energy_gen_rate"]) {
      const err = validate(data[field], [v => validateNonNegative(v, field)]);
      if (err) errors.push(err);
    }

    if (errors.length > 0) {
      showNotification(errors.join("; "), "error");
      return;
    }

    data.constellations = stringifyCinemas(constellations);
    data.potentials = stringifyPotentials(potentials);
    data.action_dict = JSON.stringify(actionDict);

    try {
      await saveCharacter(data);
      editContainer.remove();
      showNotification(t("editor.characters.saved", { name: data.name }), "success");
      allCharacters = await getCharacters();
      const tc = document.getElementById("char-table-container");
      if (tc) refreshTable(tc, rootContainer);
    } catch (e) {
      showNotification(t("editor.characters.saveError") + ": " + e, "error");
    }
  });

  const cancelBtn = document.createElement("button");
  cancelBtn.className = "btn btn-secondary";
  cancelBtn.textContent = t("editor.action.cancel");
  cancelBtn.addEventListener("click", () => editContainer.remove());

  btnRow.appendChild(saveBtn);
  btnRow.appendChild(cancelBtn);
  formEl.appendChild(btnRow);

  editContainer.appendChild(formEl);
  editContainer.scrollIntoView({ behavior: "smooth", block: "start" });
}

function getEmptyRecord() {
  return {
    char_id: "", name: "", faction: "", specialty: "", element: "",
    level: 1, ascension: 0,
    hp: 0, atk: 0, def: 0, impact: 0,
    crit_rate: 0, crit_dmg: 0, pen_ratio: 0, pen_fixed: 0,
    anomaly_mastery: 0, anomaly_proficiency: 0,
    energy_regen: 0, energy_gen_rate: 0,
    constellations: "[]",
    potentials: "[]",
    action_dict: "[]",
  };
}

// ── Notification helper ─────────────────────────────────

function showNotification(msg, type = "info") {
  const existing = document.querySelector(".editor-notification");
  if (existing) existing.remove();
  const el = document.createElement("div");
  el.className = "editor-notification " + type;
  el.textContent = msg;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}
