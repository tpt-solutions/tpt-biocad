// TPT BioCAD — Phase 6 entry point

import { Viewport3D } from './viewport';
import { ToolpathAnimator } from './toolpath';
import { MaterialEditor } from './material';
import { GCodePreview } from './gcode';
import { commandHistory, Command } from './undo';
import {
  getMaterials,
  getMaterialInfo,
  calculateSlump,
  importMaterialCsv,
} from './tauri-api';

// --- Global state ---
let selectedMaterial = '';
let viewport: Viewport3D | null = null;
let toolpathAnimator: ToolpathAnimator | null = null;
let materialEditor: MaterialEditor | null = null;
let gcodePreview: GCodePreview | null = null;

// --- Init ---
window.addEventListener('DOMContentLoaded', async () => {
  initMaterialList();
  initViewport();
  initMaterialEditor();
  initGCodePreview();
  initUndoRedo();
  initTabSystem();
});

// --- Tab System ---
function initTabSystem() {
  const tabs = document.querySelectorAll('.tab-btn');
  const panels = document.querySelectorAll('.tab-panel');

  tabs.forEach((tab) => {
    tab.addEventListener('click', () => {
      const target = (tab as HTMLElement).dataset.tab;
      tabs.forEach((t) => t.classList.remove('active'));
      panels.forEach((p) => p.classList.remove('active'));
      tab.classList.add('active');
      const panel = document.getElementById(`panel-${target}`);
      if (panel) panel.classList.add('active');
      // Resize viewport when its tab is shown
      if (target === 'viewport' && viewport) {
        setTimeout(() => viewport!.resize(), 50);
      }
    });
  });
}

// --- Material List ---
async function initMaterialList() {
  try {
    const materials = await getMaterials();
    const container = document.getElementById('materials');
    if (!container) return;
    container.innerHTML = '';
    materials.forEach((m: string) => {
      const div = document.createElement('div');
      div.className = 'material-item';
      div.textContent = m;
      div.dataset.material = m;
      div.addEventListener('click', () => selectMaterial(m));
      container.appendChild(div);
    });
  } catch {
    const container = document.getElementById('materials');
    if (container) container.textContent = 'Error loading materials';
  }
}

async function selectMaterial(name: string) {
  selectedMaterial = name;
  document.querySelectorAll('.material-item').forEach((el) => {
    (el as HTMLElement).style.border =
      el.textContent === name ? '2px solid #00d4ff' : 'none';
  });

  // Update material editor with selected material parameters
  try {
    const info = await getMaterialInfo(name);
    const densityInput = document.getElementById('mat-density') as HTMLInputElement;
    if (densityInput) densityInput.value = String(info.density);
    if (materialEditor) {
      materialEditor.setMaterialName(name);
      materialEditor.setDensity(info.density);
    }
  } catch {
    // ignore
  }
}

// --- 3D Viewport ---
function initViewport() {
  const container = document.getElementById('viewport-container');
  if (!container) return;
  viewport = new Viewport3D(container);
  toolpathAnimator = new ToolpathAnimator(viewport);
}

// Expose for HTML onclick handlers
(window as any).loadDemoMesh = function () {
  if (!viewport) return;
  // Create a demo mesh (pyramid)
  const vertices: number[][] = [
    [0, 0, 0],
    [2, 0, 0],
    [2, 2, 0],
    [0, 2, 0],
    [1, 1, 2],
  ];
  const triangles: number[][] = [
    [0, 1, 4],
    [1, 2, 4],
    [2, 3, 4],
    [3, 0, 4],
    [0, 1, 2],
    [0, 2, 3],
  ];
  viewport.loadMeshFromArrays(vertices, triangles);
};

