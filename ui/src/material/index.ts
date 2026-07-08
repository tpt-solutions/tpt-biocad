// Material property editor with live viscosity curve plot

import {
  Chart,
  LineController,
  LinearScale,
  PointElement,
  LineElement,
  CategoryScale,
  Title,
  Tooltip,
  Legend,
  Filler,
} from 'chart.js';

// Register Chart.js components
Chart.register(
  LineController,
  LinearScale,
  PointElement,
  LineElement,
  CategoryScale,
  Title,
  Tooltip,
  Legend,
  Filler
);

import type { RheologyModel } from '../types';
import { calculateViscosity } from '../tauri-api';

export interface MaterialEditorConfig {
  onViscosityUpdate?: (shearRates: number[], viscosities: number[]) => void;
  onMaterialChange?: (params: Record<string, number>) => void;
}

export class MaterialEditor {
  private canvas: HTMLCanvasElement;
  private chart: Chart | null = null;
  private params: Record<string, number> = {};
  private modelType: string = 'CarreauYasuda';
  private materialName: string = '';
  private density: number = 1000;
  private config: MaterialEditorConfig;
  private previewMode: 'live' | 'tauri' = 'live';

  constructor(canvas: HTMLCanvasElement, config?: MaterialEditorConfig) {
    this.canvas = canvas;
    this.config = config ?? {};
    this.initChart();
  }

  private initChart() {
    if (this.chart) {
      this.chart.destroy();
    }

    this.chart = new Chart(this.canvas, {
      type: 'line',
      data: {
        labels: [],
        datasets: [
          {
            label: 'Viscosity η (Pa·s)',
            data: [],
            borderColor: '#00d4ff',
            backgroundColor: 'rgba(0, 212, 255, 0.1)',
            fill: true,
            tension: 0.4,
            pointRadius: 2,
            pointHoverRadius: 5,
          },
          {
            label: 'Shear Stress τ (Pa)',
            data: [],
            borderColor: '#ff6464',
            backgroundColor: 'rgba(255, 100, 100, 0.05)',
            fill: false,
            tension: 0.4,
            pointRadius: 2,
            pointHoverRadius: 5,
            yAxisID: 'y1',
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        animation: { duration: 200 },
        scales: {
          x: {
            type: 'logarithmic',
            title: { display: true, text: 'Shear Rate γ̇ (1/s)', color: '#aaa' },
            grid: { color: 'rgba(255,255,255,0.05)' },
            ticks: { color: '#aaa' },
          },
          y: {
            type: 'logarithmic',
            title: { display: true, text: 'Viscosity η (Pa·s)', color: '#aaa' },
            position: 'left',
            grid: { color: 'rgba(255,255,255,0.05)' },
            ticks: { color: '#aaa' },
          },
          y1: {
            type: 'logarithmic',
            title: { display: true, text: 'Shear Stress τ (Pa)', color: '#ff6464' },
            position: 'right',
            grid: { display: false },
            ticks: { color: '#ff6464' },
          },
        },
        plugins: {
          legend: {
            labels: { color: '#ccc' },
          },
          tooltip: {
            mode: 'index',
            intersect: false,
          },
        },
      },
    });
  }

  setModelType(type: string) {
    this.modelType = type;
    this.updatePlot();
  }

  setParam(key: string, value: number) {
    this.params[key] = value;
    this.config.onMaterialChange?.(this.params);
    this.updatePlot();
  }

  setDensity(density: number) {
    this.density = density;
  }

  setMaterialName(name: string) {
    this.materialName = name;
  }

  setPreviewMode(mode: 'live' | 'tauri') {
    this.previewMode = mode;
  }

  getParams(): Record<string, number> {
    return { ...this.params };
  }

  getModel(): RheologyModel {
    switch (this.modelType) {
      case 'Newtonian':
        return { type: 'Newtonian', viscosity: this.params.viscosity ?? 1.0 };
      case 'CarreauYasuda':
        return {
          type: 'CarreauYasuda',
          etaZero: this.params.etaZero ?? 50,
          etaInf: this.params.etaInf ?? 0.05,
          lambda: this.params.lambda ?? 0.3,
          a: this.params.a ?? 0.4,
          n: this.params.n ?? 0.4,
        };
      case 'HerschelBulkley':
        return {
          type: 'HerschelBulkley',
          tauYield: this.params.tauYield ?? 50,
          k: this.params.k ?? 5,
          n: this.params.n ?? 0.5,
        };
      case 'Bingham':
        return {
          type: 'Bingham',
          tauYield: this.params.tauYield ?? 30,
          muP: this.params.muP ?? 3,
        };
      default:
        return { type: 'CarreauYasuda', etaZero: 50, etaInf: 0.05, lambda: 0.3, a: 0.4, n: 0.4 };
    }
  }

  async updatePlot() {
    const shearRates: number[] = [];
    const viscosities: number[] = [];
    const stresses: number[] = [];

    // Generate shear rate range: 0.01 to 10000
    for (let exp = -2; exp <= 4; exp += 0.1) {
      const sr = Math.pow(10, exp);
      shearRates.push(sr);
    }

    if (this.previewMode === 'live') {
      // Local computation (client-side estimate)
      const model = this.getModel();
      for (const sr of shearRates) {
        let eta = 0;
        let stress = 0;
        switch (model.type) {
          case 'Newtonian': {
            eta = model.viscosity;
            stress = eta * sr;
            break;
          }
          case 'CarreauYasuda': {
            const lg = model.lambda * sr;
            const term = Math.pow(1 + Math.pow(lg, model.a), (model.n - 1) / model.a);
            eta = model.etaInf + (model.etaZero - model.etaInf) * term;
            stress = eta * sr;
            break;
          }
          case 'HerschelBulkley': {
            if (sr === 0) { eta = Infinity; stress = 0; }
            else {
              eta = model.tauYield / sr + model.k * Math.pow(sr, model.n - 1);
              stress = model.tauYield + model.k * Math.pow(sr, model.n);
            }
            break;
          }
          case 'Bingham': {
            if (sr === 0) { eta = Infinity; stress = 0; }
            else {
              eta = model.tauYield / sr + model.muP;
              stress = model.tauYield + model.muP * sr;
            }
            break;
          }
        }
        viscosities.push(eta);
        stresses.push(stress);
      }
    } else {
      // Compute via Tauri backend
      try {
        for (const sr of shearRates) {
          const eta = await calculateViscosity(this.materialName, sr);
          viscosities.push(eta);
          stresses.push(eta * sr);
        }
      } catch {
        // Fall back to live mode
        this.previewMode = 'live';
        this.updatePlot();
        return;
      }
    }

    if (this.chart) {
      this.chart.data.labels = shearRates.map(sr => sr.toFixed(2));
      this.chart.data.datasets[0].data = viscosities;
      this.chart.data.datasets[1].data = stresses;
      this.chart.update('none');

      // Keep only every 5th label to avoid clutter
      const labels = this.chart.data.labels as string[];
      for (let i = 0; i < labels.length; i++) {
        if (i % 5 !== 0) labels[i] = '';
      }
      this.chart.update();
    }

    this.config.onViscosityUpdate?.(shearRates, viscosities);
  }

  dispose() {
    if (this.chart) {
      this.chart.destroy();
      this.chart = null;
    }
  }
}
