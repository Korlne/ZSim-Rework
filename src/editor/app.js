import { t, lang, switchLang } from "../i18n.js";
import { createSidebar } from "./sidebar.js";

// ── DOM refs (lazily resolved) ─────────────────────────────

let viewContainer = null;
let contentArea = null;

function ensureLayout() {
  viewContainer = document.getElementById("editor-view");
  if (!viewContainer) return;

  contentArea = document.getElementById("editor-content");
  if (!contentArea) {
    viewContainer.className = "editor-layout view-panel";
    contentArea = document.createElement("div");
    contentArea.id = "editor-content";
    contentArea.className = "editor-content";
    viewContainer.appendChild(contentArea);
  }
}

// ── Route map ──────────────────────────────────────────────

const routes = {
  dashboard: () => import("./pages/dashboard.js"),
  characters: () => import("./pages/characters.js"),
  skills: () => import("./pages/skills.js"),
  "w-engines": () => import("./pages/equipment.js"),
  "drive-discs": () => import("./pages/equipment.js"),
  "disc-sets": () => import("./pages/equipment.js"),
  enemies: () => import("./pages/enemies.js"),
  import: () => import("./pages/import.js"),
};

const routeTitles = {
  dashboard: "editor.nav.dashboard",
  characters: "editor.nav.characters",
  skills: "editor.nav.skills",
  "w-engines": "editor.nav.wEngines",
  "drive-discs": "editor.nav.driveDiscs",
  "disc-sets": "editor.nav.discSets",
  enemies: "editor.nav.enemies",
  import: "editor.nav.import",
};

// ── Router ─────────────────────────────────────────────────

function getRoute() {
  const hash = window.location.hash.replace(/^#\//, "");
  if (!hash) return "dashboard";

  // Support nested routes: #/equipment/w-engines -> "w-engines"
  const parts = hash.split("/");
  return parts[parts.length - 1];
}

async function navigate() {
  ensureLayout();
  const route = getRoute();
  contentArea.innerHTML = `<div class="page-loading">${t("editor.loading")}</div>`;

  // Update sidebar active state
  const sidebar = createSidebar(route);
  const oldSidebar = viewContainer.querySelector(".editor-sidebar");
  if (oldSidebar) oldSidebar.replaceWith(sidebar);
  else viewContainer.insertBefore(sidebar, contentArea);

  // Load the page module
  const loader = routes[route];
  if (!loader) {
    contentArea.innerHTML = `<div class="page-error"><h2>${t("editor.notFound")}</h2></div>`;
    return;
  }

  try {
    const module = await loader();
    if (module.renderPage) {
      contentArea.innerHTML = "";
      contentArea.appendChild(module.renderPage(route));
    } else {
      contentArea.innerHTML = `<h2>${t(routeTitles[route] || route)}</h2><p>${t("editor.comingSoon")}</p>`;
    }
  } catch (err) {
    console.error("Page load error:", err);
    contentArea.innerHTML = `<div class="page-error"><h2>${t("editor.loadError")}</h2><pre>${err.message}</pre></div>`;
  }
}

// ── Re-translate dynamic content ──────────────────────────

function reapplyEditorText() {
  const titleEl = document.querySelector(".editor-sidebar");
  if (titleEl) {
    const oldSidebar = titleEl;
    const newSidebar = createSidebar(getRoute());
    oldSidebar.replaceWith(newSidebar);
  }
}

// ── Init ───────────────────────────────────────────────────

let initialized = false;

export function init() {
  if (initialized) return;
  initialized = true;

  window.addEventListener("hashchange", navigate);
  window.addEventListener("langchange", reapplyEditorText);

  // Initial navigation
  navigate();
}