(window as any).demoToolpath = function () {
  if (!viewport || !toolpathAnimator) return;
  // Generate demo toolpath layers
  const layers = [];
  for (let z = 0; z < 5; z += 0.5) {
    const poly: Array<{ x: number; y: number; z: number; e: null }> = [];
    const r = 5 - z * 0.5;
    for (let i = 0; i <= 64; i++) {
      const angle = (i / 64) * Math.PI * 2;
      poly.push({
        x: 10 + r * Math.cos(angle),
        y: 10 + r * Math.sin(angle),
        z: z,
        e: null,
      });
    }
    layers.push({ z, polygons: [poly] });
  }
  toolpathAnimator.loadLayers(layers, {
    speed: 2,
    onLayerChange: (layer, total) => {
      const el = document.getElementById('layer-indicator');
      if (el) el.textContent = `Layer ${layer + 1} / ${total}`;
    },
  });
};

(window as any).playToolpath = function () {
  toolpathAnimator?.toggle();
};

(window as any).resetViewport = function () {
  viewport?.resetCamera();
};

// --- Slumping Controls ---
(window as any).calculateSlump3D = async function () {
  if (!selectedMaterial || !viewport) {
    alert('Please select a material first');
    return;
  }

  const width = parseFloat((document.getElementById('slump-width') as HTMLInputElement)?.value || '0.4');
  const height = parseFloat((document.getElementById('slump-height') as HTMLInputElement)?.value || '0.2');
  const speed = parseFloat((document.getElementById('slump-speed') as HTMLInputElement)?.value || '10');
  const layerTime = parseFloat((document.getElementById('slump-time') as HTMLInputElement)?.value || '1.0');

  try {
    const result = await calculateSlump(selectedMaterial, width, height, speed, layerTime);
    const pct = (result.slumpFactor * 100).toFixed(1);
    document.getElementById('slump-3d-results')!.innerHTML = `
      <p>Slump factor: ${pct}%</p>
      <p>Width: ${width.toFixed(2)} → ${result.widthAfter.toFixed(2)} mm</p>
      <p>Height: ${height.toFixed(2)} → ${result.heightAfter.toFixed(2)} mm</p>
      <p>Settling time: ${result.settlingTime === Infinity ? '∞' : result.settlingTime.toFixed(2) + ' s'}</p>
    `;

    // Show slumping overlay on viewport
    viewport.clearSlumping();
    viewport.addSlumpBead(10, 10, 0, result.widthAfter, result.heightAfter, result.slumpFactor);
    viewport.addSlumpBead(10, 10.5, 0.2, result.widthAfter * 0.9, result.heightAfter * 0.9, result.slumpFactor * 0.8);
    viewport.state = { ...viewport.state, showSlumping: true };
    viewport.fitCamera();
  } catch (e) {
    document.getElementById('slump-3d-results')!.innerHTML = `<p style="color:red">Error: ${e}</p>`;
  }
};

// --- Material Editor ---
function initMaterialEditor() {
  const canvas = document.getElementById('viscosity-chart') as HTMLCanvasElement;
  if (!canvas) return;
  materialEditor = new MaterialEditor(canvas, {
    onViscosityUpdate: (rates, vics) => {
      // Update viscosity readout
      const el = document.getElementById('viscosity-at-shear');
      if (el && rates.length > 0) {
        const midIdx = Math.floor(rates.length / 2);
        el.textContent = `η at γ̇=${rates[midIdx].toFixed(1)}/s: ${vics[midIdx].toFixed(4)} Pa·s`;
      }
    },
  });

  // Wire up parameter inputs
  document.querySelectorAll('.mat-param-input').forEach((input) => {
    input.addEventListener('input', () => {
      const el = input as HTMLInputElement;
      const key = el.dataset.param;
      const val = parseFloat(el.value);
      if (key && !isNaN(val)) {
        materialEditor?.setParam(key, val);
      }
    });
  });

  // Wire up model type selector
  const modelSelect = document.getElementById('model-type') as HTMLSelectElement;
  if (modelSelect) {
    modelSelect.addEventListener('change', () => {
      materialEditor?.setModelType(modelSelect.value);
      showRelevantParams(modelSelect.value);
    });
  }

  // Initial plot
  setTimeout(() => materialEditor?.updatePlot(), 100);
}

