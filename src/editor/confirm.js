/**
 * Confirmation dialog component.
 * Returns a promise that resolves to true/false.
 */
export function confirm(message) {
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
    btnCancel.textContent = "Cancel";
    btnCancel.addEventListener("click", () => {
      overlay.remove();
      resolve(false);
    });

    const btnConfirm = document.createElement("button");
    btnConfirm.className = "btn btn-danger";
    btnConfirm.textContent = "Delete";
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
  });
}
