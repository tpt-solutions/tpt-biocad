// TPT BioCAD UI - Tauri main entry point
// Licensed under Apache 2.0

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            greet,
            get_materials,
            calculate_viscosity,
            calculate_pressure,
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
}

#[tauri::command]
fn calculate_viscosity(material_name: &str, shear_rate: f64) -> Result<f64, String> {
    let material = tpt_fluid::get_material(material_name)
        .ok_or_else(|| "Material not found".to_string())?;
    Ok(tpt_fluid::viscosity(&material.rheology, shear_rate))
}

#[tauri::command]
fn calculate_pressure(
    material_name: &str,
    flow_rate: f64,
    nozzle_diameter: f64,
) -> Result<tpt_fluid::FlowResult, String> {
    let material = tpt_fluid::get_material(material_name)
        .ok_or_else(|| "Material not found".to_string())?;
    
    let geometry = tpt_fluid::NozzleGeometry {
        inlet_diameter: 1.0,
        outlet_diameter: nozzle_diameter,
        length: 10.0,
        taper_angle: 0.0,
    };
    
    tpt_fluid::solve_pressure(&material.rheology, &geometry, flow_rate, 101325.0)
        .map_err(|e| e.to_string())
}