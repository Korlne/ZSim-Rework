import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// --- DOM refs ---
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

// --- Helpers ---
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

// --- Simulation progress listener ---
listen("simulation-progress", (event) => {
  const { percent, current, total } = event.payload;
  progressBar.value = percent;
  progressText.textContent = `${Math.round(percent)}% (${current}/${total})`;
}).catch((err) => console.warn("listen(simulation-progress) failed:", err));

// --- Run simulation ---
btnRun.addEventListener("click", async () => {
  const config = getConfig();
  btnRun.disabled = true;
  btnStop.disabled = false;
  progressContainer.classList.remove("hidden");
  setStatus("Running", "running");

  try {
    const result = await invoke("run_simulation", { config: JSON.stringify(config) });
    const data = JSON.parse(result);
    resultsPanel.classList.remove("hidden");
    resultsContent.innerHTML = `<pre>${JSON.stringify(data, null, 2)}</pre>`;
    setStatus("Completed", "completed");
  } catch (err) {
    resultsPanel.classList.remove("hidden");
    resultsContent.innerHTML = `<p class="error">Simulation failed: ${err}</p>`;
    setStatus("Error", "error");
  } finally {
    btnRun.disabled = false;
    btnStop.disabled = true;
  }
});

// --- Stop simulation ---
btnStop.addEventListener("click", () => {
  // For scaffold: requires IPC mechanism for cancellation
  setStatus("Stopped", "idle");
  btnRun.disabled = false;
  btnStop.disabled = true;
});

// --- Spawn sidecar ---
btnSpawn.addEventListener("click", async () => {
  btnSpawn.disabled = true;
  sidecarStatus.textContent = "Spawning...";

  try {
    const response = await invoke("spawn_sidecar");
    const data = JSON.parse(response);
    if (data.type === "ready") {
      sidecarStatus.textContent = "Running";
      sidecarStatus.className = "status-text status-ok";
    } else {
      sidecarStatus.textContent = `Unexpected: ${response}`;
    }
  } catch (err) {
    sidecarStatus.textContent = `Error: ${err}`;
    sidecarStatus.className = "status-text status-error";
  } finally {
    btnSpawn.disabled = false;
  }
});

// --- Initial status ---
setStatus("Ready");
