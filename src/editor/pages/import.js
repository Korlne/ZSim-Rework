import { t } from "../../i18n.js";

export function renderPage() {
  const container = document.createElement("div");
  container.className = "page-import";
  container.innerHTML = `<h2>${t("editor.nav.import")}</h2><p>${t("editor.comingSoon")}</p>`;
  return container;
}
