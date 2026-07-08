// Tauri invoke wrapper — typed API calls to Rust backend

const { invoke } = window.__TAURI__;

export async function getMaterials(): Promise<string[]> {
  return invoke('get_materials');
}

export async function getMaterialInfo(name: string): Promise<{
  name: string;
  density: number;
  uvCuring: boolean;
  coaxial: boolean;
}> {
  return invoke('get_material_info', { materialName: name });
}

export async function calculateViscosity(
  materialName: string,
  shearRate: number
): Promise<number> {
  return invoke('calculate_viscosity', { materialName, shearRate });
}

export async function calculatePressure(
  materialName: string,
  flowRate: number,
  nozzleDiameter: number
): Promise<{ pressureDrop: number; wallShearStress: number }> {
  return invoke('calculate_pressure', { materialName, flowRate, nozzleDiameter });
}

export async function calculateSlump(
  materialName: string,
  initialWidth: number,
  initialHeight: number,
  printSpeed: number,
  layerTime: number
): Promise<{
  widthAfter: number;
  heightAfter: number;
  settlingTime: number;
  slumpFactor: number;
}> {
  return invoke('calculate_slump', {
    materialName,
    initialWidth,
    initialHeight,
    printSpeed,
    layerTime,
  });
}

export async function validateInputs(
  material: unknown,
  machine: unknown,
  profile: unknown
): Promise<{ valid: boolean; errors: string[] }> {
  return invoke('validate_inputs', { material, machine, profile });
}

export async function importMaterialCsv(
  name: string,
  density: number,
  csvData: string
): Promise<{ success: boolean }> {
  return invoke('import_material_csv', { name, density, csvData });
}

export async function undo(): Promise<{ id: number; label: string } | null> {
  return invoke('undo');
}

export async function redo(): Promise<{ id: number; label: string } | null> {
  return invoke('redo');
}

export async function canUndo(): Promise<boolean> {
  return invoke('can_undo');
}

export async function canRedo(): Promise<boolean> {
  return invoke('can_redo');
}

export async function executeSlice(
  materialJson: string,
  paramsJson: string
): Promise<{
  layers: Array<{ z: number; polygons: Array<Array<{ x: number; y: number; z: number }>> }>;
  commands: number;
}> {
  return invoke('execute_slice', { materialJson, paramsJson });
}

export async function generateGcode(
  materialJson: string,
  paramsJson: string
): Promise<string> {
  return invoke('generate_gcode', { materialJson, paramsJson });
}

// ===== Phase 7: Intelligence & Automation =====

export async function recommendParameters(
  materialName: string,
  volume: number,
  surfaceArea: number,
  height: number,
  minFeatureSize: number,
  maxOverhangAngle: number
): Promise<{
  nozzleDiameter: number;
  layerHeight: number;
  printSpeed: number;
  flowRate: number;
  pressure: number;
  temperature: number | null;
  confidence: number;
  reasoning: string[];
}> {
  return invoke('recommend_parameters', {
    materialName,
    volume,
    surfaceArea,
    height,
    minFeatureSize,
    maxOverhangAngle,
  });
}

export async function simulateLayers(
  materialName: string,
  beadWidth: number,
  beadHeight: number,
  interLayerTime: number,
  printSpeed: number,
  layerCount: number
): Promise<{
  layers: Array<{
    z: number;
    widthAfter: number;
    heightAfter: number;
    verticalDisplacement: number;
    slumpFactor: number;
    settled: boolean;
  }>;
  finalHeight: number;
  maxWidthIncrease: number;
  maxDisplacement: number;
  shapeRetention: number;
}> {
  return invoke('simulate_layers', {
    materialName,
    beadWidth,
    beadHeight,
    interLayerTime,
    printSpeed,
    layerCount,
  });
}

export async function feedbackUpdate(
  pressureKpa: number,
  temperatureC: number,
  timestampS: number
): Promise<{
  pressureDelta: number;
  temperatureDelta: number;
  speedMultiplier: number;
  abort: boolean;
  message: string | null;
}> {
  return invoke('feedback_update', { pressureKpa, temperatureC, timestampS });
}

export async function feedbackReset(): Promise<void> {
  return invoke('feedback_reset');
}

export async function feedbackSetTarget(
  pressureKpa: number,
  temperatureC: number
): Promise<void> {
  return invoke('feedback_set_target', { pressureKpa, temperatureC });
}

export async function queueEnqueue(
  name: string,
  gcode: string,
  materialName: string,
  estimatedTimeS: number,
  priority: string
): Promise<string> {
  return invoke('queue_enqueue', {
    name,
    gcode,
    materialName,
    estimatedTimeS,
    priority,
  });
}

export async function queueList(): Promise<
  Array<{
    id: string;
    name: string;
    priority: string;
    status: string;
    materialName: string;
    estimatedTimeS: number;
    description: string;
  }>
> {
  return invoke('queue_list');
}

export async function queueStartNext(): Promise<{
  id: string;
  name: string;
  priority: string;
  status: string;
  materialName: string;
  estimatedTimeS: number;
  description: string;
} | null> {
  return invoke('queue_start_next');
}

export async function queueCompleteCurrent(): Promise<string | null> {
  return invoke('queue_complete_current');
}

export async function queueFailCurrent(
  reason: string
): Promise<string | null> {
  return invoke('queue_fail_current', { reason });
}

export async function queueCancel(id: string): Promise<boolean> {
  return invoke('queue_cancel', { id });
}

export async function queueSetPriority(
  id: string,
  priority: string
): Promise<boolean> {
  return invoke('queue_set_priority', { id, priority });
}

export async function queueMove(
  id: string,
  position: number
): Promise<boolean> {
  return invoke('queue_move', { id, position });
}

export async function queueClearHistory(): Promise<void> {
  return invoke('queue_clear_history');
}

export async function queueCurrent(): Promise<{
  id: string;
  name: string;
  priority: string;
  status: string;
  materialName: string;
  estimatedTimeS: number;
  description: string;
} | null> {
  return invoke('queue_current');
}

export async function qualityFeed(
  pressureKpa: number,
  temperatureC: number,
  flowRate: number,
  x: number,
  y: number,
  z: number,
  extrudedVolume: number,
  timestampS: number
): Promise<
  Array<{
    defectType: string;
    severity: string;
    message: string;
    timestampS: number;
    location: [number, number, number] | null;
  }>
> {
  return invoke('quality_feed', {
    pressureKpa,
    temperatureC,
    flowRate,
    x,
    y,
    z,
    extrudedVolume,
    timestampS,
  });
}

export async function qualityAlerts(): Promise<
  Array<{
    defectType: string;
    severity: string;
    message: string;
    timestampS: number;
    location: [number, number, number] | null;
  }>
> {
  return invoke('quality_alerts');
}

export async function qualityReset(): Promise<void> {
  return invoke('quality_reset');
}

export async function qualityScore(): Promise<number> {
  return invoke('quality_score');
}

export async function qualitySetExpectedFlow(
  flowRate: number
): Promise<void> {
  return invoke('quality_set_expected_flow', { flowRate });
}
