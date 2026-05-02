import { t } from "../../i18n.js";

export function renderPage() {
  const container = document.createElement("div");
  const heading = document.createElement("h2");
  heading.textContent = t("editor.nav.characters");
  container.appendChild(heading);

  const placeholder = document.createElement("p");
  placeholder.textContent = t("editor.comingSoon");
  container.appendChild(placeholder);

  return container;
}