function showRelevantParams(modelType: string) {
  document.querySelectorAll('.param-group').forEach((el) => {
    (el as HTMLElement).style.display = 'none';
  });
  const group = document.getElementById(`params-${modelType}`);
  if (group) group.style.display = 'block';
}

// --- G-code Preview ---
function initGCodePreview() {
  const container = document.getElementById('gcode-container');
  if (!container) return;

  gcodePreview = new GCodePreview(container, {
    onLineSelect: (idx, line) => {
      document.getElementById('gcode-line-info')!.textContent =
        `Line ${idx + 1}: ${line.text.substring(0, 60)}`;
    },
  });

  // Set demo code
  gcodePreview.setCode(`; TPT BioCAD G-code
; Layer-by-layer bioprinting toolpath
G21 ; mm mode
G90 ; absolute positioning
G28 ; home all axes

; Layer 1 (z=0.20)
G0 Z0.2000 F1500
M302 U10 T30 ; UV curing
G1 X1.0000 Y0.0000 Z0.2000 E0.000100 F900
G1 X1.0000 Y0.5000 Z0.2000 E0.000150 F900
G1 X0.5000 Y0.5000 Z0.2000 E0.000200 F900
G1 X0.5000 Y0.0000 Z0.2000 E0.000100 F900
G1 X0.0000 Y0.0000 Z0.2000 E0.000050 F900

; Layer 2 (z=0.40)
G0 Z0.4000 F1500
M302 U10 T30
G1 X1.0000 Y0.0000 Z0.4000 E0.000100 F900
G1 X1.0000 Y0.5000 Z0.4000 E0.000150 F900
G1 X0.5000 Y0.5000 Z0.4000 E0.000200 F900
G1 X0.5000 Y0.0000 Z0.4000 E0.000100 F900

; Print complete
G0 Z10.0000 F1500
M104 S0 ; nozzle off
M140 S0 ; bed off`);
}

(window as any).gcodeStepForward = function () {
  gcodePreview?.stepForward();
};

(window as any).gcodeStepBackward = function () {
  gcodePreview?.stepBackward();
};

(window as any).gcodeGoToLine = function () {
  const input = document.getElementById('gcode-goto') as HTMLInputElement;
  if (input && gcodePreview) {
    const line = parseInt(input.value) - 1;
    if (!isNaN(line)) gcodePreview.goToLine(line);
  }
};

// --- Undo/Redo ---
function initUndoRedo() {
  const undoBtn = document.getElementById('btn-undo');
  const redoBtn = document.getElementById('btn-redo');

  commandHistory.subscribe(() => {
    if (undoBtn) undoBtn.classList.toggle('disabled', !commandHistory.canUndo);
    if (redoBtn) redoBtn.classList.toggle('disabled', !commandHistory.canRedo);
  });

  if (undoBtn) {
    undoBtn.addEventListener('click', async () => {
      const cmd = await commandHistory.undo();
      if (cmd) updateUndoRedoDisplay();
    });
  }

  if (redoBtn) {
    redoBtn.addEventListener('click', async () => {
      const cmd = await commandHistory.redo();
      if (cmd) updateUndoRedoDisplay();
    });
  }
}

function updateUndoRedoDisplay() {
  const el = document.getElementById('undo-info');
  if (el) {
    el.textContent = `Undo: ${commandHistory.history.length} action(s)`;
  }
}

// Helper to wrap operations as commands
(window as any).executeWithUndo = async function (
  label: string,
  executeFn: () => Promise<void>,
  undoFn: () => Promise<void>
) {
  await commandHistory.execute({
    label,
    execute: executeFn,
    undo: undoFn,
  });
  updateUndoRedoDisplay();
};

// ===== Phase 7: Intelligence & Automation =====

