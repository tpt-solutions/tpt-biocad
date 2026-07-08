// TPT project file format (.tpt)
// Licensed under Apache 2.0

use crate::{Machine, Material, Profile};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, Write};
use zip::result::ZipError;
use zip::ZipWriter;

/// TPT project file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TptProject {
    pub geometry: Option<GeometryData>,
    pub material: Option<Material>,
    pub machine: Option<Machine>,
    pub profile: Option<Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryData {
    pub format: GeometryFormat,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GeometryFormat {
    Stl,
    Obj,
    ThreeMf,
    BRep,
}

impl TptProject {
    pub fn new() -> Self {
        Self {
            geometry: None,
            material: None,
            machine: None,
            profile: None,
        }
    }

    /// Save project to .tpt file (zip archive)
    pub fn save<W: Write + Seek>(&self, writer: W) -> Result<(), ZipError> {
        let mut zip = ZipWriter::new(writer);

        // Write JSON files
        if let Some(ref material) = self.material {
            let json =
                serde_json::to_vec(material).map_err(|e| ZipError::Io(std::io::Error::other(e)))?;
            zip.start_file("material.json", zip::write::FileOptions::default())?;
            zip.write_all(&json)?;
        }

        if let Some(ref machine) = self.machine {
            let json =
                serde_json::to_vec(machine).map_err(|e| ZipError::Io(std::io::Error::other(e)))?;
            zip.start_file("machine.json", zip::write::FileOptions::default())?;
            zip.write_all(&json)?;
        }

        if let Some(ref profile) = self.profile {
            let json =
                serde_json::to_vec(profile).map_err(|e| ZipError::Io(std::io::Error::other(e)))?;
            zip.start_file("profile.json", zip::write::FileOptions::default())?;
            zip.write_all(&json)?;
        }

        zip.finish()?;
        Ok(())
    }

    /// Load project from .tpt file
    pub fn load<R: Read + Seek>(reader: R) -> Result<Self, String> {
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;

        let mut project = Self::new();

        if let Ok(mut json_file) = archive.by_name("material.json") {
            let mut contents = String::new();
            json_file
                .read_to_string(&mut contents)
                .map_err(|e| e.to_string())?;
            project.material = Some(serde_json::from_str(&contents).map_err(|e| e.to_string())?);
        }

        if let Ok(mut json_file) = archive.by_name("machine.json") {
            let mut contents = String::new();
            json_file
                .read_to_string(&mut contents)
                .map_err(|e| e.to_string())?;
            project.machine = Some(serde_json::from_str(&contents).map_err(|e| e.to_string())?);
        }

        if let Ok(mut json_file) = archive.by_name("profile.json") {
            let mut contents = String::new();
            json_file
                .read_to_string(&mut contents)
                .map_err(|e| e.to_string())?;
            project.profile = Some(serde_json::from_str(&contents).map_err(|e| e.to_string())?);
        }

        Ok(project)
    }
}

impl Default for TptProject {
    fn default() -> Self {
        Self::new()
    }
}
