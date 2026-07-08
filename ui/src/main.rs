// TPT BioCAD UI - Tauri main entry point
// Licensed under Apache 2.0
//
// Phase 6: Undo/redo command pattern, slice & G-code commands

use std::sync::Mutex;
use tauri::State;

// ----- Phase 7 state containers -----

struct FeedbackState {
    controller: Mutex<tpt_hal::FeedbackController>,
}

struct QueueState {
    queue: Mutex<tpt_core::PrintQueue>,
}

struct MonitorState {
    monitor: Mutex<tpt_hal::QualityMonitor>,
}

// ----- Undo/redo engine -----

/// Serialised snapshot of the entire application state.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct AppSnapshot {
    selected_material: String,
    material_params: String, // JSON blob
    slice_params: String,    // JSON blob
    viewport_layer: usize,
}

impl AppSnapshot {
    fn empty() -> Self {
        Self {
            selected_material: String::new(),
            material_params: "{}".into(),
            slice_params: "{}".into(),
            viewport_layer: 0,
        }
    }
}

struct UndoRedoState {
    history: Vec<AppSnapshot>,
    position: usize, // index of current state; history[position] is current
    max_history: usize,
}

impl UndoRedoState {
    fn new(max: usize) -> Self {
        Self {
            history: vec![AppSnapshot::empty()],
            position: 0,
            max_history: max,
        }
    }

    fn push(&mut self, snapshot: AppSnapshot) {
        // Truncate any redo states beyond current position
        self.history.truncate(self.position + 1);
        self.history.push(snapshot);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
        self.position = self.history.len() - 1;
    }

    fn undo(&mut self) -> Option<&AppSnapshot> {
        if self.position > 0 {
            self.position -= 1;
            Some(&self.history[self.position])
        } else {
            None
        }
    }

    fn redo(&mut self) -> Option<&AppSnapshot> {
        if self.position + 1 < self.history.len() {
            self.position += 1;
            Some(&self.history[self.position])
        } else {
            None
        }
    }

    fn can_undo(&self) -> bool {
        self.position > 0
    }

    fn can_redo(&self) -> bool {
        self.position + 1 < self.history.len()
    }
}

struct AppState {
    undo_redo: Mutex<UndoRedoState>,
}

// ----- Undo/redo Tauri commands -----

#[tauri::command]
fn can_undo(state: State<AppState>) -> bool {
    state.undo_redo.lock().unwrap().can_undo()
}

#[tauri::command]
fn can_redo(state: State<AppState>) -> bool {
    state.undo_redo.lock().unwrap().can_redo()
}

#[derive(serde::Serialize)]
struct UndoInfo {
    id: usize,
    label: String,
}

#[tauri::command]
fn undo(state: State<AppState>) -> Option<UndoInfo> {
    let mut ur = state.undo_redo.lock().unwrap();
    let _snap = ur.undo()?;
    Some(UndoInfo {
        id: ur.position,
        label: format!("undo to state {}", ur.position),
    })
}

#[tauri::command]
fn redo(state: State<AppState>) -> Option<UndoInfo> {
    let mut ur = state.undo_redo.lock().unwrap();
    let _snap = ur.redo()?;
    Some(UndoInfo {
        id: ur.position,
        label: format!("redo to state {}", ur.position),
    })
}

#[tauri::command]
fn push_snapshot(state: State<AppState>, label: String) {
    let snapshot = AppSnapshot {
        selected_material: label.clone(),
        material_params: "{}".into(),
        slice_params: "{}".into(),
        viewport_layer: 0,
    };
    state.undo_redo.lock().unwrap().push(snapshot);
}

// ----- Original commands (extended) -----

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! Welcome to TPT BioCAD.", name)
}