// --- 1. AI Optimizer ---
(window as any).runOptimizer = async function () {
  if (!selectedMaterial) {
    alert('Please select a material first');
    return;
  }

  const volume = parseFloat((document.getElementById('opt-volume') as HTMLInputElement)?.value || '1000');
  const surfaceArea = parseFloat((document.getElementById('opt-surface') as HTMLInputElement)?.value || '600');
  const height = parseFloat((document.getElementById('opt-height') as HTMLInputElement)?.value || '20');
  const minFeature = parseFloat((document.getElementById('opt-feature') as HTMLInputElement)?.value || '1.0');
  const overhang = parseFloat((document.getElementById('opt-overhang') as HTMLInputElement)?.value || '45');

  try {
    const { recommendParameters } = await import('./tauri-api');
    const rec = await recommendParameters(selectedMaterial, volume, surfaceArea, height, minFeature, overhang);

    const el = document.getElementById('opt-results')!;
    const pct = (rec.confidence * 100).toFixed(0);
    const tempStr = rec.temperature !== null ? `${rec.temperature}°C` : 'none';
    el.innerHTML = `
      <p style="color:#00d4ff;font-weight:bold">Recommended Parameters (confidence: ${pct}%)</p>
      <p>🔧 Nozzle Ø: ${rec.nozzleDiameter.toFixed(2)} mm</p>
      <p>📐 Layer Height: ${rec.layerHeight.toFixed(2)} mm</p>
      <p>⚡ Print Speed: ${rec.printSpeed.toFixed(0)} mm/s</p>
      <p>💧 Flow Rate: ${rec.flowRate.toFixed(3)} mm³/s</p>
      <p>📊 Pressure: ${rec.pressure.toFixed(0)} kPa</p>
      <p>🌡️ Temperature: ${tempStr}</p>
      <hr style="border-color:#0f3460;margin:6px 0">
      <p style="font-size:11px;color:#8b949e"><strong>Reasoning:</strong></p>
      <ul style="font-size:11px;color:#8b949e;margin:2px 0;padding-left:16px">
        ${rec.reasoning.map((r: string) => `<li>${r}</li>`).join('')}
      </ul>
    `;
  } catch (e) {
    document.getElementById('opt-results')!.innerHTML = `<p style="color:red">Error: ${e}</p>`;
  }
};

// --- 2. Simulation ---
(window as any).runSimulation = async function () {
  if (!selectedMaterial) {
    alert('Please select a material first');
    return;
  }

  const width = parseFloat((document.getElementById('sim-width') as HTMLInputElement)?.value || '0.4');
  const height = parseFloat((document.getElementById('sim-height') as HTMLInputElement)?.value || '0.2');
  const layerTime = parseFloat((document.getElementById('sim-time') as HTMLInputElement)?.value || '2.0');
  const speed = parseFloat((document.getElementById('sim-speed') as HTMLInputElement)?.value || '10');
  const layerCount = parseInt((document.getElementById('sim-layers') as HTMLInputElement)?.value || '10');

  try {
    const { simulateLayers } = await import('./tauri-api');
    const result = await simulateLayers(selectedMaterial, width, height, layerTime, speed, layerCount);

    const el = document.getElementById('sim-results')!;
    const retentionPct = (result.shapeRetention * 100).toFixed(1);
    const settledCount = result.layers.filter((l: any) => l.settled).length;

    let tableRows = result.layers.slice(0, 20).map((l: any, i: number) => `
      <tr>
        <td style="padding:2px 4px;border:1px solid #0f3460">${i + 1}</td>
        <td style="padding:2px 4px;border:1px solid #0f3460">${l.widthAfter.toFixed(2)}</td>
        <td style="padding:2px 4px;border:1px solid #0f3460">${l.heightAfter.toFixed(2)}</td>
        <td style="padding:2px 4px;border:1px solid #0f3460">${l.verticalDisplacement.toFixed(3)}</td>
        <td style="padding:2px 4px;border:1px solid #0f3460">${(l.slumpFactor * 100).toFixed(0)}%</td>
        <td style="padding:2px 4px;border:1px solid #0f3460">${l.settled ? '✓' : '—'}</td>
      </tr>
    `).join('');

    el.innerHTML = `
      <p style="color:#00d4ff;font-weight:bold">Simulation Results</p>
      <p>Shape Retention: ${retentionPct}% | Final Height: ${result.finalHeight.toFixed(2)}mm | Max Width Increase: ${result.maxWidthIncrease.toFixed(2)}mm | Settled Layers: ${settledCount}/${result.layers.length}</p>
      <hr style="border-color:#0f3460;margin:6px 0">
      <table style="width:100%;font-size:11px;color:#8b949e;border-collapse:collapse">
        <tr style="color:#00d4ff">
          <th style="padding:2px 4px;border:1px solid #0f3460">Layer</th>
          <th style="padding:2px 4px;border:1px solid #0f3460">Width</th>
          <th style="padding:2px 4px;border:1px solid #0f3460">Height</th>
          <th style="padding:2px 4px;border:1px solid #0f3460">Disp.</th>
          <th style="padding:2px 4px;border:1px solid #0f3460">Slump</th>
          <th style="padding:2px 4px;border:1px solid #0f3460">Set.</th>
        </tr>
        ${tableRows}
        ${result.layers.length > 20 ? `<tr><td colspan="6" style="text-align:center;padding:4px;color:#8b949e">... and ${result.layers.length - 20} more layers</td></tr>` : ''}
      </table>
    `;

    // Also update viewport if available
    if (viewport) {
      viewport.clearSlumping();
      result.layers.forEach((l: any, i: number) => {
        viewport!.addSlumpBead(
          i * 2, 0, l.z,
          l.widthAfter, l.heightAfter,
          l.slumpFactor
        );
      });
      viewport.state = { ...viewport.state, showSlumping: true };
    }
  } catch (e) {
    document.getElementById('sim-results')!.innerHTML = `<p style="color:red">Error: ${e}</p>`;
  }
};

