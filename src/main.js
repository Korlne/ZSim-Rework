import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { t, switchLang, init, lang } from "./i18n.js";

// --- DOM refs (original) ---
const statusBadge = document.getElementById("status-badge");
const btnRun = document.getElementById("btn-run");
const btnStop = document.getElementById("btn-stop");
const btnSpawn = document.getElementById("btn-spawn-sidecar");
const progressContainer = document.getElementById("progress-container");
const progressBar = document.getElementById("progress-bar");
const progressText = document.getElementById("progress-text");
const sidecarStatus = document.getElementById("sidecar-status");
const resultsPanel = document.getElementById("results-panel");
const resultsContent = document.getElementById("results-content");
const notification = document.getElementById("notification");

// --- DOM refs (charts) ---
const btnAnalyze = document.getElementById("btn-analyze");
const chartStatus = document.getElementById("chart-status");
const statsSummary = document.getElementById("stats-summary");
const percentileChart = document.getElementById("percentile-chart");
const dpsChart = document.getElementById("dps-chart");
const damageChart = document.getElementById("damage-chart");
const anomalyChart = document.getElementById("anomaly-chart");
const damageGroupBy = document.getElementById("damage-group-by");

// --- DOM refs (export) ---
const btnExportHtml = document.getElementById("btn-export-html");
const btnExportCsv = document.getElementById("btn-export-csv");
const exportStatus = document.getElementById("export-status");

// --- DOM refs (i18n) ---
const langSelect = document.getElementById("lang-select");

// --- State ---
let sidecarRunning = false;
let simulationComplete = false;
let simOutputPath = "";
let analyzing = false;

// --- Export state ---
let chartsReady = false;

// --- Plotly.js default config ---
const PLOTLY_CONFIG = {
  scrollZoom: true,
  responsive: true,
  displayModeBar: true,
  modeBarButtonsToRemove: ["toImage"],
};

// --- Helpers ---
function setStatus(key, variant = "idle") {
  statusBadge.textContent = t(key);
  statusBadge.className = `badge badge-${variant}`;
}

function getConfig() {
  return {
    sim_count: parseInt(document.getElementById("sim-count").value, 10),
    max_tick: parseInt(document.getElementById("max-tick").value, 10),
    base_seed: parseInt(document.getElementById("base-seed").value, 10),
    data_dir: document.getElementById("data-dir").value,
    apl_file: document.getElementById("apl-file").value,
    output_path: document.getElementById("output-path").value,
  };
}

function resetUI() {
  btnRun.disabled = false;
  btnStop.disabled = true;
  progressContainer.classList.add("hidden");
  progressBar.value = 0;
  progressText.textContent = "0%";
}

function showNotification(message, type = "info") {
  notification.textContent = message;
  notification.className = `notification notification-${type}`;
  notification.classList.remove("hidden");
  setTimeout(() => notification.classList.add("hidden"), 5000);
}

// --- Chart helpers ---

/** Render a Plotly figure into a container. */
function renderPlotlyChart(container, figure) {
  if (!figure || !figure.data || figure.data.length === 0) return;
  try {
    Plotly.newPlot(container, figure.data, figure.layout || {}, PLOTLY_CONFIG);
  } catch (err) {
    console.warn("Plotly render error:", err);
  }
}