#[tauri::command]
fn get_materials() -> Vec<String> {
    tpt_fluid::list_materials()
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

#[tauri::command]
fn calculate_viscosity(material_name: &str, shear_rate: f64) -> Result<f64, String> {
    let material =
        tpt_fluid::get_material(material_name).ok_or_else(|| "Material not found".to_string())?;
    Ok(tpt_fluid::viscosity(&material.rheology, shear_rate))
}

#[tauri::command]
fn calculate_pressure(
    material_name: &str,
    flow_rate: f64,
    nozzle_diameter: f64,
) -> Result<tpt_fluid::FlowResult, String> {
    if flow_rate < 0.0 {
        return Err("flow rate must be non-negative".to_string());
    }
    if nozzle_diameter <= 0.0 {
        return Err("nozzle diameter must be positive".to_string());
    }

    let material =
        tpt_fluid::get_material(material_name).ok_or_else(|| "Material not found".to_string())?;

    let geometry = tpt_fluid::NozzleGeometry {
        inlet_diameter: 1.0,
        outlet_diameter: nozzle_diameter,
        length: 10.0,
        taper_angle: 0.0,
    };

    tpt_fluid::solve_pressure(&material.rheology, &geometry, flow_rate, 101325.0)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_material_info(material_name: &str) -> Result<MaterialInfo, String> {
    let material =
        tpt_fluid::get_material(material_name).ok_or_else(|| "Material not found".to_string())?;
    Ok(MaterialInfo {
        name: material.name,
        density: material.density,
        has_uv_curing: material.curing.is_some(),
        uv_intensity: material.curing.as_ref().map(|c| c.uv_intensity),
        uv_exposure_time: material.curing.as_ref().map(|c| c.exposure_time),
        has_coaxial: material.coaxial.is_some(),
        crosslinker: material
            .coaxial
            .as_ref()
            .map(|c| c.crosslinker_name.clone()),
    })
}

#[tauri::command]
fn import_material_csv(
    name: &str,
    density: f64,
    csv_data: &str,
) -> Result<tpt_core::Material, String> {
    if name.is_empty() {
        return Err("material name must not be empty".to_string());
    }
    if density <= 0.0 {
        return Err("density must be positive".to_string());
    }
    tpt_fluid::import_material_from_csv(name, density, csv_data)
}

#[tauri::command]
fn validate_inputs(
    material_json: &str,
    machine_json: &str,
    profile_json: &str,
) -> Result<Vec<String>, String> {
    let mut all_errors = Vec::new();

    if !material_json.is_empty() {
        let material: tpt_core::Material = serde_json::from_str(material_json)
            .map_err(|e| format!("invalid material JSON: {}", e))?;
        if let Err(errors) = tpt_core::validate_material(&material) {
            all_errors.extend(errors);
        }
    }

    if !machine_json.is_empty() {
        let machine: tpt_core::Machine = serde_json::from_str(machine_json)
            .map_err(|e| format!("invalid machine JSON: {}", e))?;
        if let Err(errors) = tpt_core::validate_machine(&machine) {
            all_errors.extend(errors);
        }
    }

    if !profile_json.is_empty() {
        let profile: tpt_core::Profile = serde_json::from_str(profile_json)
            .map_err(|e| format!("invalid profile JSON: {}", e))?;
        if let Err(errors) = tpt_core::validate_profile(&profile) {
            all_errors.extend(errors);
        }
    }

    Ok(all_errors)
}

#[tauri::command]
fn calculate_slump(
    material_name: &str,
    initial_width: f64,
    initial_height: f64,
    print_speed: f64,
    layer_time: f64,
) -> Result<SlumpInfo, String> {
    if initial_width <= 0.0 {
        return Err("bead width must be positive".to_string());
    }
    if initial_height <= 0.0 {
        return Err("bead height must be positive".to_string());
    }
    if layer_time < 0.0 {
        return Err("layer time must be non-negative".to_string());
    }

    let material =
        tpt_fluid::get_material(material_name).ok_or_else(|| "Material not found".to_string())?;
    let result = tpt_fluid::predict_slump(
        &material.rheology,
        material.density,
        initial_width,
        initial_height,
        print_speed,
        layer_time,
    );
    Ok(SlumpInfo {
        width_after: result.width_after,
        height_after: result.height_after,
        settling_time: result.settling_time,
        slump_factor: result.slump_factor,
    })
}

// ----- Phase 6 new commands: slice & G-code -----

#[derive(serde::Serialize)]
struct SliceResultInfo {
    layers: Vec<SliceLayerInfo>,
    commands: usize,
    print_time_seconds: f64,
    total_length: f64,
}

#[derive(serde::Serialize)]
struct SliceLayerInfo {
    z: f64,
    polygons: Vec<Vec<PointInfo>>,
}

#[derive(serde::Serialize, Clone)]
struct PointInfo {
    x: f64,
    y: f64,
    z: f64,
}

#[tauri::command]
fn execute_slice(material_json: &str, params_json: &str) -> Result<SliceResultInfo, String> {
    // Parse material
    let material: tpt_core::Material =
        serde_json::from_str(material_json).map_err(|e| format!("invalid material JSON: {}", e))?;

    // Parse slicing params
    #[derive(serde::Deserialize)]
    struct SliceParams {
        layer_height: f64,
        infill_density: f64,
        perimeters: usize,
        #[serde(default = "default_infill")]
        infill_pattern: String,
        #[serde(default)]
        top_solid_layers: usize,
        #[serde(default)]
        bottom_solid_layers: usize,
    }
    fn default_infill() -> String {
        "grid".into()
    }

    let sp: SliceParams = serde_json::from_str(params_json)
        .map_err(|e| format!("invalid slice params JSON: {}", e))?;

    let infill_pattern = match sp.infill_pattern.to_lowercase().as_str() {
        "grid" => tpt_core::InfillPattern::Grid,
        "gyroid" => tpt_core::InfillPattern::Gyroid,
        "honeycomb" => tpt_core::InfillPattern::Honeycomb,
        "voronoi" => tpt_core::InfillPattern::Voronoi,
        _ => tpt_core::InfillPattern::Grid,
    };

    let slicing_params = tpt_slice::SlicingParams {
        layer_height: sp.layer_height,
        infill_density: sp.infill_density,
        infill_pattern,
        perimeters: sp.perimeters,
        top_solid_layers: sp.top_solid_layers,
        bottom_solid_layers: sp.bottom_solid_layers,
    };

    let slicer = tpt_slice::Slicer::new(slicing_params);

    let mesh = demo_mesh();

    let result = slicer.slice(&mesh, Some(&material));

    let layers: Vec<SliceLayerInfo> = result
        .layers
        .iter()
        .map(|l| SliceLayerInfo {
            z: l.z,
            polygons: l
                .polygons
                .iter()
                .map(|p| {
                    p.points
                        .iter()
                        .map(|pt| PointInfo {
                            x: pt.x,
                            y: pt.y,
                            z: pt.z,
                        })
                        .collect()
                })
                .collect(),
        })
        .collect();

    Ok(SliceResultInfo {
        layers,
        commands: result.commands.len(),
        print_time_seconds: result.print_time_seconds(),
        total_length: result.total_toolpath_length(),
    })
}

fn demo_mesh() -> tpt_geometry::Mesh {
    let v = |x: f64, y: f64, z: f64| nalgebra::Point3::new(x, y, z);
    tpt_geometry::Mesh {
        vertices: vec![
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
            v(0.0, 0.0, 1.0),
            v(1.0, 0.0, 1.0),
            v(1.0, 1.0, 1.0),
            v(0.0, 1.0, 1.0),
        ],
        triangles: vec![
            // Bottom
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(1.0, 0.0, 0.0),
                v3: v(1.0, 1.0, 0.0),
                normal: v(0.0, 0.0, -1.0),
            },
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(1.0, 1.0, 0.0),
                v3: v(0.0, 1.0, 0.0),
                normal: v(0.0, 0.0, -1.0),
            },
            // Top
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 1.0),
                v2: v(1.0, 1.0, 1.0),
                v3: v(1.0, 0.0, 1.0),
                normal: v(0.0, 0.0, 1.0),
            },
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 1.0),
                v2: v(0.0, 1.0, 1.0),
                v3: v(1.0, 1.0, 1.0),
                normal: v(0.0, 0.0, 1.0),
            },
            // Front
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(1.0, 0.0, 1.0),
                v3: v(1.0, 0.0, 0.0),
                normal: v(0.0, -1.0, 0.0),
            },
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(0.0, 0.0, 1.0),
                v3: v(1.0, 0.0, 1.0),
                normal: v(0.0, -1.0, 0.0),
            },
            // Back
            tpt_geometry::Triangle {
                v1: v(0.0, 1.0, 0.0),
                v2: v(1.0, 1.0, 0.0),
                v3: v(1.0, 1.0, 1.0),
                normal: v(0.0, 1.0, 0.0),
            },
            tpt_geometry::Triangle {
                v1: v(0.0, 1.0, 0.0),
                v2: v(1.0, 1.0, 1.0),
                v3: v(0.0, 1.0, 1.0),
                normal: v(0.0, 1.0, 0.0),
            },
            // Left
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(0.0, 1.0, 1.0),
                v3: v(0.0, 1.0, 0.0),
                normal: v(-1.0, 0.0, 0.0),
            },
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(0.0, 0.0, 1.0),
                v3: v(0.0, 1.0, 1.0),
                normal: v(-1.0, 0.0, 0.0),
            },
            // Right
            tpt_geometry::Triangle {
                v1: v(1.0, 0.0, 0.0),
                v2: v(1.0, 1.0, 0.0),
                v3: v(1.0, 1.0, 1.0),
                normal: v(1.0, 0.0, 0.0),
            },
            tpt_geometry::Triangle {
                v1: v(1.0, 0.0, 0.0),
                v2: v(1.0, 1.0, 1.0),
                v3: v(1.0, 0.0, 1.0),
                normal: v(1.0, 0.0, 0.0),
            },
        ],
    }
}

