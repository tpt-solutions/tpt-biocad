// Shared TypeScript types matching Rust structs

export interface MaterialInfo {
  name: string;
  density: number;
  uvCuring: boolean;
  coaxial: boolean;
}

export interface ViscosityResult {
  viscosity: number;
  shearStress: number;
}

export interface PressureResult {
  pressureDrop: number;
  wallShearStress: number;
}

export interface SlumpInfo {
  widthAfter: number;
  heightAfter: number;
  settlingTime: number;
  slumpFactor: number;
}

export interface RheologyParams {
  etaZero: number;
  etaInf: number;
  lambda: number;
  a: number;
  n: number;
}

export interface Material {
  name: string;
  density: number;
  rheology: RheologyModel;
  curing: CuringParams | null;
  coaxial: CoaxialParams | null;
}

export type RheologyModel =
  | { type: 'Newtonian'; viscosity: number }
  | { type: 'CarreauYasuda'; etaZero: number; etaInf: number; lambda: number; a: number; n: number }
  | { type: 'HerschelBulkley'; tauYield: number; k: number; n: number }
  | { type: 'Bingham'; tauYield: number; muP: number };

export interface CuringParams {
  uvIntensity: number;
  exposureTime: number;
  wavelength: number;
}

export interface CoaxialParams {
  crosslinkerName: string;
  flowRatio: number;
  concentration: number;
}

// Toolpath types for 3D preview

export interface ToolpathPoint {
  x: number;
  y: number;
  z: number;
  e?: number | null;
}

export interface ToolpathSegment {
  start: ToolpathPoint;
  end: ToolpathPoint;
  rapid: boolean;
  layer: number;
}

export interface SliceLayer {
  z: number;
  polygons: ToolpathPoint[][];
}

export interface SliceResult {
  layers: SliceLayer[];
  commands: number;
  printTimeSeconds: number;
  totalLength: number;
}

// G-code types

export interface GCodeLine {
  number: number;
  text: string;
  type: 'G0' | 'G1' | 'G28' | 'M104' | 'M109' | 'M300' | 'M301' | 'M302' | 'M303' | 'T' | 'comment' | 'other';
}

// Undo/redo types

export interface UndoState {
  id: number;
  label: string;
  timestamp: number;
}

// 3D viewport state

export interface ViewportState {
  showMesh: boolean;
  showToolpath: boolean;
  showSlumping: boolean;
  currentLayer: number;
  totalLayers: number;
  animationSpeed: number;
  isPlaying: boolean;
}

// Phase 7: Intelligence & Automation Types

export interface RecommendationInfo {
  nozzleDiameter: number;
  layerHeight: number;
  printSpeed: number;
  flowRate: number;
  pressure: number;
  temperature: number | null;
  confidence: number;
  reasoning: string[];
}

export interface SimulationLayerInfo {
  z: number;
  widthAfter: number;
  heightAfter: number;
  verticalDisplacement: number;
  slumpFactor: number;
  settled: boolean;
}

export interface SimulationResultInfo {
  layers: SimulationLayerInfo[];
  finalHeight: number;
  maxWidthIncrease: number;
  maxDisplacement: number;
  shapeRetention: number;
}

export interface FeedbackActionInfo {
  pressureDelta: number;
  temperatureDelta: number;
  speedMultiplier: number;
  abort: boolean;
  message: string | null;
}

export interface JobInfo {
  id: string;
  name: string;
  priority: string;
  status: string;
  materialName: string;
  estimatedTimeS: number;
  description: string;
}

export interface QualityAlertInfo {
  defectType: string;
  severity: string;
  message: string;
  timestampS: number;
  location: [number, number, number] | null;
}
