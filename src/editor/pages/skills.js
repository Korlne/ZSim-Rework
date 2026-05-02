import { t } from "../../i18n.js";
import { createDataTable } from "../components/datatable.js";
import { confirm } from "../components/confirm.js";
import { getCharacters, getSkills, saveSkill, deleteSkill } from "../utils/api.js";

// ── State ──────────────────────────────────────────────────

let currentCharId = "";
let charSkills = [];

const ACTION_TYPES = ["Normal", "Special", "Ultimate", "Chain", "Assist", "Quick", "Dodge", "Parry", "Evade"];

// ── Render ─────────────────────────────────────────────────

export function renderPage() {
  const container = document.createElement("div");

  const heading = document.createElement("h2");
  heading.textContent = t("editor.nav.skills");
  container.appendChild(heading);

  // Character selector
  const selectorRow = document.createElement("div");
  selectorRow.style.cssText = "display:flex;align-items:center;gap:10px;margin-bottom:16px;";

  const selectorLabel = document.createElement("label");
  selectorLabel.className = "form-label";
  selectorLabel.textContent = t("editor.skills.selectCharacter") + ":";
  selectorRow.appendChild(selectorLabel);

  const charSelect = document.createElement("select");
  charSelect.className = "form-input form-select";
  charSelect.style.flex = "1";
  const placeholderOpt = document.createElement("option");
  placeholderOpt.value = "";
  placeholderOpt.textContent = t("editor.skills.charSelectPlaceholder");
  charSelect.appendChild(placeholderOpt);
  selectorRow.appendChild(charSelect);

  const addBtn = document.createElement("button");
  addBtn.className = "btn btn-primary";
  addBtn.textContent = t("editor.skills.addBtn");
  addBtn.disabled = true;
  addBtn.addEventListener("click", () => showEditForm(null, container));
  selectorRow.appendChild(addBtn);
  container.appendChild(selectorRow);

  // Skill table container
  const tableContainer = document.createElement("div");
  tableContainer.id = "skill-table-container";
  container.appendChild(tableContainer);

  // Load characters for dropdown
  getCharacters().then((chars) => {
    for (const c of chars) {
      const opt = document.createElement("option");
      opt.value = c.char_id;
      opt.textContent = c.char_id + " - " + c.name;
      charSelect.appendChild(opt);
    }
  }).catch(() => {});

  charSelect.addEventListener("change", async () => {
    currentCharId = charSelect.value;
    addBtn.disabled = !currentCharId;

    // Remove existing edit panel
    const existing = document.getElementById("skill-edit-container");
    if (existing) existing.remove();

    if (!currentCharId) {
      tableContainer.innerHTML = "";
      return;
    }

    try {
      charSkills = await getSkills(currentCharId);
    } catch (e) {
      charSkills = [];
    }
    refreshSkillTable(tableContainer, container);
  });

  return container;
}

// ── Table ──────────────────────────────────────────────────

function refreshSkillTable(tableContainer, rootContainer) {
  const columns = [
    { key: "action_id", label: t("editor.skills.colActionId") },
    { key: "action_type", label: t("editor.skills.colActionType") },
    { key: "energy_cost", label: t("editor.skills.colEnergyCost") },
    { key: "cooldown_ticks", label: t("editor.skills.colCooldown") },
  ];

  const rows = charSkills.map((swm) => ({
    ...swm.skill,
    _multipliers: swm.multipliers,
  }));

  const table = createDataTable(columns, rows, {
    onEdit: (row) => showEditForm(row.action_id, rootContainer),
    onDelete: (row) => handleDelete(row.id, row.action_id, rootContainer),
  });

  tableContainer.innerHTML = "";
  tableContainer.appendChild(table);
}

async function handleDelete(skillId, actionId, rootContainer) {
  const ok = await confirm(t("editor.skills.confirmDelete", { action_id: actionId }));
  if (!ok) return;
  try {
    await deleteSkill(skillId);
    showNotification(t("editor.skills.deleted", { action_id: actionId }), "success");
    charSkills = await getSkills(currentCharId);
    const tc = document.getElementById("skill-table-container");
    if (tc) refreshSkillTable(tc, rootContainer);
  } catch (e) {
    showNotification(t("editor.skills.deleteError") + ": " + e, "error");
  }
}

// ── Edit Form ──────────────────────────────────────────────

