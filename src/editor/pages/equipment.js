import { t } from "../../i18n.js";

const routeTitles = {
  "w-engines": "editor.nav.wEngines",
  "drive-discs": "editor.nav.driveDiscs",
  "disc-sets": "editor.nav.discSets",
};

export function renderPage(route) {
  const container = document.createElement("div");
  const heading = document.createElement("h2");
  heading.textContent = t(routeTitles[route] || route);
  container.appendChild(heading);

  const placeholder = document.createElement("p");
  placeholder.textContent = t("editor.comingSoon");
  container.appendChild(placeholder);

  return container;
}