#[tauri::command]
fn generate_gcode(material_json: &str, params_json: &str) -> Result<String, String> {
    let material: tpt_core::Material =
        serde_json::from_str(material_json).map_err(|e| format!("invalid material JSON: {}", e))?;

    #[derive(serde::Deserialize)]
    struct SliceParams {
        layer_height: f64,
        infill_density: f64,
        perimeters: usize,
        #[serde(default = "default_infill")]
        infill_pattern: String,
        #[serde(default)]
        top_solid_layers: usize,
        #[serde(default)]
        bottom_solid_layers: usize,
    }
    fn default_infill() -> String {
        "grid".into()
    }

    let sp: SliceParams = serde_json::from_str(params_json)
        .map_err(|e| format!("invalid slice params JSON: {}", e))?;

    let infill_pattern = match sp.infill_pattern.to_lowercase().as_str() {
        "grid" => tpt_core::InfillPattern::Grid,
        "gyroid" => tpt_core::InfillPattern::Gyroid,
        "honeycomb" => tpt_core::InfillPattern::Honeycomb,
        "voronoi" => tpt_core::InfillPattern::Voronoi,
        _ => tpt_core::InfillPattern::Grid,
    };

    let slicing_params = tpt_slice::SlicingParams {
        layer_height: sp.layer_height,
        infill_density: sp.infill_density,
        infill_pattern,
        perimeters: sp.perimeters,
        top_solid_layers: sp.top_solid_layers,
        bottom_solid_layers: sp.bottom_solid_layers,
    };

    let slicer = tpt_slice::Slicer::new(slicing_params);
    let mesh = demo_mesh();
    let result = slicer.slice(&mesh, Some(&material));

    let mut gcode_gen = tpt_slice::GCodeGenerator::new();
    gcode_gen.add_toolpath(&result.commands);

    Ok(gcode_gen.generate())
}

