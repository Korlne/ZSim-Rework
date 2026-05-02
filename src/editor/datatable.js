/**
 * Reusable data table component.
 *
 * @param {Array<{key: string, label: string}>} columns  Column definitions
 * @param {Array<Record<string, any>>} rows  Data rows
 * @param {Object} [options]
 * @param {function} [options.onEdit]  Called with row data
 * @param {function} [options.onDelete]  Called with row data
 * @returns {HTMLTableElement}
 */
export function createDataTable(columns, rows, options = {}) {
  const table = document.createElement("table");
  table.className = "data-table";

  // Header
  const thead = document.createElement("thead");
  const headerRow = document.createElement("tr");
  for (const col of columns) {
    const th = document.createElement("th");
    th.textContent = col.label;
    headerRow.appendChild(th);
  }
  if (options.onEdit || options.onDelete) {
    const th = document.createElement("th");
    th.className = "col-actions";
    th.textContent = "Actions";
    headerRow.appendChild(th);
  }
  thead.appendChild(headerRow);
  table.appendChild(thead);

  // Body
  const tbody = document.createElement("tbody");
  if (rows.length === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = columns.length + (options.onEdit || options.onDelete ? 1 : 0);
    td.className = "empty-state";
    td.textContent = "No data";
    tr.appendChild(td);
    tbody.appendChild(tr);
  } else {
    for (const row of rows) {
      const tr = document.createElement("tr");
      for (const col of columns) {
        const td = document.createElement("td");
        let val = row[col.key];
        if (col.format) {
          val = col.format(val);
        } else if (typeof val === "number") {
          val = Number.isInteger(val) ? val : val.toFixed(2);
        } else if (val === null || val === undefined) {
          val = "-";
        }
        td.textContent = val;
        if (col.className) td.className = col.className;
        tr.appendChild(td);
      }

      if (options.onEdit || options.onDelete) {
        const td = document.createElement("td");
        td.className = "col-actions";

        if (options.onEdit) {
          const btnEdit = document.createElement("button");
          btnEdit.className = "btn btn-sm btn-secondary";
          btnEdit.textContent = "Edit";
          btnEdit.addEventListener("click", () => options.onEdit(row));
          td.appendChild(btnEdit);
        }

        if (options.onDelete) {
          const btnDel = document.createElement("button");
          btnDel.className = "btn btn-sm btn-danger";
          btnDel.textContent = "Del";
          btnDel.addEventListener("click", () => options.onDelete(row));
          td.appendChild(btnDel);
        }

        tr.appendChild(td);
      }

      tbody.appendChild(tr);
    }
  }
  table.appendChild(tbody);

  return table;
}
