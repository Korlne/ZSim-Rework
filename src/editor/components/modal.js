/**
 * Modal dialog component.
 * Returns an object with .close() method.
 *
 * @param {string} title  Modal title
 * @param {HTMLElement} body  Body content element
 * @param {Object} [options]
 * @param {boolean} [options.wide]  Use wide modal (860px instead of 640px)
 * @param {function} [options.onClose]  Called when modal is closed
 * @returns {{ close: function }}
 */
export function createModal(title, body, options = {}) {
  const overlay = document.createElement("div");
  overlay.className = "modal-overlay";

  const dialog = document.createElement("div");
  dialog.className = "modal-dialog" + (options.wide ? " modal-wide" : "");

  const header = document.createElement("div");
  header.className = "modal-header";

  const titleEl = document.createElement("h3");
  titleEl.textContent = title;
  header.appendChild(titleEl);

  const closeBtn = document.createElement("button");
  closeBtn.className = "modal-close";
  closeBtn.innerHTML = "&times;";
  closeBtn.addEventListener("click", () => close());
  header.appendChild(closeBtn);

  const bodyContainer = document.createElement("div");
  bodyContainer.className = "modal-body";
  bodyContainer.appendChild(body);

  dialog.appendChild(header);
  dialog.appendChild(bodyContainer);
  overlay.appendChild(dialog);
  document.body.appendChild(overlay);

  function close() {
    overlay.remove();
    if (options.onClose) options.onClose();
  }

  // Close on overlay click
  overlay.addEventListener("click", (e) => {
    if (e.target === overlay) close();
  });

  // Close on Escape
  const keyHandler = (e) => {
    if (e.key === "Escape") close();
  };
  document.addEventListener("keydown", keyHandler);
  overlay.addEventListener("remove", () => {
    document.removeEventListener("keydown", keyHandler);
  }, { once: true });

  return { close };
}
