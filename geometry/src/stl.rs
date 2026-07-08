// STL import/export
// Licensed under Apache 2.0

use crate::mesh::{Mesh, Triangle};
use nalgebra::Point3;
use std::io::{BufRead, BufReader, Cursor, Read, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StlError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid STL format")]
    InvalidFormat,
    #[error("Binary STL header mismatch")]
    HeaderMismatch,
}

/// Import STL file (ASCII or binary)
pub fn import_stl<P: AsRef<Path>>(path: P) -> Result<Mesh, StlError> {
    let file = std::fs::File::open(path.as_ref())?;
    let mut reader = BufReader::new(file);

    // Check for binary STL (first 5 bytes should be "solid" for ASCII, but binary starts with 80-byte header)
    let mut magic = [0u8; 5];
    reader.read_exact(&mut magic)?;

    if &magic == b"solid" {
        // ASCII STL
        import_ascii_stl(reader)
    } else {
        // Binary STL - need to seek back
        use std::io::Seek;
        reader.seek(std::io::SeekFrom::Start(0))?;
        import_binary_stl(reader)
    }
}

fn import_ascii_stl<R: BufRead>(reader: R) -> Result<Mesh, StlError> {
    let mut mesh = Mesh::new();
    let mut vertices = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "vertex" if parts.len() >= 4 => {
                let x: f64 = parts[1].parse().map_err(|_| StlError::InvalidFormat)?;
                let y: f64 = parts[2].parse().map_err(|_| StlError::InvalidFormat)?;
                let z: f64 = parts[3].parse().map_err(|_| StlError::InvalidFormat)?;
                vertices.push(Point3::new(x, y, z));
            }
            "endloop" | "endsolid" | "facet" => {}
            _ => {}
        }

        // Every 3 vertices form a triangle
        if vertices.len() == 3 {
            let v1 = vertices[0];
            let v2 = vertices[1];
            let v3 = vertices[2];
            let normal = (v2 - v1).cross(&(v3 - v1)).normalize();

            mesh.vertices.extend(vertices.clone());
            mesh.triangles.push(Triangle {
                v1,
                v2,
                v3,
                normal: normal.into(),
            });
            vertices.clear();
        }
    }

    Ok(mesh)
}

fn import_binary_stl<R: BufRead>(reader: R) -> Result<Mesh, StlError> {
    use byteorder::{LittleEndian, ReadBytesExt};

    let mut reader = reader;

    // Skip 80-byte header
    let mut header = [0u8; 80];
    reader.read_exact(&mut header)?;

    // Read triangle count
    let triangle_count = reader.read_u32::<LittleEndian>()? as usize;

    let mut mesh = Mesh::new();

    for _ in 0..triangle_count {
        // Read normal
        let normal_x = reader.read_f32::<LittleEndian>()? as f64;
        let normal_y = reader.read_f32::<LittleEndian>()? as f64;
        let normal_z = reader.read_f32::<LittleEndian>()? as f64;

        // Read vertices
        let mut vertices = [Point3::origin(); 3];
        for v in &mut vertices {
            v.x = reader.read_f32::<LittleEndian>()? as f64;
            v.y = reader.read_f32::<LittleEndian>()? as f64;
            v.z = reader.read_f32::<LittleEndian>()? as f64;
        }

        // Read attribute byte count (2 bytes)
        let _ = reader.read_u16::<LittleEndian>()?;

        mesh.vertices.extend(vertices.iter().copied());
        mesh.triangles.push(Triangle {
            v1: vertices[0],
            v2: vertices[1],
            v3: vertices[2],
            normal: Point3::new(normal_x, normal_y, normal_z),
        });
    }

    Ok(mesh)
}

/// Convert a Mesh to GeometryData (serialized as ASCII STL bytes).
pub fn mesh_to_geometry_data(mesh: &Mesh) -> tpt_core::GeometryData {
    let mut buf = Vec::new();
    // Write ASCII STL into the buffer
    writeln!(buf, "solid tpt-biocad").unwrap();
    for tri in &mesh.triangles {
        writeln!(
            buf,
            "  facet normal {} {} {}",
            tri.normal.x, tri.normal.y, tri.normal.z
        )
        .unwrap();
        writeln!(buf, "    outer loop").unwrap();
        writeln!(buf, "      vertex {} {} {}", tri.v1.x, tri.v1.y, tri.v1.z).unwrap();
        writeln!(buf, "      vertex {} {} {}", tri.v2.x, tri.v2.y, tri.v2.z).unwrap();
        writeln!(buf, "      vertex {} {} {}", tri.v3.x, tri.v3.y, tri.v3.z).unwrap();
        writeln!(buf, "    endloop").unwrap();
        writeln!(buf, "  endfacet").unwrap();
    }
    writeln!(buf, "endsolid tpt-biocad").unwrap();

    tpt_core::GeometryData {
        format: tpt_core::GeometryFormat::Stl,
        data: buf,
    }
}

/// Convert GeometryData back to a Mesh (reads the STL bytes from the data field).
pub fn geometry_data_to_mesh(data: &tpt_core::GeometryData) -> Result<Mesh, String> {
    let cursor = Cursor::new(&data.data);
    let reader = std::io::BufReader::new(cursor);
    // Use the existing import logic (detect ASCII vs binary)
    import_ascii_stl(reader).map_err(|e| e.to_string())
}

/// Export mesh to ASCII STL format
pub fn export_stl<P: AsRef<Path>>(mesh: &Mesh, path: P) -> Result<(), StlError> {
    let file = std::fs::File::create(path.as_ref())?;
    let mut writer = std::io::BufWriter::new(file);

    writeln!(writer, "solid tpt-biocad")?;

    for tri in &mesh.triangles {
        writeln!(
            writer,
            "  facet normal {} {} {}",
            tri.normal.x, tri.normal.y, tri.normal.z
        )?;
        writeln!(writer, "    outer loop")?;
        writeln!(
            writer,
            "      vertex {} {} {}",
            tri.v1.x, tri.v1.y, tri.v1.z
        )?;
        writeln!(
            writer,
            "      vertex {} {} {}",
            tri.v2.x, tri.v2.y, tri.v2.z
        )?;
        writeln!(
            writer,
            "      vertex {} {} {}",
            tri.v3.x, tri.v3.y, tri.v3.z
        )?;
        writeln!(writer, "    endloop")?;
        writeln!(writer, "  endfacet")?;
    }

    writeln!(writer, "endsolid tpt-biocad")?;

    Ok(())
}
