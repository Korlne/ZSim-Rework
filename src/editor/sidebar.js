import { t } from "../i18n.js";

/**
 * Sidebar navigation component.
 * Returns a sidebar element with nav items.
 */
export function createSidebar(activeRoute) {
  const sidebar = document.createElement("nav");
  sidebar.className = "editor-sidebar";

  const items = [
    { id: "dashboard", icon: "📊", label: "editor.nav.dashboard" },
    { id: "characters", icon: "👤", label: "editor.nav.characters" },
    {
      id: "equipment",
      icon: "⚙️",
      label: "editor.nav.equipment",
      children: [
        { id: "w-engines", label: "editor.nav.wEngines" },
        { id: "drive-discs", label: "editor.nav.driveDiscs" },
        { id: "disc-sets", label: "editor.nav.discSets" },
      ],
    },
    { id: "enemies", icon: "👾", label: "editor.nav.enemies" },
    { id: "import", icon: "📥", label: "editor.nav.import" },
  ];

  const ul = document.createElement("ul");
  ul.className = "sidebar-list";

  for (const item of items) {
    const li = document.createElement("li");
    li.className = "sidebar-item";

    if (item.children) {
      const header = document.createElement("div");
      header.className = "sidebar-header";
      header.textContent = `${item.icon} ${t(item.label)}`;
      li.appendChild(header);

      const subUl = document.createElement("ul");
      subUl.className = "sidebar-sublist";
      for (const child of item.children) {
        const subLi = document.createElement("li");
        subLi.className = `sidebar-subitem${child.id === activeRoute ? " active" : ""}`;
        subLi.textContent = t(child.label);
        subLi.dataset.route = child.id;
        subLi.addEventListener("click", () => {
          window.location.hash = `#/${child.id}`;
        });
        subUl.appendChild(subLi);
      }
      li.appendChild(subUl);
    } else {
      const a = document.createElement("a");
      a.className = `sidebar-link${item.id === activeRoute ? " active" : ""}`;
      a.href = `#/${item.id}`;
      a.textContent = `${item.icon} ${t(item.label)}`;
      li.appendChild(a);
    }

    ul.appendChild(li);
  }

  sidebar.appendChild(ul);
  return sidebar;
}
