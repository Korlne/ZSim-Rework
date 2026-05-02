import { getDataSummary, initDatabase, reimportAll } from "../utils/api.js";
import { t } from "../../i18n.js";

export function renderPage() {
  const container = document.createElement("div");
  container.className = "page-dashboard";

  const header = document.createElement("div");
  header.className = "dashboard-header";
  header.innerHTML = `<h2>${t("editor.nav.dashboard")}</h2>`;
  container.appendChild(header);

  // Action buttons
  const actionRow = document.createElement("div");
  actionRow.className = "button-row";
  actionRow.style.marginBottom = "20px";

  const btnInit = document.createElement("button");
  btnInit.className = "btn btn-primary";
  btnInit.textContent = t("editor.dashboard.initBtn");
  btnInit.addEventListener("click", async () => {
    btnInit.disabled = true;
    try {
      const result = await initDatabase();
      showEditorNotif(
        t("editor.dashboard.dbReady", { tables: (result.tables || []).length }),
        "success"
      );
      loadSummary(cardsContainer);
    } catch (err) {
      showEditorNotif(t("editor.dashboard.dbError") + ": " + err, "error");
    } finally {
      btnInit.disabled = false;
    }
  });
  actionRow.appendChild(btnInit);

  const btnReimport = document.createElement("button");
  btnReimport.className = "btn btn-secondary";
  btnReimport.textContent = t("editor.dashboard.importAllBtn");
  btnReimport.addEventListener("click", async () => {
    btnReimport.disabled = true;
    try {
      const result = await reimportAll();
      showEditorNotif(
        t("editor.notif.imported", {
          success: Object.values(result.imported || {}).reduce((s, r) => s + (r.success || 0), 0),
          errors: Object.values(result.imported || {}).reduce((s, r) => s + (r.errors || []).length, 0),
        }),
        "success"
      );
      loadSummary(cardsContainer);
    } catch (err) {
      showEditorNotif(t("editor.dashboard.importError") + ": " + err, "error");
    } finally {
      btnReimport.disabled = false;
    }
  });
  actionRow.appendChild(btnReimport);

  container.appendChild(actionRow);

  // Cards container
  const cardsContainer = document.createElement("div");
  cardsContainer.className = "dashboard-grid";
  cardsContainer.id = "dashboard-cards";
  container.appendChild(cardsContainer);

  loadSummary(cardsContainer);

  return container;
}

async function loadSummary(container) {
  container.innerHTML = `<div class="page-loading">${t("editor.loading")}</div>`;
  try {
    const json = await getDataSummary();
    const summary = typeof json === "string" ? JSON.parse(json) : json;
    const cards = [
      { key: "characters", label: t("editor.dashboard.card.characters") },
      { key: "skills", label: t("editor.dashboard.card.skills") },
      { key: "w_engines", label: t("editor.dashboard.card.w_engines") },
      { key: "drive_discs", label: t("editor.dashboard.card.drive_discs") },
      { key: "disc_sets", label: t("editor.dashboard.card.disc_sets") },
      { key: "enemies", label: t("editor.dashboard.card.enemies") },
      { key: "apl", label: t("editor.dashboard.card.apl") },
    ];
    container.innerHTML = "";
    for (const card of cards) {
      const div = document.createElement("div");
      div.className = "dashboard-card";
      const value = document.createElement("div");
      value.className = "card-value";
      value.textContent = (summary[card.key] ?? 0).toLocaleString();
      const label = document.createElement("div");
      label.className = "card-label";
      label.textContent = card.label;
      div.appendChild(value);
      div.appendChild(label);
      container.appendChild(div);
    }
  } catch (err) {
    container.innerHTML = `<div class="page-error"><p>${t("editor.loadError")}: ${err.message}</p></div>`;
  }
}

function showEditorNotif(message, type = "info") {
  const existing = document.querySelector(".editor-notification");
  if (existing) existing.remove();

  const el = document.createElement("div");
  el.className = `editor-notification ${type}`;
  el.textContent = message;
  document.body.appendChild(el);
  setTimeout(() => el.remove(), 4000);
}
