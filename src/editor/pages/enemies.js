import { t } from "../../i18n.js";
import { ENEMY_TYPE_MAP, ELEMENT_MAP } from "../utils/i18n-maps.js";
import { createDataTable } from "../components/datatable.js";
import { createForm } from "../components/form.js";
import { confirm } from "../components/confirm.js";
import { getEnemies, getEnemy, saveEnemy, deleteEnemy } from "../utils/api.js";
import { formatStat, formatFixed } from "../utils/format.js";

const ENEMY_TYPES = ["Normal", "Elite", "Boss"];
const ELEMENTS = ["Physical", "Fire", "Ice", "Electric", "Ether"];

// ── Render ──────────────────────────────────────────────

export function renderPage() {
  const container = document.createElement("div");
  renderEnemyList(container);
  return container;
}

async function renderEnemyList(container) {
  let data = [];
  try {
    data = await getEnemies();
  } catch (e) {
    container.innerHTML = `<p class="error">${t("editor.loadError")}</p>`;
    return;
  }

  const headerRow = document.createElement("div");
  headerRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;margin-bottom:12px";

  const title = document.createElement("h2");
  title.style.margin = "0";
  title.textContent = t("editor.nav.enemies");
  headerRow.appendChild(title);

  const addBtn = document.createElement("button");
  addBtn.className = "btn btn-primary";
  addBtn.textContent = t("editor.enemies.addBtn");
  addBtn.addEventListener("click", () => showEnemyForm(null, container));
  headerRow.appendChild(addBtn);
  container.appendChild(headerRow);

  const table = createDataTable(
    [
      { key: "enemy_id", label: t("editor.enemies.colId") },
      { key: "name", label: t("editor.enemies.colName") },
      { key: "enemy_type", label: t("editor.enemies.colType"), format: (v) => t(ENEMY_TYPE_MAP[v]) },
      { key: "level", label: t("editor.enemies.colLevel") },
      { key: "hp", label: t("editor.enemies.colHp"), format: formatStat },
      { key: "def", label: t("editor.enemies.colDef"), format: formatStat },
      { key: "base_res", label: t("editor.enemies.colRes"), format: (v) => formatFixed(v, 3) },
    ],
    data,
    {
      onEdit: (row) => showEnemyForm(row.enemy_id, container),
      onDelete: (row) => handleDelete(row.enemy_id, row.name, container),
      emptyMessage: t("editor.enemies.empty"),
    }
  );
  container.appendChild(table);
}

// ── Enemy Form ──────────────────────────────────────────

