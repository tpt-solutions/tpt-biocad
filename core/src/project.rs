// TPT project file format (.tpt)
// Licensed under Apache 2.0

use crate::{Machine, Material, Profile};
use serde::{Deserialize, Serialize};
use std::io::Write;
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
    pub fn save<W: Write>(&self, writer: W) -> Result<(), zip::ZipError> {
        let mut zip = ZipWriter::new(writer);
        
        // Write JSON files
        if let Some(ref material) = self.material {
            let json = serde_json::to_vec(material).map_err(|_| zip::ZipError::Io)?;
            zip.start_file("material.json", zip::write::FileOptions::default())?;
            zip.write_all(&json)?;
        }
        
        if let Some(ref machine) = self.machine {
            let json = serde_json::to_vec(machine).map_err(|_| zip::ZipError::Io)?;
            zip.start_file("machine.json", zip::write::FileOptions::default())?;
            zip.write_all(&json)?;
        }
        
        if let Some(ref profile) = self.profile {
            let json = serde_json::to_vec(profile).map_err(|_| zip::ZipError::Io)?;
            zip.start_file("profile.json", zip::write::FileOptions::default())?;
            zip.write_all(&json)?;
        }
        
        zip.finish()?;
        Ok(())
    }

    /// Load project from .tpt file
    pub fn load<R: std::io::Read>(reader: R) -> Result<Self, String> {
        let archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
        
        let mut project = Self::new();
        
        if let Ok(file) = archive.file_names().find(|n| n == "material.json") {
            let mut json_file = archive.by_name(file).map_err(|e| e.to_string())?;
            let mut contents = String::new();
            json_file.read_to_string(&mut contents).map_err(|e| e.to_string())?;
            project.material = Some(serde_json::from_str(&contents).map_err(|e| e.to_string())?);
        }
        
        if let Ok(file) = archive.file_names().find(|n| n == "machine.json") {
            let mut json_file = archive.by_name(file).map_err(|e| e.to_string())?;
            let mut contents = String::new();
            json_file.read_to_string(&mut contents).map_err(|e| e.to_string())?;
            project.machine = Some(serde_json::from_str(&contents).map_err(|e| e.to_string())?);
        }
        
        if let Ok(file) = archive.file_names().find(|n| n == "profile.json") {
            let mut json_file = archive.by_name(file).map_err(|e| e.to_string())?;
            let mut contents = String::new();
            json_file.read_to_string(&mut contents).map_err(|e| e.to_string())?;
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