// 3MF format export with material metadata
// Licensed under Apache 2.0
//
// Implements export of triangle meshes to the 3D Manufacturing Format (3MF),
// an XML-based, zip-container format. Supports embedding material metadata
// (rheology model type, density, UV curing parameters, coaxial params) as
// 3MF material resources.

use crate::mesh::Mesh;
use std::io::{Seek, Write};
use thiserror::Error;
use tpt_core::{GeometryData, GeometryFormat, Material};

/// Errors that can occur during 3MF export.
#[derive(Error, Debug)]
pub enum ThreeMfError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Invalid mesh: {0}")]
    InvalidMesh(String),
    #[error("Serialization error: {0}")]
    Serde(String),
}

/// 3MF export options.
#[derive(Debug, Clone)]
pub struct ThreeMfOptions {
    /// Include material metadata as a 3MF material resource.
    pub embed_material: bool,
    /// Unit scale: "millimeter" (default), "micron", "centimeter", etc.
    pub units: String,
    /// Application name for the 3MF metadata.
    pub application: String,
}

impl Default for ThreeMfOptions {
    fn default() -> Self {
        Self {
            embed_material: true,
            units: "millimeter".to_string(),
            application: "TPT BioCAD".to_string(),
        }
    }
}

/// Export a mesh to a 3MF file, optionally embedding material metadata.
///
/// The 3MF file is a zip archive containing:
/// - `[Content_Types].xml` — content type declarations
/// - `_rels/.rels` — package relationships
/// - `3D/3dmodel.model` — the 3D model (mesh + materials)
/// - `Metadata/model.xml` — material properties metadata
///
/// # Arguments
/// * `writer` — Any writable/seekable destination (file, buffer, etc.).
/// * `mesh` — The triangle mesh to export.
/// * `material` — Optional material metadata to embed.
/// * `options` — Export configuration.
pub fn export_3mf<W: Write + Seek>(
    writer: W,
    mesh: &Mesh,
    material: Option<&Material>,
    options: &ThreeMfOptions,
) -> Result<(), ThreeMfError> {
    if mesh.triangles.is_empty() {
        return Err(ThreeMfError::InvalidMesh(
            "mesh has no triangles".to_string(),
        ));
    }

    let mut zip = zip::ZipWriter::new(writer);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", zip::write::FileOptions::default())?;
    write_content_types(&mut zip, material.is_some())?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", zip::write::FileOptions::default())?;
    write_rels(&mut zip)?;

    // 3. 3D/3dmodel.model
    zip.start_file("3D/3dmodel.model", zip::write::FileOptions::default())?;
    write_model(&mut zip, mesh, material, options)?;

    // 4. 3D/_rels/model.rels — relationship to material metadata
    if material.is_some() && options.embed_material {
        zip.start_file("3D/_rels/model.rels", zip::write::FileOptions::default())?;
        write_model_rels(&mut zip)?;
    }

    // 5. Metadata/model.xml — material properties
    if let Some(mat) = material {
        if options.embed_material {
            zip.start_file("Metadata/model.xml", zip::write::FileOptions::default())?;
            write_metadata(&mut zip, mat)?;
        }
    }

    zip.finish()?;
    Ok(())
}

/// Export a mesh to 3MF bytes (in-memory).
pub fn export_3mf_bytes(
    mesh: &Mesh,
    material: Option<&Material>,
    options: &ThreeMfOptions,
) -> Result<Vec<u8>, ThreeMfError> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    export_3mf(&mut cursor, mesh, material, options)?;
    Ok(cursor.into_inner())
}

/// Convert a mesh and material to `GeometryData` (3MF format).
pub fn mesh_to_3mf_geometry_data(
    mesh: &Mesh,
    material: Option<&Material>,
) -> Result<GeometryData, ThreeMfError> {
    let data = export_3mf_bytes(mesh, material, &ThreeMfOptions::default())?;
    Ok(GeometryData {
        format: GeometryFormat::ThreeMf,
        data,
    })
}

// -- XML writers --