function showEditForm(actionId, rootContainer) {
  // Remove existing edit panel
  const existing = document.getElementById("skill-edit-container");
  if (existing) existing.remove();

  let data;
  let multipliers = [];

  if (actionId) {
    const found = charSkills.find((s) => s.skill.action_id === actionId);
    if (!found) {
      showNotification(t("editor.skills.loadError"), "error");
      return;
    }
    data = { ...found.skill };
    multipliers = found.multipliers.map((m) => ({ ...m }));
  } else {
    data = {
      id: null, char_id: currentCharId, action_id: "", action_type: "Normal",
      daze_multiplier: 0,
      energy_cost: 0, decibel_cost: 0, hp_cost: 0,
      cooldown_ticks: 0, animation_frames: 0, interruptible_frame: null,
      is_snapshot: false, prerequisite_action_id: null, effect_id: null,
    };
    multipliers = [];
  }

  const editContainer = document.createElement("div");
  editContainer.id = "skill-edit-container";
  editContainer.style.marginTop = "20px";
  rootContainer.appendChild(editContainer);

  // Get existing action_ids for prerequisite dropdown
  const existingActionIds = charSkills
    .filter((s) => s.skill.action_id !== actionId)
    .map((s) => s.skill.action_id);

  const formEl = document.createElement("div");
  formEl.className = "editor-form";

  const sectionTitle = document.createElement("h3");
  sectionTitle.textContent = actionId
    ? t("editor.skills.editTitle")
    : t("editor.skills.addTitle");
  sectionTitle.style.marginBottom = "16px";
  formEl.appendChild(sectionTitle);

  const fieldsWrapper = document.createElement("div");
  formEl.appendChild(fieldsWrapper);

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
          if (opt === data[key]) el.selected = true;
          input.appendChild(el);
        }
      }
      input.addEventListener("change", () => { data[key] = input.value; });
    } else if (type === "checkbox") {
      input = document.createElement("input");
      input.type = "checkbox";
      input.className = "form-checkbox";
      input.checked = !!data[key];
      input.addEventListener("change", () => { data[key] = input.checked; });
    } else {
      input = document.createElement("input");
      input.type = type === "number" ? "number" : "text";
      input.className = "form-input";
      if (type === "number") {
        if (extra.step !== undefined) input.step = extra.step;
        if (extra.min !== undefined) input.min = extra.min;
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
    basicGrid.appendChild(group);
    return input;
  }

  // Section 1: Basic params
  const basicGrid = document.createElement("div");
  basicGrid.className = "form-fields";

  addField("action_id", t("editor.skills.fieldActionId"), "text", { readonly: !!actionId });
  addField("action_type", t("editor.skills.fieldActionType"), "select", { options: ACTION_TYPES });
  addField("energy_cost", t("editor.skills.fieldEnergyCost"), "number");
  addField("decibel_cost", t("editor.skills.fieldDecibelCost"), "number");
  addField("hp_cost", t("editor.skills.fieldHpCost"), "number");
  addField("cooldown_ticks", t("editor.skills.fieldCooldown"), "number");
  addField("animation_frames", t("editor.skills.fieldAnimationFrames"), "number");
  addField("interruptible_frame", t("editor.skills.fieldInterruptible"), "number", { min: 0 });
  addField("daze_multiplier", t("editor.skills.fieldDazeMult"), "number", { step: 0.1 });
  addField("is_snapshot", t("editor.skills.fieldIsSnapshot"), "checkbox");

  const prereqOptions = ["", ...existingActionIds];
  addField("prerequisite_action_id", t("editor.skills.fieldPrereq"), "select", { options: prereqOptions });
  addField("effect_id", t("editor.skills.fieldEffectId"), "text");

  fieldsWrapper.appendChild(basicGrid);

  // Section 2: Multipliers
  const multSection = document.createElement("div");
  multSection.style.marginTop = "16px";
  multSection.style.paddingTop = "16px";
  multSection.style.borderTop = "1px solid var(--border)";

  const multTitleRow = document.createElement("div");
  multTitleRow.style.cssText = "display:flex;align-items:center;justify-content:space-between;margin-bottom:8px;";

  const multTitle = document.createElement("div");
  multTitle.className = "form-label";
  multTitle.textContent = t("editor.skills.sectionMultipliers");
  multTitleRow.appendChild(multTitle);

  const addMultBtn = document.createElement("button");
  addMultBtn.className = "btn btn-sm btn-secondary";
  addMultBtn.textContent = t("editor.skills.addMultBtn");
  addMultBtn.addEventListener("click", () => {
    const maxIdx = multipliers.length > 0 ? Math.max(...multipliers.map((m) => m.segment_index)) : 0;
    multipliers.push({ segment_index: maxIdx + 1, frame: 0, multiplier: 0, decay_coeff: null });
    renderMultTable();
  });
  multTitleRow.appendChild(addMultBtn);

  multSection.appendChild(multTitleRow);

  const multContainer = document.createElement("div");
  multSection.appendChild(multContainer);
  fieldsWrapper.appendChild(multSection);

  function renderMultTable() {
    if (multipliers.length === 0) {
      multContainer.innerHTML = '<p class="text-muted" style="font-size:13px;font-style:italic;">' + t("editor.skills.noMultipliers") + "</p>";
      return;
    }

    const table = document.createElement("table");
    table.className = "data-table";

    const thead = document.createElement("thead");
    const headerRow = document.createElement("tr");
    const headers = [
      t("editor.skills.multSegment"),
      t("editor.skills.multFrame"),
      t("editor.skills.multMultiplier"),
      t("editor.skills.multDecay"),
      "",
    ];
    for (const h of headers) {
      const th = document.createElement("th");
      th.textContent = h;
      headerRow.appendChild(th);
    }
    thead.appendChild(headerRow);
    table.appendChild(thead);

    const tbody = document.createElement("tbody");
    for (let i = 0; i < multipliers.length; i++) {
      const m = multipliers[i];
      const tr = document.createElement("tr");

      const tdIdx = document.createElement("td");
      tdIdx.textContent = String(m.segment_index);
      tr.appendChild(tdIdx);

      const tdFrame = document.createElement("td");
      const frameInput = document.createElement("input");
      frameInput.type = "number";
      frameInput.min = 0;
      frameInput.className = "form-input";
      frameInput.value = m.frame;
      frameInput.addEventListener("input", () => { m.frame = parseInt(frameInput.value) || 0; });
      tdFrame.appendChild(frameInput);
      tr.appendChild(tdFrame);

      const tdMult = document.createElement("td");
      const multInput = document.createElement("input");
      multInput.type = "number";
      multInput.step = 0.01;
      multInput.className = "form-input";
      multInput.value = m.multiplier;
      multInput.addEventListener("input", () => { m.multiplier = parseFloat(multInput.value) || 0; });
      tdMult.appendChild(multInput);
      tr.appendChild(tdMult);

      const tdDecay = document.createElement("td");
      const decayInput = document.createElement("input");
      decayInput.type = "number";
      decayInput.step = 0.01;
      decayInput.className = "form-input";
      decayInput.placeholder = "None";
      decayInput.value = m.decay_coeff ?? "";
      decayInput.addEventListener("input", () => {
        m.decay_coeff = decayInput.value === "" ? null : parseFloat(decayInput.value);
      });
      tdDecay.appendChild(decayInput);
      tr.appendChild(tdDecay);

      const tdDel = document.createElement("td");
      tdDel.className = "col-actions";
      const delBtn = document.createElement("button");
      delBtn.className = "btn btn-sm btn-danger";
      delBtn.textContent = t("editor.skills.delMultBtn");
      delBtn.addEventListener("click", () => {
        multipliers.splice(i, 1);
        renderMultTable();
      });
      tdDel.appendChild(delBtn);
      tr.appendChild(tdDel);

      tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    multContainer.innerHTML = "";
    multContainer.appendChild(table);
  }

  renderMultTable();

  // Buttons
  const btnRow = document.createElement("div");
  btnRow.className = "form-buttons";

  const saveBtn = document.createElement("button");
  saveBtn.className = "btn btn-primary";
  saveBtn.textContent = t("editor.save");
  saveBtn.addEventListener("click", async () => {
    const payload = {
      skill: {
        id: data.id,
        char_id: currentCharId,
        action_id: data.action_id,
        action_type: data.action_type,
        daze_multiplier: data.daze_multiplier || 0,
        energy_cost: data.energy_cost || 0,
        decibel_cost: data.decibel_cost || 0,
        hp_cost: data.hp_cost || 0,
        cooldown_ticks: data.cooldown_ticks || 0,
        animation_frames: data.animation_frames || 0,
        interruptible_frame: data.interruptible_frame || null,
        is_snapshot: data.is_snapshot || false,
        prerequisite_action_id: data.prerequisite_action_id || null,
        effect_id: data.effect_id || null,
      },
      multipliers: multipliers.map((m) => ({
        segment_index: m.segment_index,
        frame: m.frame || 0,
        multiplier: m.multiplier || 0,
        decay_coeff: m.decay_coeff ?? null,
      })),
    };

    try {
      await saveSkill(payload);
      editContainer.remove();
      showNotification(t("editor.skills.saved", { action_id: data.action_id }), "success");
      charSkills = await getSkills(currentCharId);
      const tc = document.getElementById("skill-table-container");
      if (tc) refreshSkillTable(tc, rootContainer);
    } catch (e) {
      showNotification(t("editor.skills.saveError") + ": " + e, "error");
    }
  });
  btnRow.appendChild(saveBtn);

  const cancelBtn = document.createElement("button");
  cancelBtn.className = "btn btn-secondary";
  cancelBtn.textContent = t("editor.cancel");
  cancelBtn.addEventListener("click", () => editContainer.remove());
  btnRow.appendChild(cancelBtn);

  formEl.appendChild(btnRow);
  editContainer.appendChild(formEl);
  editContainer.scrollIntoView({ behavior: "smooth", block: "start" });
}

// ── Notification helper ───────────────────────────────────

function showNotification(msg, type = "info") {
  const existing = document.querySelector(".editor-notification");
  if (existing) existing.remove();
  const el = document.createElement("div");
  el.className = "editor-notification " + type;
  el.textContent = msg;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}
