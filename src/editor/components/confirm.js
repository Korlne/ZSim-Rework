import { t } from "../../i18n.js";

/**
 * Confirmation dialog component.
 * Returns a promise that resolves to true/false.
 *
 * @param {string} message  Confirmation message
 * @param {Object} [options]
 * @param {string} [options.confirmLabel]  Label for confirm button
 * @param {string} [options.cancelLabel]  Label for cancel button
 * @param {string} [options.variant]  'danger' (default) or 'primary'
 * @returns {Promise<boolean>}
 */
export function confirm(message, options = {}) {
  return new Promise((resolve) => {
    const overlay = document.createElement("div");
    overlay.className = "confirm-overlay";

    const dialog = document.createElement("div");
    dialog.className = "confirm-dialog";

    const msgEl = document.createElement("p");
    msgEl.className = "confirm-message";
    msgEl.textContent = message;

    const btnRow = document.createElement("div");
    btnRow.className = "confirm-buttons";

    const btnCancel = document.createElement("button");
    btnCancel.className = "btn btn-secondary";
    btnCancel.textContent = options.cancelLabel || t("editor.confirm.cancel");
    btnCancel.addEventListener("click", () => {
      overlay.remove();
      resolve(false);
    });

    const btnConfirm = document.createElement("button");
    const variant = options.variant || "danger";
    btnConfirm.className = `btn btn-${variant}`;
    btnConfirm.textContent = options.confirmLabel || t("editor.confirm.delete");
    btnConfirm.addEventListener("click", () => {
      overlay.remove();
      resolve(true);
    });

    btnRow.appendChild(btnCancel);
    btnRow.appendChild(btnConfirm);
    dialog.appendChild(msgEl);
    dialog.appendChild(btnRow);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    // Close on overlay click
    overlay.addEventListener("click", (e) => {
      if (e.target === overlay) {
        overlay.remove();
        resolve(false);
      }
    });

    // Focus confirm button for keyboard users
    btnConfirm.focus();
  });
}