fn write_content_types<W: Write + Seek>(
    w: &mut zip::ZipWriter<W>,
    has_metadata: bool,
) -> std::io::Result<()> {
    writeln!(w, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        w,
        r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#
    )?;
    writeln!(
        w,
        r#"  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#
    )?;
    writeln!(
        w,
        r#"  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>"#
    )?;
    if has_metadata {
        writeln!(
            w,
            r#"  <Default Extension="xml" ContentType="application/xml"/>"#
        )?;
    }
    writeln!(w, r#"</Types>"#)?;
    Ok(())
}

fn write_rels<W: Write + Seek>(w: &mut zip::ZipWriter<W>) -> std::io::Result<()> {
    writeln!(w, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        w,
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
    )?;
    writeln!(
        w,
        r#"  <Relationship Target="/3D/3dmodel.model" Id="rel1" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>"#
    )?;
    writeln!(w, r#"</Relationships>"#)?;
    Ok(())
}

fn write_model_rels<W: Write + Seek>(w: &mut zip::ZipWriter<W>) -> std::io::Result<()> {
    writeln!(w, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        w,
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#
    )?;
    writeln!(
        w,
        r#"  <Relationship Target="/Metadata/model.xml" Id="rel-mat" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel/metadata"/>"#
    )?;
    writeln!(w, r#"</Relationships>"#)?;
    Ok(())
}

fn write_model<W: Write + Seek>(
    w: &mut zip::ZipWriter<W>,
    mesh: &Mesh,
    material: Option<&Material>,
    options: &ThreeMfOptions,
) -> std::io::Result<()> {
    writeln!(w, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        w,
        r#"<model unit="{}" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">"#,
        options.units
    )?;

    // Metadata
    writeln!(
        w,
        r#"  <metadata name="Application">{}</metadata>"#,
        escape_xml(&options.application)
    )?;

    // Resources block
    let has_material = material.is_some() && options.embed_material;
    if has_material {
        writeln!(w, r#"  <resources>"#)?;
        // Material resource with base colors and properties
        writeln!(
            w,
            r##"    <base id="1" name="{}" displaycolor="#{:06X}"/>"##,
            escape_xml(&material.unwrap().name),
            material_color(material.unwrap()),
        )?;
        writeln!(w, r#"  </resources>"#)?;
    } else {
        writeln!(w, r#"  <resources/>"#)?;
    }

    // Build block
    writeln!(w, r#"  <build>"#)?;
    writeln!(
        w,
        r#"    <item objectid="1"{} transform="1 0 0 0 1 0 0 0 1"/>"#,
        if has_material {
            " resourceid=\"1\"".to_string()
        } else {
            String::new()
        },
    )?;
    writeln!(w, r#"  </build>"#)?;

    // Object — mesh definition
    writeln!(
        w,
        r#"  <object id="1" type="model"{}{}>"#,
        if has_material { " pid=\"1\"" } else { "" },
        if has_material {
            " pindex=\"1\"".to_string()
        } else {
            String::new()
        }
    )?;
    writeln!(w, r#"    <mesh>"#)?;

    // Vertices
    let vert_set: Vec<_> = mesh
        .triangles
        .iter()
        .flat_map(|t| [t.v1, t.v2, t.v3])
        .collect();
    writeln!(w, r#"      <vertices>"#)?;
    for v in &vert_set {
        writeln!(
            w,
            r#"        <vertex x="{:.6}" y="{:.6}" z="{:.6}"/>"#,
            v.x, v.y, v.z
        )?;
    }
    writeln!(w, r#"      </vertices>"#)?;

    // Triangles (indices into the vertex list)
    writeln!(w, r#"      <triangles>"#)?;
    for i in 0..mesh.triangles.len() {
        let idx = i * 3;
        writeln!(
            w,
            r#"        <triangle v1="{}" v2="{}" v3="{}"/>"#,
            idx,
            idx + 1,
            idx + 2
        )?;
    }
    writeln!(w, r#"      </triangles>"#)?;

    writeln!(w, r#"    </mesh>"#)?;
    writeln!(w, r#"  </object>"#)?;

    writeln!(w, r#"</model>"#)?;
    Ok(())
}

fn write_metadata<W: Write + Seek>(
    w: &mut zip::ZipWriter<W>,
    mat: &Material,
) -> std::io::Result<()> {
    writeln!(w, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(
        w,
        r#"<metadata xmlns="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel/metadata">"#
    )?;
    writeln!(
        w,
        r#"  <material name="{}" density="{:.1}">"#,
        escape_xml(&mat.name),
        mat.density
    )?;

    // Rheology metadata
    writeln!(
        w,
        r#"    <rheology model="{}">"#,
        rheology_model_name(&mat.rheology)
    )?;
    let params = rheology_params(&mat.rheology);
    for (key, val) in &params {
        writeln!(
            w,
            r#"      <param name="{}" value="{}"/>"#,
            escape_xml(key),
            val,
        )?;
    }
    writeln!(w, r#"    </rheology>"#)?;

    // Curing metadata
    if let Some(ref curing) = mat.curing {
        writeln!(w, r#"    <curing>"#)?;
        writeln!(
            w,
            r#"      <uv_intensity value="{:.1}"/>"#,
            curing.uv_intensity
        )?;
        writeln!(
            w,
            r#"      <exposure_time value="{:.1}"/>"#,
            curing.exposure_time
        )?;
        writeln!(w, r#"      <wavelength value="{:.0}"/>"#, curing.wavelength)?;
        writeln!(w, r#"    </curing>"#)?;
    }

    // Coaxial metadata
    if let Some(ref coaxial) = mat.coaxial {
        writeln!(w, r#"    <coaxial>"#)?;
        writeln!(
            w,
            r#"      <crosslinker>{}</crosslinker>"#,
            escape_xml(&coaxial.crosslinker_name)
        )?;
        writeln!(
            w,
            r#"      <flow_ratio value="{:.3}"/>"#,
            coaxial.flow_ratio
        )?;
        writeln!(
            w,
            r#"      <concentration value="{:.3}"/>"#,
            coaxial.concentration
        )?;
        writeln!(w, r#"    </coaxial>"#)?;
    }

    writeln!(w, r#"  </material>"#)?;
    writeln!(w, r#"</metadata>"#)?;
    Ok(())
}

// -- Helpers --

/// Escape XML special characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Generate a color from material properties for display.
fn material_color(mat: &Material) -> u32 {
    // Simple hash-based color from material name
    let hash: u32 = mat
        .name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let r = ((hash >> 16) & 0xFF).max(0x40);
    let g = ((hash >> 8) & 0xFF).max(0x40);
    let b = (hash & 0xFF).max(0x40);
    (r << 16) | (g << 8) | b
}

/// Get a human-readable name for the rheology model type.
fn rheology_model_name(model: &tpt_core::RheologyModel) -> &str {
    use tpt_core::RheologyModel;
    match model {
        RheologyModel::Newtonian { .. } => "Newtonian",
        RheologyModel::CarreauYasuda { .. } => "Carreau-Yasuda",
        RheologyModel::HerschelBulkley { .. } => "Herschel-Bulkley",
        RheologyModel::Bingham { .. } => "Bingham",
        RheologyModel::Custom { .. } => "Custom",
    }
}

/// Extract rheology parameters as key-value pairs.
fn rheology_params(model: &tpt_core::RheologyModel) -> Vec<(&str, f64)> {
    use tpt_core::RheologyModel;
    match model {
        RheologyModel::Newtonian { viscosity } => vec![("viscosity", *viscosity)],
        RheologyModel::CarreauYasuda {
            eta_zero,
            eta_inf,
            lambda,
            a,
            n,
        } => vec![
            ("eta_zero", *eta_zero),
            ("eta_inf", *eta_inf),
            ("lambda", *lambda),
            ("a", *a),
            ("n", *n),
        ],
        RheologyModel::HerschelBulkley { tau_yield, k, n } => {
            vec![("tau_yield", *tau_yield), ("k", *k), ("n", *n)]
        }
        RheologyModel::Bingham { tau_yield, mu_p } => {
            vec![("tau_yield", *tau_yield), ("mu_p", *mu_p)]
        }
        RheologyModel::Custom { .. } => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::Point3;
    use std::io::Read;
    use tpt_core::RheologyModel;

    fn test_mesh() -> Mesh {
        let mut mesh = Mesh::new();
        let v = |x, y, z| Point3::new(x, y, z);
        mesh.vertices = vec![
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
            v(0.0, 0.0, 1.0),
            v(1.0, 0.0, 1.0),
            v(1.0, 1.0, 1.0),
            v(0.0, 1.0, 1.0),
        ];
        mesh.triangles = vec![
            crate::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(1.0, 0.0, 0.0),
                v3: v(1.0, 1.0, 0.0),
                normal: v(0.0, 0.0, -1.0),
            },
            crate::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(1.0, 1.0, 0.0),
                v3: v(0.0, 1.0, 0.0),
                normal: v(0.0, 0.0, -1.0),
            },
        ];
        mesh
    }

    #[test]
    fn test_export_3mf_bytes() {
        let mesh = test_mesh();
        let material = Material {
            name: "Test GelMA".to_string(),
            density: 1020.0,
            rheology: RheologyModel::CarreauYasuda {
                eta_zero: 50.0,
                eta_inf: 0.05,
                lambda: 0.3,
                a: 0.4,
                n: 0.4,
            },
            curing: None,
            coaxial: None,
        };
        let result = export_3mf_bytes(&mesh, Some(&material), &ThreeMfOptions::default());
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());

        // Verify it's a valid zip archive
        let cursor = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        assert!(archive.by_name("[Content_Types].xml").is_ok());
        assert!(archive.by_name("_rels/.rels").is_ok());
        assert!(archive.by_name("3D/3dmodel.model").is_ok());
        assert!(archive.by_name("Metadata/model.xml").is_ok());
    }

    #[test]
    fn test_export_3mf_no_material() {
        let mesh = test_mesh();
        let result = export_3mf_bytes(&mesh, None, &ThreeMfOptions::default());
        assert!(result.is_ok());
        let bytes = result.unwrap();
        let cursor = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        assert!(archive.by_name("3D/3dmodel.model").is_ok());
        // No metadata without material
        assert!(archive.by_name("Metadata/model.xml").is_err());
    }

    #[test]
    fn test_export_empty_mesh_fails() {
        let mesh = Mesh::new();
        let result = export_3mf_bytes(&mesh, None, &ThreeMfOptions::default());
        assert!(result.is_err());
        match result {
            Err(ThreeMfError::InvalidMesh(msg)) => assert!(msg.contains("no triangles")),
            _ => panic!("expected InvalidMesh error"),
        }
    }

    #[test]
    fn test_mesh_to_geometry_data() {
        let mesh = test_mesh();
        let material = Material {
            name: "Alginate".to_string(),
            density: 1010.0,
            rheology: RheologyModel::Newtonian { viscosity: 1.0 },
            curing: None,
            coaxial: None,
        };
        let result = mesh_to_3mf_geometry_data(&mesh, Some(&material));
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.format, GeometryFormat::ThreeMf);
        assert!(!data.data.is_empty());
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(
            escape_xml("a & b < c > d \" e '"),
            "a &amp; b &lt; c &gt; d &quot; e &apos;"
        );
    }

    #[test]
    fn test_rheology_model_name() {
        use tpt_core::RheologyModel;
        assert_eq!(
            rheology_model_name(&RheologyModel::Newtonian { viscosity: 1.0 }),
            "Newtonian"
        );
        assert_eq!(
            rheology_model_name(&RheologyModel::Bingham {
                tau_yield: 10.0,
                mu_p: 5.0
            }),
            "Bingham"
        );
    }

    #[test]
    fn test_export_with_all_material_properties() {
        let mesh = test_mesh();
        let material = Material {
            name: "Full Material".to_string(),
            density: 1050.0,
            rheology: RheologyModel::HerschelBulkley {
                tau_yield: 50.0,
                k: 10.0,
                n: 0.5,
            },
            curing: Some(tpt_core::CuringParams {
                uv_intensity: 10.0,
                exposure_time: 30.0,
                wavelength: 365.0,
            }),
            coaxial: Some(tpt_core::CoaxialParams {
                crosslinker_name: "CaCl₂".to_string(),
                flow_ratio: 0.1,
                concentration: 0.1,
            }),
        };
        let bytes = export_3mf_bytes(&mesh, Some(&material), &ThreeMfOptions::default()).unwrap();
        let cursor = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        // Read metadata to verify content
        let mut meta = String::new();
        archive
            .by_name("Metadata/model.xml")
            .unwrap()
            .read_to_string(&mut meta)
            .unwrap();
        assert!(meta.contains("Herschel-Bulkley"));
        assert!(meta.contains("uv_intensity"));
        assert!(meta.contains("CaCl₂"));
    }
}
