import { t } from "../../i18n.js";

export function renderPage(route) {
  const labels = {
    "w-engines": "editor.nav.wEngines",
    "drive-discs": "editor.nav.driveDiscs",
    "disc-sets": "editor.nav.discSets",
  };
  const container = document.createElement("div");
  container.className = "page-equipment";
  container.innerHTML = `<h2>${t(labels[route] || "editor.nav.equipment")}</h2><p>${t("editor.comingSoon")}</p>`;
  return container;
}
