/**
 * Reusable form component.
 *
 * @param {Array<{
 *   key: string,
 *   label: string,
 *   type: 'text'|'number'|'select'|'textarea'|'checkbox'|'tag-input',
 *   options?: Array<{value: string, label: string}>,
 *   required?: boolean,
 *   readonly?: boolean,
 *   step?: number,
 *   min?: number,
 *   max?: number,
 * }>} fields  Field definitions
 * @param {Record<string, any>} values  Initial values
 * @param {Object} [options]
 * @param {function} [options.onSave]  Called with current values
 * @param {function} [options.onCancel]  Called on cancel
 * @returns {HTMLDivElement}
 */
export function createForm(fields, values, options = {}) {
  const container = document.createElement("div");
  container.className = "editor-form";

  const formFields = document.createElement("div");
  formFields.className = "form-fields";

  const state = { ...values };

  const inputs = {};

  for (const field of fields) {
    const fieldGroup = document.createElement("div");
    fieldGroup.className = "form-field";

    const label = document.createElement("label");
    label.className = "form-label";
    label.textContent = field.label;
    fieldGroup.appendChild(label);

    let input;

    switch (field.type) {
      case "select": {
        input = document.createElement("select");
        input.className = "form-input form-select";
        if (field.options) {
          for (const opt of field.options) {
            const option = document.createElement("option");
            option.value = opt.value;
            option.textContent = opt.label;
            if (opt.value === state[field.key]) option.selected = true;
            input.appendChild(option);
          }
        }
        input.addEventListener("change", () => {
          state[field.key] = input.value;
        });
        break;
      }

      case "checkbox": {
        input = document.createElement("input");
        input.type = "checkbox";
        input.className = "form-checkbox";
        input.checked = !!state[field.key];
        input.addEventListener("change", () => {
          state[field.key] = input.checked;
        });
        break;
      }

      case "textarea": {
        input = document.createElement("textarea");
        input.className = "form-input form-textarea";
        input.value = state[field.key] ?? "";
        input.addEventListener("input", () => {
          state[field.key] = input.value;
        });
        break;
      }

      case "tag-input": {
        const wrapper = document.createElement("div");
        wrapper.className = "tag-input-wrapper";

        const tagContainer = document.createElement("div");
        tagContainer.className = "tag-container";

        const textInput = document.createElement("input");
        textInput.type = "text";
        textInput.className = "form-input tag-text-input";
        textInput.placeholder = "Add...";

        const tags = Array.isArray(state[field.key])
          ? [...state[field.key]]
          : [];

        function renderTags() {
          tagContainer.innerHTML = "";
          for (const tag of tags) {
            const tagEl = document.createElement("span");
            tagEl.className = "tag-item";
            tagEl.textContent = tag;
            const removeBtn = document.createElement("button");
            removeBtn.type = "button";
            removeBtn.className = "tag-remove";
            removeBtn.textContent = "×";
            removeBtn.addEventListener("click", () => {
              const idx = tags.indexOf(tag);
              if (idx !== -1) tags.splice(idx, 1);
              renderTags();
              state[field.key] = [...tags];
            });
            tagEl.appendChild(removeBtn);
            tagContainer.appendChild(tagEl);
          }
        }

        textInput.addEventListener("keydown", (e) => {
          if (e.key === "Enter" && textInput.value.trim()) {
            e.preventDefault();
            tags.push(textInput.value.trim());
            textInput.value = "";
            renderTags();
            state[field.key] = [...tags];
          }
        });

        renderTags();
        wrapper.appendChild(tagContainer);
        wrapper.appendChild(textInput);
        fieldGroup.appendChild(wrapper);
        inputs[field.key] = { value: () => state[field.key] };
        break;
      }

      default: {
        // text / number
        input = document.createElement("input");
        input.type = field.type === "number" ? "number" : "text";
        input.className = "form-input";
        input.value = state[field.key] ?? "";

        if (field.readonly) input.readOnly = true;
        if (field.step !== undefined) input.step = field.step;
        if (field.min !== undefined) input.min = field.min;
        if (field.max !== undefined) input.max = field.max;

        if (field.type === "number") {
          input.addEventListener("input", () => {
            state[field.key] = input.value === "" ? "" : parseFloat(input.value);
          });
        } else {
          input.addEventListener("input", () => {
            state[field.key] = input.value;
          });
        }
        break;
      }
    }

    if (input && field.type !== "tag-input") {
      fieldGroup.appendChild(input);
      inputs[field.key] = input;
    }

    formFields.appendChild(fieldGroup);
  }

  container.appendChild(formFields);

  // Buttons
  const btnRow = document.createElement("div");
  btnRow.className = "form-buttons";

  if (options.onSave) {
    const btnSave = document.createElement("button");
    btnSave.className = "btn btn-primary";
    btnSave.textContent = "Save";
    btnSave.addEventListener("click", () => options.onSave(state));
    btnRow.appendChild(btnSave);
  }

  if (options.onCancel) {
    const btnCancel = document.createElement("button");
    btnCancel.className = "btn btn-secondary";
    btnCancel.textContent = "Cancel";
    btnCancel.addEventListener("click", () => options.onCancel());
    btnRow.appendChild(btnCancel);
  }

  container.appendChild(btnRow);
  return container;
}