/** Render stats summary cards. */
function renderStatsSummary(stats) {
  if (!stats || Object.keys(stats).length === 0) return;

  const fields = [
    { key: "count", i18nKey: "stats.count", cls: "int" },
    { key: "mean", i18nKey: "stats.mean", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "std_dev", i18nKey: "stats.stdDev", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "min", i18nKey: "stats.min", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "p50", i18nKey: "stats.p50", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "p90", i18nKey: "stats.p90", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "p95", i18nKey: "stats.p95", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "p99", i18nKey: "stats.p99", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "max", i18nKey: "stats.max", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
  ];

  statsSummary.innerHTML = fields
    .map((f) => {
      const raw = stats[f.key];
      if (raw === undefined || raw === null) return "";
      const value = f.fmt ? f.fmt(raw) : raw;
      return `<div class="stat-card"><div class="stat-label">${t(f.i18nKey)}</div><div class="stat-value${f.cls ? " " + f.cls : ""}">${value}</div></div>`;
    })
    .join("");
  statsSummary.classList.remove("hidden");
}

/** Show a chart section by ID. */
function showChartSection(id) {
  const el = document.getElementById(id);
  if (el) el.classList.remove("hidden");
}

/** Hide a chart section by ID. */
function hideChartSection(id) {
  const el = document.getElementById(id);
  if (el) el.classList.add("hidden");
}

/** Re-render chart section titles after language change. */
function reapplyDynamicText() {
  const mappings = [
    { id: "percentile-chart-container", sel: "h3", key: "chart.percentile.title" },
    { id: "chart-section-dps", sel: "h3", key: "chart.dps.title" },
    { id: "chart-section-damage", sel: "h3", key: "chart.damage.title" },
    { id: "chart-section-anomaly", sel: "h3", key: "chart.anomaly.title" },
  ];
  for (const { id, sel, key } of mappings) {
    const section = document.getElementById(id);
    if (section && !section.classList.contains("hidden")) {
      const el = section.querySelector(sel);
      if (el) el.textContent = t(key);
    }
  }
  if (sidecarRunning) sidecarStatus.textContent = t("panel.sidecar.running");
  if (simulationComplete) {
    chartStatus.textContent = sidecarRunning
      ? t("analysis.complete")
      : t("panel.results.spawnFirst");
  }
  if (chartsReady) {
    exportStatus.textContent = t("export.statusReady");
  } else {
    exportStatus.textContent = t("export.statusDisabled");
  }
}

// --- Sidecar communication ---

/** Send a JSON command to the sidecar and return the parsed response. */
async function sendToSidecar(command) {
  const response = await invoke("send_to_sidecar", { command: JSON.stringify(command) });
  return JSON.parse(response);
}

// --- Analysis workflow ---

async function analyzeResults() {
  if (analyzing) return;
  analyzing = true;
  btnAnalyze.disabled = true;
  chartStatus.textContent = t("analysis.loading");

  const windowTicks = 60;

  try {
    // 1. Fetch summary (stats cards + percentile chart)
    chartStatus.textContent = t("analysis.loadingSummary");
    const summaryResp = await sendToSidecar({ summary: { parquet_path: simOutputPath } });
    if (summaryResp.type === "summary" && summaryResp.data) {
      const { summary, chart } = summaryResp.data;
      renderStatsSummary(summary);
      if (chart) {
        renderPlotlyChart(percentileChart, chart);
        showChartSection("percentile-chart-container");
      }
    }

    // 2. Fetch DPS curve
    chartStatus.textContent = t("analysis.loadingDps");
    const dpsResp = await sendToSidecar({ dps_curve: { parquet_path: simOutputPath, window_ticks: windowTicks } });
    if (dpsResp.type === "chart" && dpsResp.data) {
      renderPlotlyChart(dpsChart, dpsResp.data);
      showChartSection("chart-section-dps");
    }

    // 3. Fetch damage breakdown
    chartStatus.textContent = t("analysis.loadingDamage");
    const bdResp = await sendToSidecar({ damage_breakdown: { parquet_path: simOutputPath } });
    if (bdResp.type === "chart" && bdResp.data) {
      renderPlotlyChart(damageChart, bdResp.data);
      showChartSection("chart-section-damage");
    }

    // 4. Fetch anomaly timeline
    chartStatus.textContent = t("analysis.loadingAnomaly");
    const anomalyResp = await sendToSidecar({ anomaly_timeline: { parquet_path: simOutputPath } });
    if (anomalyResp.type === "chart" && anomalyResp.data) {
      renderPlotlyChart(anomalyChart, anomalyResp.data);
      showChartSection("chart-section-anomaly");
    }

    chartStatus.textContent = t("analysis.complete");
    enableExport();
    showNotification(t("notif.analysisComplete"), "success");
  } catch (err) {
    console.error("Analysis failed:", err);
    chartStatus.textContent = t("analysis.failed", { err });
    showNotification(t("analysis.failed", { err }), "error");
  } finally {
    analyzing = false;
    btnAnalyze.disabled = false;
  }
}

// --- Export workflow ---

/** Enable export buttons when charts are loaded. */
function enableExport() {
  chartsReady = true;
  btnExportHtml.disabled = false;
  btnExportCsv.disabled = false;
  exportStatus.textContent = t("export.statusReady");
}

/** Disable export buttons (e.g., when charts are reset). */
function disableExport() {
  chartsReady = false;
  btnExportHtml.disabled = true;
  btnExportCsv.disabled = true;
  exportStatus.textContent = t("export.statusDisabled");
}

/**
 * Collect all Plotly chart data from the page and export as a standalone
 * HTML file that includes embedded JSON + Plotly.js CDN.
 */
async function exportHtml() {
  const containers = [
    { id: "dps-chart", title: t("chart.dps.title") },
    { id: "damage-chart", title: t("chart.damage.title") },
    { id: "anomaly-chart", title: t("chart.anomaly.title") },
    { id: "percentile-chart", title: t("chart.percentile.title") },
  ];

  const figures = [];
  for (const { id, title } of containers) {
    const el = document.getElementById(id);
    if (!el || el.classList.contains("hidden")) continue;
    try {
      const layout = el._fullLayout ? { ...el._fullLayout } : null;
      const traces = el._fullData
        ? el._fullData.map((t) => ({
            type: t.type,
            x: t.x,
            y: t.y,
            name: t.name,
            marker: t.marker,
            line: t.line,
            text: t.text,
            hovertemplate: t.hovertemplate,
            orientation: t.orientation,
          }))
        : null;
      if (traces && traces.length > 0) {
        figures.push({
          title,
          data: traces,
          layout: layout
            ? {
                title: layout.title,
                xaxis: layout.xaxis
                  ? { title: layout.xaxis.title, dtick: layout.xaxis.dtick, tickformat: layout.xaxis.tickformat }
                  : undefined,
                yaxis: layout.yaxis
                  ? { title: layout.yaxis.title }
                  : undefined,
                width: undefined,
                height: undefined,
                paper_bgcolor: layout.paper_bgcolor,
                plot_bgcolor: layout.plot_bgcolor,
                font: layout.font,
                showlegend: layout.showlegend,
                legend: layout.legend,
                colorway: layout.colorway,
                barmode: layout.barmode,
                bargap: layout.bargap,
              }
            : {},
        });
      }
    } catch (_) { /* skip unrendered containers */ }
  }

  if (figures.length === 0) {
    exportStatus.textContent = t("export.noCharts");
    return;
  }

  const htmlContent = generateStandaloneHtml(figures);

  let filePath;
  try {
    filePath = await save({
      defaultPath: "zsim_charts.html",
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
  } catch (_) {
    return;
  }
  if (!filePath) return;

  try {
    await invoke("write_file", { path: filePath, content: htmlContent });
    exportStatus.textContent = t("export.exportedTo", { path: filePath });
    showNotification(t("notif.exportHtml"), "success");
  } catch (err) {
    exportStatus.textContent = t("export.failed");
    showNotification(t("export.failed") + ": " + err, "error");
  }
}

/** Generate a standalone HTML page with embedded chart data. */
function generateStandaloneHtml(figures) {
  const jsonData = JSON.stringify(figures);
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>ZSim Analysis Report</title>
  <script src="https://cdn.plot.ly/plotly-2.35.2.min.js"><\/script>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body { background: #0f1219; color: #e4e6f0; font-family: "Segoe UI", system-ui, sans-serif; padding: 24px; }
    h1 { font-size: 22px; margin-bottom: 8px; }
    p { color: #8b90a8; margin-bottom: 24px; font-size: 13px; }
    .chart-section { margin-bottom: 32px; }
    .chart-section h2 { font-size: 16px; margin-bottom: 12px; }
    .chart-container { width: 100%; height: 380px; background: #1a1e2b; border-radius: 8px; }
  </style>
</head>
<body>
  <h1>ZSim Analysis Report</h1>
  <p>Generated by ZSim Analyzer</p>
  <div id="charts"></div>
  <script>
    var figures = ${jsonData};
    var container = document.getElementById("charts");
    figures.forEach(function(fig, i) {
      var section = document.createElement("div");
      section.className = "chart-section";
      var title = document.createElement("h2");
      title.textContent = fig.title;
      section.appendChild(title);
      var div = document.createElement("div");
      div.className = "chart-container";
      div.id = "chart-" + i;
      section.appendChild(div);
      container.appendChild(section);
      Plotly.newPlot(div, fig.data, fig.layout, {
        scrollZoom: true, responsive: true, displayModeBar: true
      });
    });
  <\/script>
</body>
</html>`;
}

/**
 * Export stats summary as CSV.
 * Reads the stat-card values rendered by renderStatsSummary().
 */
async function exportCsv() {
  const statCards = document.querySelectorAll(".stat-card");
  if (statCards.length === 0) {
    exportStatus.textContent = t("export.noData");
    return;
  }

  const rows = [[t("stats.count"), "Value"]];
  statCards.forEach((card) => {
    const label = card.querySelector(".stat-label");
    const value = card.querySelector(".stat-value");
    if (label && value) {
      rows.push([label.textContent.trim(), value.textContent.trim().replace(/,/g, "")]);
    }
  });

  const csvContent = rows.map((r) => r.join(",")).join("\n");

  let filePath;
  try {
    filePath = await save({
      defaultPath: "zsim_summary.csv",
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
  } catch (_) {
    return;
  }
  if (!filePath) return;

  try {
    await invoke("write_file", { path: filePath, content: csvContent });
    exportStatus.textContent = t("export.exportedTo", { path: filePath });
    showNotification(t("notif.exportCsv"), "success");
  } catch (err) {
    exportStatus.textContent = t("export.failed");
    showNotification(t("export.failed") + ": " + err, "error");
  }
}

// --- Event listeners ---

// Simulation progress listener
listen("simulation-progress", (event) => {
  const { percent, current, total } = event.payload;
  progressBar.value = percent;
  progressText.textContent = t("progress.text", { percent: Math.round(percent), current, total });
}).catch((err) => console.warn("listen(simulation-progress) failed:", err));

// Simulation complete listener
listen("simulation-complete", (event) => {
  const { status, message, config } = event.payload;

  resetUI();

  if (status === "completed") {
    setStatus("status.completed", "completed");
    showNotification(t("notif.simCompleted"), "success");

    simOutputPath = (config && config.output_path) || document.getElementById("output-path").value;
    simulationComplete = true;

    resultsPanel.classList.remove("hidden");
    resultsContent.innerHTML = `<p class="text-muted">${message}</p>`;

    if (sidecarRunning) {
      btnAnalyze.disabled = false;
      chartStatus.textContent = t("analysis.complete");
    } else {
      btnAnalyze.disabled = true;
      chartStatus.textContent = t("panel.results.spawnFirst");
    }
  } else if (status === "cancelled") {
    setStatus("status.cancelled", "idle");
    showNotification(t("notif.simCancelled"), "warning");
    resetChartPanels();
  } else {
    setStatus("status.error", "error");
    showNotification(t("notif.simError", { message }), "error");
    resetChartPanels();
  }
}).catch((err) => console.warn("listen(simulation-complete) failed:", err));

function resetChartPanels() {
  disableExport();
  simulationComplete = false;
  simOutputPath = "";
  statsSummary.classList.add("hidden");
  ["percentile-chart-container", "chart-section-dps", "chart-section-damage", "chart-section-anomaly"]
    .forEach(hideChartSection);
  [percentileChart, dpsChart, damageChart, anomalyChart].forEach((el) => {
    try { Plotly.purge(el); } catch (_) { /* ignore */ }
  });
}

// Run simulation
btnRun.addEventListener("click", async () => {
  const config = getConfig();
  btnRun.disabled = true;
  btnStop.disabled = false;
  progressBar.value = 0;
  progressText.textContent = "0%";
  progressContainer.classList.remove("hidden");
  resultsPanel.classList.add("hidden");
  resetChartPanels();
  setStatus("status.running", "running");

  try {
    const result = await invoke("run_simulation", { config: JSON.stringify(config) });
    const data = JSON.parse(result);
    if (data.status !== "started") {
      resetUI();
      resultsPanel.classList.remove("hidden");
      resultsContent.innerHTML = `<pre>${JSON.stringify(data, null, 2)}</pre>`;
      setStatus("status.completed", "completed");
    }
  } catch (err) {
    resetUI();
    resultsPanel.classList.remove("hidden");
    resultsContent.innerHTML = `<p class="error">${t("sim.failed", { err })}</p>`;
    setStatus("status.error", "error");
  }
});

// Stop simulation
btnStop.addEventListener("click", async () => {
  btnStop.disabled = true;
  try {
    await invoke("stop_simulation");
    setStatus("status.stopping", "running");
  } catch (err) {
    console.warn("stop_simulation failed:", err);
    resetUI();
  }
});

// Spawn sidecar
btnSpawn.addEventListener("click", async () => {
  btnSpawn.disabled = true;
  sidecarStatus.textContent = t("panel.sidecar.spawning");

  try {
    const response = await invoke("spawn_sidecar");
    const data = JSON.parse(response);
    if (data.type === "ready") {
      sidecarRunning = true;
      sidecarStatus.textContent = t("panel.sidecar.running");
      sidecarStatus.className = "status-text status-ok";

      if (simulationComplete) {
        btnAnalyze.disabled = false;
        chartStatus.textContent = t("analysis.complete");
      }
    } else {
      sidecarStatus.textContent = t("panel.sidecar.unexpected", { response });
      sidecarStatus.className = "status-text status-error";
    }
  } catch (err) {
    sidecarRunning = false;
    sidecarStatus.textContent = t("sim.failed", { err });
    sidecarStatus.className = "status-text status-error";
  } finally {
    btnSpawn.disabled = false;
  }
});

// --- Analyze Charts button ---
btnAnalyze.addEventListener("click", analyzeResults);

// --- Damage group-by toggle ---
damageGroupBy.addEventListener("change", () => {
  const val = damageGroupBy.value;
  if (val !== "source") {
    showNotification(`"${val}" grouping not yet available`, "info");
    damageGroupBy.value = "source";
  }
});

// --- Export buttons ---
btnExportHtml.addEventListener("click", exportHtml);
btnExportCsv.addEventListener("click", exportCsv);

// --- i18n: Language switcher ---
langSelect.addEventListener("change", (e) => {
  switchLang(e.target.value);
});

// Re-apply dynamic text on language change
window.addEventListener("langchange", () => {
  reapplyDynamicText();
});

// --- Initialize ---
setStatus("status.ready");
init().then(() => {
  langSelect.value = lang;
});
