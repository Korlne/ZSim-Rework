import { getDataSummary } from "../utils/api.js";
import { t } from "../../i18n.js";

export function renderPage() {
  const container = document.createElement("div");
  container.className = "page-characters";
  container.innerHTML = `<h2>${t("editor.nav.characters")}</h2><p>${t("editor.comingSoon")}</p>`;
  return container;
}