// ----- Info structs -----

#[derive(serde::Serialize)]
struct MaterialInfo {
    name: String,
    density: f64,
    has_uv_curing: bool,
    uv_intensity: Option<f64>,
    uv_exposure_time: Option<f64>,
    has_coaxial: bool,
    crosslinker: Option<String>,
}

#[derive(serde::Serialize)]
struct SlumpInfo {
    width_after: f64,
    height_after: f64,
    settling_time: f64,
    slump_factor: f64,
}

// ===== Phase 7: Intelligence & Automation Commands =====

// --- 1. AI-assisted parameter selection ---

#[derive(serde::Serialize)]
struct RecommendationInfo {
    nozzle_diameter: f64,
    layer_height: f64,
    print_speed: f64,
    flow_rate: f64,
    pressure: f64,
    temperature: Option<f64>,
    confidence: f64,
    reasoning: Vec<String>,
}

#[tauri::command]
fn recommend_parameters(
    material_name: &str,
    volume: f64,
    surface_area: f64,
    height: f64,
    min_feature_size: f64,
    max_overhang_angle: f64,
) -> Result<RecommendationInfo, String> {
    let material =
        tpt_fluid::get_material(material_name).ok_or_else(|| "Material not found".to_string())?;

    let features = tpt_fluid::GeometryFeatures {
        volume,
        surface_area,
        height,
        min_feature_size,
        max_overhang_angle,
        part_count: 1,
    };

    let rec = tpt_fluid::recommend_parameters(&features, &material);
    Ok(RecommendationInfo {
        nozzle_diameter: rec.nozzle_diameter,
        layer_height: rec.layer_height,
        print_speed: rec.print_speed,
        flow_rate: rec.flow_rate,
        pressure: rec.pressure,
        temperature: rec.temperature,
        confidence: rec.confidence,
        reasoning: rec.reasoning,
    })
}

