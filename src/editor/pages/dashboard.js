import { getDataSummary, initDatabase, reimportAll, searchData, exportToJson } from "../utils/api.js";
import { t } from "../../i18n.js";

export function renderPage() {
  const container = document.createElement("div");
  container.className = "page-dashboard";

  const header = document.createElement("div");
  header.className = "dashboard-header";
  header.innerHTML = `<h2>${t("editor.nav.dashboard")}</h2>`;
  container.appendChild(header);

  // Quick search
  const searchSection = document.createElement("div");
  searchSection.className = "search-bar";
  const searchInput = document.createElement("input");
  searchInput.type = "text";
  searchInput.placeholder = t("editor.dashboard.searchPlaceholder");
  searchInput.id = "dashboard-search";
  searchSection.appendChild(searchInput);

  const searchResults = document.createElement("div");
  searchResults.className = "search-results";
  searchResults.id = "dashboard-search-results";
  searchResults.style.display = "none";
  searchSection.appendChild(searchResults);
  container.appendChild(searchSection);

  let searchTimeout = null;
  searchInput.addEventListener("input", () => {
    clearTimeout(searchTimeout);
    const q = searchInput.value.trim();
    if (!q) {
      searchResults.style.display = "none";
      searchResults.innerHTML = "";
      return;
    }
    searchTimeout = setTimeout(async () => {
      try {
        const raw = await searchData(q, "all");
        const results = typeof raw === "string" ? JSON.parse(raw) : raw;
        if (!results || results.length === 0) {
          searchResults.innerHTML = `<div class="search-result-empty">${t("editor.dashboard.searchNoResults")}</div>`;
        } else {
          searchResults.innerHTML = results.map(r => {
            const typeLabel = {
              character: t("editor.dashboard.searchResultChars"),
              skill: t("editor.dashboard.searchResultSkills"),
              enemy: t("editor.dashboard.searchResultEnemies"),
              w_engine: t("editor.dashboard.searchResultWEngines"),
              disc_set: t("editor.dashboard.searchResultDiscSets"),
            }[r.data_type] || r.data_type;
            return `<div class="search-result-item" data-type="${r.data_type}" data-id="${r.id}">
              <span class="search-result-type">${typeLabel}</span>
              <span class="search-result-id">${r.id}</span>
              <span class="search-result-name">${r.name || ""}</span>
            </div>`;
          }).join("");
          // Click handler to navigate
          searchResults.querySelectorAll(".search-result-item").forEach(el => {
            el.addEventListener("click", () => {
              const type = el.dataset.type;
              const route = {
                character: "characters",
                skill: "skills",
                enemy: "enemies",
                w_engine: "w-engines",
                disc_set: "disc-sets",
              }[type] || "dashboard";
              searchResults.style.display = "none";
              searchInput.value = "";
              window.location.hash = `#/${route}`;
            });
          });
        }
        searchResults.style.display = "block";
      } catch (err) {
        searchResults.innerHTML = `<div class="search-result-empty error">${err.message}</div>`;
        searchResults.style.display = "block";
      }
    }, 300);
  });

  // Dismiss search results on click outside
  document.addEventListener("click", (e) => {
    if (!searchSection.contains(e.target)) {
      searchResults.style.display = "none";
    }
  });

  // Action buttons
  const actionRow = document.createElement("div");
  actionRow.className = "button-row";
  actionRow.style.marginBottom = "20px";
  actionRow.style.display = "flex";
  actionRow.style.gap = "8px";
  actionRow.style.flexWrap = "wrap";

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
      loadRecentItems(recentContainer);
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
      loadRecentItems(recentContainer);
    } catch (err) {
      showEditorNotif(t("editor.dashboard.importError") + ": " + err, "error");
    } finally {
      btnReimport.disabled = false;
    }
  });
  actionRow.appendChild(btnReimport);

  const btnExport = document.createElement("button");
  btnExport.className = "btn btn-secondary";
  btnExport.textContent = t("editor.dashboard.exportBtn");
  btnExport.addEventListener("click", async () => {
    btnExport.disabled = true;
    try {
      await exportToJson();
      showEditorNotif(t("editor.dashboard.exportSuccess"), "success");
    } catch (err) {
      showEditorNotif(t("editor.dashboard.exportError") + ": " + err, "error");
    } finally {
      btnExport.disabled = false;
    }
  });
  actionRow.appendChild(btnExport);

  container.appendChild(actionRow);

  // Cards container
  const cardsContainer = document.createElement("div");
  cardsContainer.className = "dashboard-grid";
  cardsContainer.id = "dashboard-cards";
  container.appendChild(cardsContainer);

  // Recently edited section
  const recentTitle = document.createElement("h3");
  recentTitle.className = "recent-title";
  recentTitle.textContent = t("editor.dashboard.recentTitle");
  container.appendChild(recentTitle);

  const recentContainer = document.createElement("div");
  recentContainer.className = "recent-list";
  recentContainer.id = "recent-items";
  container.appendChild(recentContainer);

  loadSummary(cardsContainer);
  loadRecentItems(recentContainer);

  return container;
}

async function loadRecentItems(container) {
  try {
    const chars = await (await import("../utils/api.js")).getCharacters();
    const charList = Array.isArray(chars) ? chars.slice(0, 5) : [];
    container.innerHTML = "";
    if (charList.length === 0) {
      container.innerHTML = `<div class="recent-empty">${t("editor.dashboard.recentEmpty")}</div>`;
      return;
    }
    for (const c of charList) {
      const item = document.createElement("div");
      item.className = "recent-item";
      item.textContent = `${c.name || c.char_id} (${c.char_id})`;
      item.style.cursor = "pointer";
      item.addEventListener("click", () => {
        window.location.hash = "#/characters";
      });
      container.appendChild(item);
    }
  } catch {
    container.innerHTML = "";
  }
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