// --- 3. Print Queue ---
(window as any).enqueueDemoJob = async function () {
  const nameInput = document.getElementById('queue-job-name') as HTMLInputElement;
  const prioritySelect = document.getElementById('queue-priority') as HTMLSelectElement;
  const name = nameInput?.value || 'Demo Job';
  const priority = prioritySelect?.value || 'normal';

  try {
    const { queueEnqueue, generateGcode } = await import('./tauri-api');
    const demoGcode = 'G28\nG1 X10 Y10 Z0.2 F900\nG1 X90 Y10 Z0.2\nG1 X90 Y90 Z0.2\nG1 X10 Y90 Z0.2\n';
    const id = await queueEnqueue(name, demoGcode, selectedMaterial || 'Unknown', 120, priority);
    refreshQueue();
    if (nameInput) nameInput.value = '';
  } catch (e) {
    console.error('Failed to enqueue job:', e);
  }
};

(window as any).startNextJob = async function () {
  try {
    const { queueStartNext } = await import('./tauri-api');
    const job = await queueStartNext();
    if (job) {
      document.getElementById('queue-results')!.innerHTML =
        `<p style="color:#00d4ff">▶ Started: ${job.name} (${job.priority})</p>`;
    } else {
      document.getElementById('queue-results')!.innerHTML =
        `<p style="color:#8b949e">No queued jobs to start</p>`;
    }
    refreshQueue();
  } catch (e) {
    console.error(e);
  }
};

(window as any).completeJob = async function () {
  try {
    const { queueCompleteCurrent } = await import('./tauri-api');
    const id = await queueCompleteCurrent();
    if (id) {
      document.getElementById('queue-results')!.innerHTML =
        `<p style="color:#00d4ff">✓ Completed job ${id}</p>`;
    }
    refreshQueue();
  } catch (e) {
    console.error(e);
  }
};

