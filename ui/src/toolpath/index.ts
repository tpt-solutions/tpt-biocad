// Toolpath preview — layer-by-layer animation and controls

import { Viewport3D } from '../viewport';
import type { SliceLayer } from '../types';

export interface ToolpathAnimatorOptions {
  speed: number;
  onLayerChange?: (layer: number, total: number) => void;
  onComplete?: () => void;
}

export class ToolpathAnimator {
  private viewport: Viewport3D;
  private layers: SliceLayer[] = [];
  private currentLayer: number = 0;
  private speed: number = 1;
  private playing: boolean = false;
  private timerId: number | null = null;
  private onLayerChange?: (layer: number, total: number) => void;
  private onComplete?: () => void;

  constructor(viewport: Viewport3D) {
    this.viewport = viewport;
  }

  loadLayers(layers: SliceLayer[], options?: ToolpathAnimatorOptions) {
    this.layers = layers;
    this.currentLayer = 0;
    this.speed = options?.speed ?? 1;
    this.onLayerChange = options?.onLayerChange;
    this.onComplete = options?.onComplete;

    this.viewport.clearToolpath();

    for (let i = 0; i < layers.length; i++) {
      const layer = layers[i];
      for (const poly of layer.polygons) {
        this.viewport.loadToolpathLayer(poly, i, layers.length);
      }
    }

    this.viewport.setCurrentLayer(0);
    this.viewport.fitCamera();
    this.onLayerChange?.(0, layers.length);
  }

  play() {
    if (this.playing || this.layers.length === 0) return;
    this.playing = true;
    this.scheduleNext();
  }

  pause() {
    this.playing = false;
    if (this.timerId !== null) {
      clearTimeout(this.timerId);
      this.timerId = null;
    }
  }

  toggle() {
    if (this.playing) this.pause();
    else this.play();
  }

  goToLayer(layer: number) {
    this.currentLayer = Math.max(0, Math.min(layer, this.layers.length - 1));
    this.viewport.setCurrentLayer(this.currentLayer);
    this.onLayerChange?.(this.currentLayer, this.layers.length);
  }

  nextLayer() {
    this.goToLayer(this.currentLayer + 1);
  }

  prevLayer() {
    this.goToLayer(this.currentLayer - 1);
  }

  setSpeed(speed: number) {
    this.speed = speed;
  }

  private scheduleNext() {
    if (!this.playing) return;

    this.viewport.setCurrentLayer(this.currentLayer);
    this.onLayerChange?.(this.currentLayer, this.layers.length);

    if (this.currentLayer >= this.layers.length - 1) {
      this.playing = false;
      this.onComplete?.();
      return;
    }

    this.currentLayer++;
    const delay = Math.max(50, 500 / this.speed);
    this.timerId = window.setTimeout(() => this.scheduleNext(), delay);
  }

  get isPlaying(): boolean {
    return this.playing;
  }

  get layer(): number {
    return this.currentLayer;
  }

  get totalLayers(): number {
    return this.layers.length;
  }

  dispose() {
    this.pause();
    this.layers = [];
  }
}