// --- 2. Layer-by-layer slumping simulation ---

#[derive(serde::Serialize)]
struct SimulationLayerInfo {
    z: f64,
    width_after: f64,
    height_after: f64,
    vertical_displacement: f64,
    slump_factor: f64,
    settled: bool,
}

#[derive(serde::Serialize)]
struct SimulationResultInfo {
    layers: Vec<SimulationLayerInfo>,
    final_height: f64,
    max_width_increase: f64,
    max_displacement: f64,
    shape_retention: f64,
}

#[tauri::command]
fn simulate_layers(
    material_name: &str,
    bead_width: f64,
    bead_height: f64,
    inter_layer_time: f64,
    print_speed: f64,
    layer_count: usize,
) -> Result<SimulationResultInfo, String> {
    if bead_width <= 0.0 || bead_height <= 0.0 {
        return Err("bead dimensions must be positive".to_string());
    }
    if layer_count == 0 {
        return Err("layer count must be positive".to_string());
    }

    let material =
        tpt_fluid::get_material(material_name).ok_or_else(|| "Material not found".to_string())?;

    let layer_heights: Vec<f64> = (0..layer_count)
        .map(|i| bead_height * 0.5 + i as f64 * bead_height)
        .collect();

    let config = tpt_fluid::SimulationConfig {
        inter_layer_time,
        print_speed,
        accumulate_weight: true,
        settle_threshold: 0.01,
    };

    let result =
        tpt_fluid::simulate_layers(&layer_heights, &material, bead_width, bead_height, &config);

    Ok(SimulationResultInfo {
        layers: result
            .layers
            .iter()
            .map(|l| SimulationLayerInfo {
                z: l.z,
                width_after: l.width_after,
                height_after: l.height_after,
                vertical_displacement: l.vertical_displacement,
                slump_factor: l.slump_factor,
                settled: l.settled,
            })
            .collect(),
        final_height: result.final_height,
        max_width_increase: result.max_width_increase,
        max_displacement: result.max_displacement,
        shape_retention: result.shape_retention,
    })
}

// --- 3. Real-time feedback loop ---

#[derive(serde::Serialize)]
struct FeedbackActionInfo {
    pressure_delta: f64,
    temperature_delta: f64,
    speed_multiplier: f64,
    abort: bool,
    message: Option<String>,
}