async function refreshQueue() {
  try {
    const { queueList } = await import('./tauri-api');
    const jobs = await queueList();
    const el = document.getElementById('queue-results')!;
    if (jobs.length === 0) {
      el.innerHTML = `<p>Queue is empty.</p>`;
      return;
    }
    let table = `<table style="width:100%;font-size:11px;color:#8b949e;border-collapse:collapse">
      <tr style="color:#00d4ff">
        <th style="padding:2px 4px;border:1px solid #0f3460">Name</th>
        <th style="padding:2px 4px;border:1px solid #0f3460">Priority</th>
        <th style="padding:2px 4px;border:1px solid #0f3460">Status</th>
        <th style="padding:2px 4px;border:1px solid #0f3460">Material</th>
        <th style="padding:2px 4px;border:1px solid #0f3460">Est. Time</th>
      </tr>`;
    jobs.forEach((j: any) => {
      const statusColor = j.status === 'printing' ? '#00d4ff' : j.status === 'completed' ? '#4caf50' : j.status.includes('fail') ? '#f44336' : '#8b949e';
      table += `<tr>
        <td style="padding:2px 4px;border:1px solid #0f3460">${j.name}</td>
        <td style="padding:2px 4px;border:1px solid #0f3460">${j.priority}</td>
        <td style="padding:2px 4px;border:1px solid #0f3460;color:${statusColor}">${j.status}</td>
        <td style="padding:2px 4px;border:1px solid #0f3460">${j.materialName}</td>
        <td style="padding:2px 4px;border:1px solid #0f3460">${j.estimatedTimeS.toFixed(0)}s</td>
      </tr>`;
    });
    table += `</table>`;
    el.innerHTML = table;
  } catch (e) {
    console.error(e);
  }
}

(window as any).refreshQueue = refreshQueue;

// --- 4. Quality Monitor ---
let qualityTime = 0;

(window as any).feedNormalSample = async function () {
  qualityTime += 0.5;
  try {
    const { qualityFeed, qualityScore } = await import('./tauri-api');
    const alerts = await qualityFeed(100, 37, 1.0, qualityTime * 10, 0, 0.2, qualityTime * 1.0, qualityTime);
    const score = await qualityScore();
    updateQualityDisplay(alerts, score);
  } catch (e) {
    console.error(e);
  }
};

(window as any).feedUnderExtrusion = async function () {
  qualityTime += 0.5;
  try {
    const { qualityFeed, qualityScore } = await import('./tauri-api');
    const alerts = await qualityFeed(50, 37, 0.1, qualityTime * 10, 0, 0.2, qualityTime * 0.05, qualityTime);
    const score = await qualityScore();
    updateQualityDisplay(alerts, score);
  } catch (e) {
    console.error(e);
  }
};

(window as any).feedPressureSpike = async function () {
  qualityTime += 0.5;
  try {
    const { qualityFeed, qualityScore } = await import('./tauri-api');
    const alerts = await qualityFeed(350, 37, 1.0, qualityTime * 10, 0, 0.2, qualityTime * 1.0, qualityTime);
    const score = await qualityScore();
    updateQualityDisplay(alerts, score);
  } catch (e) {
    console.error(e);
  }
};

(window as any).resetQuality = async function () {
  qualityTime = 0;
  try {
    const { qualityReset, qualityScore } = await import('./tauri-api');
    await qualityReset();
    const score = await qualityScore();
    document.getElementById('quality-score-display')!.textContent = score.toFixed(2);
    document.getElementById('quality-results')!.innerHTML = '<p>Monitor reset. No alerts.</p>';
  } catch (e) {
    console.error(e);
  }
};

async function updateQualityDisplay(alerts: any[], score: number) {
  document.getElementById('quality-score-display')!.textContent = score.toFixed(2);

  const el = document.getElementById('quality-results')!;
  if (alerts.length === 0) {
    el.innerHTML = `<p style="color:#4caf50">✓ No defects detected.</p>`;
    return;
  }

  let html = `<p style="color:#ff9800">⚠ ${alerts.length} new alert(s):</p>`;
  alerts.forEach((a: any) => {
    const color = a.severity === 'Critical' ? '#f44336' : a.severity === 'Warning' ? '#ff9800' : '#8b949e';
    const loc = a.location ? ` @ (${a.location[0].toFixed(1)}, ${a.location[1].toFixed(1)}, ${a.location[2].toFixed(1)})` : '';
    html += `<p style="color:${color};font-size:11px;margin:2px 0">• [${a.severity}] ${a.defectType}: ${a.message}${loc}</p>`;
  });

  el.innerHTML = html;
}