async function showEnemyForm(enemyId, rootContainer) {
  const existing = document.getElementById("enemy-edit-container");
  if (existing) existing.remove();

  const editContainer = document.createElement("div");
  editContainer.id = "enemy-edit-container";
  editContainer.style.marginTop = "20px";
  rootContainer.appendChild(editContainer);

  let values = getEmptyEnemy(enemyId);
  const isEdit = !!enemyId;

  if (isEdit) {
    try {
      const full = await getEnemy(enemyId);
      if (full) values = { ...values, ...full };
    } catch (e) { /* use defaults */ }
  }

  // Parse JSON fields
  let resistances = {};
  let weaknesses = [];
  try {
    resistances = typeof values.resistances === "string"
      ? JSON.parse(values.resistances)
      : values.resistances || {};
  } catch (e) { resistances = {}; }
  try {
    weaknesses = typeof values.weaknesses === "string"
      ? JSON.parse(values.weaknesses)
      : values.weaknesses || [];
  } catch (e) { weaknesses = []; }

  const formEl = document.createElement("div");
  formEl.className = "editor-form";

  const sectionTitle = document.createElement("h3");
  sectionTitle.textContent = isEdit
    ? t("editor.enemies.editTitle")
    : t("editor.enemies.addTitle");
  sectionTitle.style.marginBottom = "16px";
  formEl.appendChild(sectionTitle);

  const typeOptions = ENEMY_TYPES.map((t) => ({ value: t, label: t(ENEMY_TYPE_MAP[t]) }));
  const fields = [
    { key: "enemy_id", label: t("editor.enemies.fieldEnemyId"), type: "text", readonly: isEdit },
    { key: "name", label: t("editor.enemies.fieldName"), type: "text" },
    { key: "enemy_type", label: t("editor.enemies.fieldType"), type: "select", options: typeOptions },
    { key: "level", label: t("editor.enemies.fieldLevel"), type: "number", min: 1, max: 99 },
    { key: "hp", label: t("editor.enemies.fieldHp"), type: "number", step: 1 },
    { key: "def", label: t("editor.enemies.fieldDef"), type: "number", step: 1 },
    { key: "base_res", label: t("editor.enemies.fieldBaseRes"), type: "number", step: 0.001 },
    { key: "daze_max", label: t("editor.enemies.fieldDazeMax"), type: "number", step: 1 },
  ];

  const form = createForm(fields, values, {});
  const formBtns = form.querySelector(".form-buttons");
  if (formBtns) formBtns.remove();
  formEl.appendChild(form);

  // ── Resistance rows ──────────────────────────────────
  const resistSection = document.createElement("div");
  resistSection.style.marginTop = "16px";
  resistSection.style.paddingTop = "16px";
  resistSection.style.borderTop = "1px solid var(--border)";

  const resistTitle = document.createElement("div");
  resistTitle.className = "form-label";
  resistTitle.textContent = t("editor.enemies.sectionResistances");
  resistSection.appendChild(resistTitle);

  const resistContainer = document.createElement("div");
  resistContainer.style.marginTop = "8px";

  function renderResistRows() {
    resistContainer.innerHTML = "";
    for (const [element, value] of Object.entries(resistances)) {
      const row = document.createElement("div");
      row.style.cssText = "display:flex;gap:8px;margin-top:4px;align-items:center";

      const currentElement = element;
      const elemSelect = document.createElement("select");
      elemSelect.className = "form-input form-select";
      elemSelect.style.flex = "1";
      for (const el of ELEMENTS) {
        const opt = document.createElement("option");
        opt.value = el;
        opt.textContent = t(ELEMENT_MAP[el]);
        if (el === currentElement) opt.selected = true;
        elemSelect.appendChild(opt);
      }
      elemSelect.addEventListener("change", () => {
        const val = resistances[currentElement];
        delete resistances[currentElement];
        resistances[elemSelect.value] = val;
      });
      row.appendChild(elemSelect);

      const valInput = document.createElement("input");
      valInput.type = "number";
      valInput.className = "form-input";
      valInput.step = 0.1;
      valInput.style.width = "100px";
      valInput.value = String(value);
      valInput.addEventListener("input", () => {
        resistances[elemSelect.value] = parseFloat(valInput.value) || 0;
      });
      row.appendChild(valInput);

      const delBtn = document.createElement("button");
      delBtn.className = "btn btn-danger btn-sm del-resist-btn";
      delBtn.textContent = "×";
      delBtn.addEventListener("click", () => {
        delete resistances[currentElement];
        renderResistRows();
      });
      row.appendChild(delBtn);

      resistContainer.appendChild(row);
    }
  }

  renderResistRows();
  resistSection.appendChild(resistContainer);

  const addResistBtn = document.createElement("button");
  addResistBtn.className = "btn btn-secondary";
  addResistBtn.style.marginTop = "8px";
  addResistBtn.textContent = t("editor.enemies.addResistBtn");
  addResistBtn.addEventListener("click", () => {
    const unused = ELEMENTS.find((el) => !(el in resistances));
    if (unused) resistances[unused] = 0;
    else resistances["Ether"] = 0;
    renderResistRows();
  });
  resistSection.appendChild(addResistBtn);

  formEl.appendChild(resistSection);

  // ── Weakness tags ────────────────────────────────────
  const weakSection = document.createElement("div");
  weakSection.style.marginTop = "16px";
  weakSection.style.paddingTop = "16px";
  weakSection.style.borderTop = "1px solid var(--border)";

  const weakLabel = document.createElement("div");
  weakLabel.className = "form-label";
  weakLabel.textContent = t("editor.enemies.sectionWeaknesses");
  weakSection.appendChild(weakLabel);

  const tagWrapper = document.createElement("div");
  tagWrapper.className = "tag-input-wrapper";
  tagWrapper.style.marginTop = "8px";

  const tagContainer = document.createElement("div");
  tagContainer.className = "tag-container";

  function renderWeaknessTags() {
    tagContainer.innerHTML = "";
    for (const tag of weaknesses) {
      const tagEl = document.createElement("span");
      tagEl.className = "tag-item";
      tagEl.textContent = ELEMENT_MAP[tag] ? t(ELEMENT_MAP[tag]) : tag;
      const removeBtn = document.createElement("button");
      removeBtn.type = "button";
      removeBtn.className = "tag-remove";
      removeBtn.textContent = "×";
      removeBtn.addEventListener("click", () => {
        const idx = weaknesses.indexOf(tag);
        if (idx !== -1) weaknesses.splice(idx, 1);
        renderWeaknessTags();
      });
      tagEl.appendChild(removeBtn);
      tagContainer.appendChild(tagEl);
    }
  }

  const tagInput = document.createElement("input");
  tagInput.type = "text";
  tagInput.className = "form-input tag-text-input";
  tagInput.placeholder = t("editor.enemies.weaknessPlaceholder");
  tagInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && tagInput.value.trim()) {
      e.preventDefault();
      weaknesses.push(tagInput.value.trim());
      tagInput.value = "";
      renderWeaknessTags();
    }
  });

  renderWeaknessTags();
  tagWrapper.appendChild(tagContainer);
  tagWrapper.appendChild(tagInput);
  weakSection.appendChild(tagWrapper);

  formEl.appendChild(weakSection);

  // ── Buttons ──────────────────────────────────────────
  const btnRow = document.createElement("div");
  btnRow.className = "form-buttons";

  const btnSave = document.createElement("button");
  btnSave.className = "btn btn-primary";
  btnSave.textContent = t("editor.save");
  btnSave.addEventListener("click", async () => {
    // Read form values from inputs
    const state = { ...values };
    const fieldEls = form.querySelectorAll(".form-field");
    fieldEls.forEach((el) => {
      const label = el.querySelector(".form-label")?.textContent;
      const input = el.querySelector("input, select");
      if (!label || !input) return;
      const def = fields.find((f) => f.label === label);
      if (!def) return;
      if (input.type === "number") {
        state[def.key] = input.value === "" ? 0 : parseFloat(input.value);
      } else if (input.type === "select-one") {
        state[def.key] = input.value;
      } else if (input.type === "text") {
        state[def.key] = input.value;
      }
    });

    state.resistances = JSON.stringify(resistances);
    state.weaknesses = JSON.stringify(weaknesses);
    state.anomaly_buildup = values.anomaly_buildup || "{}";

    try {
      await saveEnemy(state);
      editContainer.remove();
      showNotification(t("editor.enemies.saved"), "success");
      renderCurrentTab(rootContainer);
    } catch (e) {
      showNotification(t("editor.enemies.saveError") + ": " + e, "error");
    }
  });
  btnRow.appendChild(btnSave);

  const btnCancel = document.createElement("button");
  btnCancel.className = "btn btn-secondary";
  btnCancel.textContent = t("editor.cancel");
  btnCancel.addEventListener("click", () => editContainer.remove());
  btnRow.appendChild(btnCancel);

  formEl.appendChild(btnRow);
  editContainer.appendChild(formEl);
  editContainer.scrollIntoView({ behavior: "smooth", block: "start" });
}

// ── Helpers ─────────────────────────────────────────────

function getEmptyEnemy(enemyId) {
  return {
    enemy_id: enemyId || "",
    name: "",
    enemy_type: "Normal",
    level: 1,
    hp: 0,
    def: 0,
    base_res: 0,
    daze_max: 0,
    resistances: "{}",
    weaknesses: "[]",
    anomaly_buildup: "{}",
  };
}

function getCurrentRoute() {
  const hash = window.location.hash.replace(/^#\//, "");
  return hash.split("/")[0] || "enemies";
}

function renderCurrentTab(container) {
  container.innerHTML = "";
  renderEnemyList(container);
}

async function handleDelete(enemyId, name, rootContainer) {
  const ok = await confirm(t("editor.enemies.confirmDelete", { name }));
  if (!ok) return;
  try {
    await deleteEnemy(enemyId);
    showNotification(t("editor.enemies.deleted"), "success");
    rootContainer.innerHTML = "";
    renderEnemyList(rootContainer);
  } catch (e) {
    showNotification(t("editor.enemies.deleteError") + ": " + e, "error");
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
