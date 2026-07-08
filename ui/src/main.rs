// TPT BioCAD UI - Tauri main entry point
// Licensed under Apache 2.0

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            greet,
            get_materials,
            calculate_viscosity,
            calculate_pressure,
            get_material_info,
            calculate_slump,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

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

#[derive(serde::Serialize)]
struct SlumpInfo {
    width_after: f64,
    height_after: f64,
    settling_time: f64,
    slump_factor: f64,
}
