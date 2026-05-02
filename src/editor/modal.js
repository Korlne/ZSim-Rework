/**
 * Modal dialog component.
 * @param {string} title  Modal title
 * @param {HTMLElement} bodyEl  Content element
 * @param {Object} [options]
 * @param {boolean} [options.wide]  Use wider dialog
 * @returns {{ overlay: HTMLDivElement, close: () => void }}
 */
export function createModal(title, bodyEl, options = {}) {
  const overlay = document.createElement("div");
  overlay.className = "modal-overlay";

  const dialog = document.createElement("div");
  dialog.className = `modal-dialog${options.wide ? " modal-wide" : ""}`;

  const header = document.createElement("div");
  header.className = "modal-header";

  const titleEl = document.createElement("h3");
  titleEl.textContent = title;

  const closeBtn = document.createElement("button");
  closeBtn.className = "modal-close";
  closeBtn.textContent = "×";
  closeBtn.addEventListener("click", () => close());

  header.appendChild(titleEl);
  header.appendChild(closeBtn);

  const bodyWrapper = document.createElement("div");
  bodyWrapper.className = "modal-body";
  bodyWrapper.appendChild(bodyEl);

  dialog.appendChild(header);
  dialog.appendChild(bodyWrapper);
  overlay.appendChild(dialog);
  document.body.appendChild(overlay);

  const close = () => {
    overlay.remove();
    if (options.onClose) options.onClose();
  };

  overlay.addEventListener("click", (e) => {
    if (e.target === overlay) close();
  });

  return { overlay, close };
}
