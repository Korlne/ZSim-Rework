import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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

// --- State ---
let sidecarRunning = false;
let simulationComplete = false;
let simOutputPath = "";
let analyzing = false;

// --- Plotly.js default config ---
const PLOTLY_CONFIG = {
  scrollZoom: true,
  responsive: true,
  displayModeBar: true,
  modeBarButtonsToRemove: ["toImage"],
};

// --- Helpers (original) ---
function setStatus(text, variant = "idle") {
  statusBadge.textContent = text;
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
  if (!figure || !figure.data || figure.data.length === 0) {
    return;
  }
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
    { key: "count", label: "Count", cls: "int" },
    { key: "mean", label: "Mean", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "std_dev", label: "Std Dev", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "min", label: "Min", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "p50", label: "P50 (Median)", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "p90", label: "P90", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "p95", label: "P95", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "p99", label: "P99", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
    { key: "max", label: "Max", fmt: (v) => v.toLocaleString(undefined, { maximumFractionDigits: 1 }) },
  ];

  statsSummary.innerHTML = fields
    .map((f) => {
      const raw = stats[f.key];
      if (raw === undefined || raw === null) return "";
      const value = f.fmt ? f.fmt(raw) : raw;
      return `<div class="stat-card"><div class="stat-label">${f.label}</div><div class="stat-value${f.cls ? " " + f.cls : ""}">${value}</div></div>`;
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
  chartStatus.textContent = "Loading charts...";

  const windowTicks = 60;

  try {
    // 1. Fetch summary (stats cards + percentile chart)
    chartStatus.textContent = "Loading summary...";
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
    chartStatus.textContent = "Loading DPS curve...";
    const dpsResp = await sendToSidecar({ dps_curve: { parquet_path: simOutputPath, window_ticks: windowTicks } });
    if (dpsResp.type === "chart" && dpsResp.data) {
      renderPlotlyChart(dpsChart, dpsResp.data);
      showChartSection("chart-section-dps");
    }

    // 3. Fetch damage breakdown
    chartStatus.textContent = "Loading damage breakdown...";
    const bdResp = await sendToSidecar({ damage_breakdown: { parquet_path: simOutputPath } });
    if (bdResp.type === "chart" && bdResp.data) {
      renderPlotlyChart(damageChart, bdResp.data);
      showChartSection("chart-section-damage");
    }

    // 4. Fetch anomaly timeline
    chartStatus.textContent = "Loading anomaly statistics...";
    const anomalyResp = await sendToSidecar({ anomaly_timeline: { parquet_path: simOutputPath } });
    if (anomalyResp.type === "chart" && anomalyResp.data) {
      renderPlotlyChart(anomalyChart, anomalyResp.data);
      showChartSection("chart-section-anomaly");
    }

    chartStatus.textContent = "Charts loaded — use zoom, pan, and hover to explore";
    showNotification("Analysis complete — charts ready", "success");
  } catch (err) {
    console.error("Analysis failed:", err);
    chartStatus.textContent = `Analysis error: ${err}`;
    showNotification(`Analysis failed: ${err}`, "error");
  } finally {
    analyzing = false;
    btnAnalyze.disabled = false;
  }
}

// --- Event listeners (original) ---

// Simulation progress listener
listen("simulation-progress", (event) => {
  const { percent, current, total } = event.payload;
  progressBar.value = percent;
  progressText.textContent = `${Math.round(percent)}% (${current}/${total})`;
}).catch((err) => console.warn("listen(simulation-progress) failed:", err));

// Simulation complete listener
listen("simulation-complete", (event) => {
  const { status, message, config } = event.payload;

  resetUI();

  if (status === "completed") {
    setStatus("Completed", "completed");
    showNotification("Simulation completed successfully!", "success");

    // Store output path from config
    simOutputPath = (config && config.output_path) || document.getElementById("output-path").value;
    simulationComplete = true;

    // Show results panel with instructions
    resultsPanel.classList.remove("hidden");
    resultsContent.innerHTML = `<p class="text-muted">${message}</p>`;

    // Enable analyze button if sidecar is running
    if (sidecarRunning) {
      btnAnalyze.disabled = false;
      chartStatus.textContent = "Ready to analyze — click 'Analyze Charts'";
    } else {
      btnAnalyze.disabled = true;
      chartStatus.textContent = "Spawn sidecar first, then analyze";
    }
  } else if (status === "cancelled") {
    setStatus("Cancelled", "idle");
    showNotification("Simulation was cancelled.", "warning");
    resetChartPanels();
  } else {
    setStatus("Error", "error");
    showNotification(`Simulation error: ${message}`, "error");
    resetChartPanels();
  }
}).catch((err) => console.warn("listen(simulation-complete) failed:", err));

function resetChartPanels() {
  simulationComplete = false;
  simOutputPath = "";
  statsSummary.classList.add("hidden");
  ["percentile-chart-container", "chart-section-dps", "chart-section-damage", "chart-section-anomaly"]
    .forEach(hideChartSection);
  // Clear Plotly charts
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
  setStatus("Running", "running");

  try {
    const result = await invoke("run_simulation", { config: JSON.stringify(config) });
    const data = JSON.parse(result);
    if (data.status !== "started") {
      resetUI();
      resultsPanel.classList.remove("hidden");
      resultsContent.innerHTML = `<pre>${JSON.stringify(data, null, 2)}</pre>`;
      setStatus("Completed", "completed");
    }
  } catch (err) {
    resetUI();
    resultsPanel.classList.remove("hidden");
    resultsContent.innerHTML = `<p class="error">Simulation failed: ${err}</p>`;
    setStatus("Error", "error");
  }
});

// Stop simulation
btnStop.addEventListener("click", async () => {
  btnStop.disabled = true;
  try {
    await invoke("stop_simulation");
    setStatus("Stopping...", "running");
  } catch (err) {
    console.warn("stop_simulation failed:", err);
    resetUI();
  }
});

// Spawn sidecar
btnSpawn.addEventListener("click", async () => {
  btnSpawn.disabled = true;
  sidecarStatus.textContent = "Spawning...";

  try {
    const response = await invoke("spawn_sidecar");
    const data = JSON.parse(response);
    if (data.type === "ready") {
      sidecarRunning = true;
      sidecarStatus.textContent = "Running";
      sidecarStatus.className = "status-text status-ok";

      // Enable analyze button if simulation is complete
      if (simulationComplete) {
        btnAnalyze.disabled = false;
        chartStatus.textContent = "Ready to analyze — click 'Analyze Charts'";
      }
    } else {
      sidecarStatus.textContent = `Unexpected: ${response}`;
      sidecarStatus.className = "status-text status-error";
    }
  } catch (err) {
    sidecarRunning = false;
    sidecarStatus.textContent = `Error: ${err}`;
    sidecarStatus.className = "status-text status-error";
  } finally {
    btnSpawn.disabled = false;
  }
});

// --- Analyze Charts button ---
btnAnalyze.addEventListener("click", analyzeResults);

// --- Damage group-by toggle ---
damageGroupBy.addEventListener("change", () => {
  // Currently only "source" grouping is available from the sidecar.
  // Element and Action Type groupings require new AggQuery variants.
  // For now, this is a placeholder for future expansion.
  const val = damageGroupBy.value;
  if (val !== "source") {
    showNotification(`"${val}" grouping not yet available`, "info");
    damageGroupBy.value = "source";
  }
});

// --- Initial status ---
setStatus("Ready");