#[tauri::command]
fn feedback_update(
    state: State<FeedbackState>,
    pressure_kpa: f64,
    temperature_c: f64,
    timestamp_s: f64,
) -> FeedbackActionInfo {
    let mut ctrl = state.controller.lock().unwrap();
    let reading = tpt_hal::SensorReading {
        pressure_kpa,
        temperature_c,
        timestamp_s,
    };
    let action = ctrl.update(reading);
    FeedbackActionInfo {
        pressure_delta: action.pressure_delta,
        temperature_delta: action.temperature_delta,
        speed_multiplier: action.speed_multiplier,
        abort: action.abort,
        message: action.message,
    }
}

#[tauri::command]
fn feedback_reset(state: State<FeedbackState>) {
    state.controller.lock().unwrap().reset();
}

#[tauri::command]
fn feedback_set_target(state: State<FeedbackState>, pressure_kpa: f64, temperature_c: f64) {
    let mut ctrl = state.controller.lock().unwrap();
    ctrl.target.pressure_kpa = pressure_kpa;
    ctrl.target.temperature_c = temperature_c;
}

// --- 4. Print queue management ---

#[derive(serde::Serialize)]
struct JobInfo {
    id: String,
    name: String,
    priority: String,
    status: String,
    material_name: String,
    estimated_time_s: f64,
    description: String,
}

#[tauri::command]
fn queue_enqueue(
    state: State<QueueState>,
    name: String,
    gcode: String,
    material_name: String,
    estimated_time_s: f64,
    priority: String,
) -> String {
    let prio = match priority.to_lowercase().as_str() {
        "low" => tpt_core::Priority::Low,
        "high" => tpt_core::Priority::High,
        "critical" => tpt_core::Priority::Critical,
        _ => tpt_core::Priority::Normal,
    };

    let id = format!(
        "job_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );

    let job = tpt_core::PrintJob::with_priority(
        id.clone(),
        name,
        gcode,
        material_name,
        estimated_time_s,
        prio,
    );

    state.queue.lock().unwrap().enqueue(job);
    id
}

#[tauri::command]
fn queue_list(state: State<QueueState>) -> Vec<JobInfo> {
    state
        .queue
        .lock()
        .unwrap()
        .all_jobs()
        .iter()
        .map(|j| JobInfo {
            id: j.id.clone(),
            name: j.name.clone(),
            priority: j.priority.to_string(),
            status: j.status.to_string(),
            material_name: j.material_name.clone(),
            estimated_time_s: j.estimated_time_s,
            description: j.description.clone(),
        })
        .collect()
}

#[tauri::command]
fn queue_start_next(state: State<QueueState>) -> Option<JobInfo> {
    let mut queue = state.queue.lock().unwrap();
    let job = queue.start_next()?;
    Some(JobInfo {
        id: job.id.clone(),
        name: job.name.clone(),
        priority: job.priority.to_string(),
        status: job.status.to_string(),
        material_name: job.material_name.clone(),
        estimated_time_s: job.estimated_time_s,
        description: job.description.clone(),
    })
}

#[tauri::command]
fn queue_complete_current(state: State<QueueState>) -> Option<String> {
    state.queue.lock().unwrap().complete_current()
}

#[tauri::command]
fn queue_fail_current(state: State<QueueState>, reason: String) -> Option<String> {
    state.queue.lock().unwrap().fail_current(reason)
}

#[tauri::command]
fn queue_cancel(state: State<QueueState>, id: String) -> bool {
    state.queue.lock().unwrap().cancel(&id)
}

#[tauri::command]
fn queue_set_priority(state: State<QueueState>, id: String, priority: String) -> bool {
    let prio = match priority.to_lowercase().as_str() {
        "low" => tpt_core::Priority::Low,
        "high" => tpt_core::Priority::High,
        "critical" => tpt_core::Priority::Critical,
        _ => tpt_core::Priority::Normal,
    };
    state.queue.lock().unwrap().set_priority(&id, prio)
}

#[tauri::command]
fn queue_move(state: State<QueueState>, id: String, position: usize) -> bool {
    state.queue.lock().unwrap().move_to_position(&id, position)
}

#[tauri::command]
fn queue_clear_history(state: State<QueueState>) {
    state.queue.lock().unwrap().clear_history();
}

#[tauri::command]
fn queue_current(state: State<QueueState>) -> Option<JobInfo> {
    let queue = state.queue.lock().unwrap();
    let job = queue.current_job()?;
    Some(JobInfo {
        id: job.id.clone(),
        name: job.name.clone(),
        priority: job.priority.to_string(),
        status: job.status.to_string(),
        material_name: job.material_name.clone(),
        estimated_time_s: job.estimated_time_s,
        description: job.description.clone(),
    })
}

// --- 5. Quality monitoring ---

#[derive(serde::Serialize)]
struct QualityAlertInfo {
    defect_type: String,
    severity: String,
    message: String,
    timestamp_s: f64,
    location: Option<(f64, f64, f64)>,
}

#[tauri::command]
fn quality_feed(
    state: State<MonitorState>,
    pressure_kpa: f64,
    temperature_c: f64,
    flow_rate: f64,
    x: f64,
    y: f64,
    z: f64,
    extruded_volume: f64,
    timestamp_s: f64,
) -> Vec<QualityAlertInfo> {
    let mut monitor = state.monitor.lock().unwrap();
    let sample = tpt_hal::PrintSample {
        pressure_kpa,
        temperature_c,
        flow_rate,
        x,
        y,
        z,
        extruded_volume,
        timestamp_s,
    };

    monitor
        .feed(sample)
        .into_iter()
        .map(|a| QualityAlertInfo {
            defect_type: a.defect_type.to_string(),
            severity: format!("{:?}", a.severity),
            message: a.message,
            timestamp_s: a.timestamp_s,
            location: a.location,
        })
        .collect()
}

#[tauri::command]
fn quality_alerts(state: State<MonitorState>) -> Vec<QualityAlertInfo> {
    state
        .monitor
        .lock()
        .unwrap()
        .alerts()
        .iter()
        .map(|a| QualityAlertInfo {
            defect_type: a.defect_type.to_string(),
            severity: format!("{:?}", a.severity),
            message: a.message.clone(),
            timestamp_s: a.timestamp_s,
            location: a.location,
        })
        .collect()
}

#[tauri::command]
fn quality_reset(state: State<MonitorState>) {
    state.monitor.lock().unwrap().reset();
}

#[tauri::command]
fn quality_score(state: State<MonitorState>) -> f64 {
    state.monitor.lock().unwrap().quality_score()
}

#[tauri::command]
fn quality_set_expected_flow(state: State<MonitorState>, flow_rate: f64) {
    state
        .monitor
        .lock()
        .unwrap()
        .config_mut()
        .expected_flow_rate = flow_rate;
}

// ----- Main -----

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            undo_redo: Mutex::new(UndoRedoState::new(50)),
        })
        .manage(FeedbackState {
            controller: Mutex::new(tpt_hal::FeedbackController::new()),
        })
        .manage(QueueState {
            queue: Mutex::new(tpt_core::PrintQueue::new()),
        })
        .manage(MonitorState {
            monitor: Mutex::new(tpt_hal::QualityMonitor::new(
                tpt_hal::MonitorConfig::default(),
            )),
        })
        .invoke_handler(tauri::generate_handler![
            // Original
            greet,
            get_materials,
            calculate_viscosity,
            calculate_pressure,
            get_material_info,
            calculate_slump,
            import_material_csv,
            validate_inputs,
            // Phase 6
            can_undo,
            can_redo,
            undo,
            redo,
            push_snapshot,
            execute_slice,
            generate_gcode,
            // Phase 7
            recommend_parameters,
            simulate_layers,
            feedback_update,
            feedback_reset,
            feedback_set_target,
            queue_enqueue,
            queue_list,
            queue_start_next,
            queue_complete_current,
            queue_fail_current,
            queue_cancel,
            queue_set_priority,
            queue_move,
            queue_clear_history,
            queue_current,
            quality_feed,
            quality_alerts,
            quality_reset,
            quality_score,
            quality_set_expected_flow,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
