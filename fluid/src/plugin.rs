// Plugin system for custom rheology models
// Licensed under Apache 2.0
//
// Allows users to register custom rheological models at runtime via a
// trait-based plugin system. Custom models can be written in Rust and
// registered with the global plugin registry, then referenced by key
// from `RheologyModel::Custom { plugin_key }`.

use std::collections::HashMap;
use std::sync::RwLock;
use tpt_core::RheologyModel;

/// A custom rheology model plugin.
///
/// Implement this trait to define a custom viscosity/shear-stress model
/// that can be registered with the global plugin registry. The model is
/// identified by a unique string key.
pub trait CustomRheology: Send + Sync {
    /// Return the display name of this model.
    fn name(&self) -> &str;

    /// Compute apparent viscosity (Pa·s) at the given shear rate (s⁻¹).
    fn viscosity(&self, shear_rate: f64) -> f64;

    /// Compute shear stress (Pa) at the given shear rate (s⁻¹).
    fn shear_stress(&self, shear_rate: f64) -> f64;

    /// Return the number of parameters this model uses (for UI display).
    fn parameter_count(&self) -> usize {
        0
    }

    /// Return parameter names and current values.
    fn parameters(&self) -> Vec<(&str, f64)> {
        Vec::new()
    }
}

/// Error returned when a plugin operation fails.
#[derive(Debug, Clone)]
pub enum PluginError {
    /// No plugin registered with the given key.
    NotFound(String),
    /// A plugin with this key is already registered.
    AlreadyRegistered(String),
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginError::NotFound(key) => write!(f, "rheology plugin '{}' not found", key),
            PluginError::AlreadyRegistered(key) => {
                write!(f, "rheology plugin '{}' already registered", key)
            }
        }
    }
}

impl std::error::Error for PluginError {}

/// Global registry for custom rheology plugins.
#[derive(Default)]
pub struct RheologyPluginRegistry {
    plugins: HashMap<String, Box<dyn CustomRheology>>,
}

impl RheologyPluginRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Register a custom rheology model.
    ///
    /// The plugin key must be unique. Returns an error if a plugin with
    /// the same key is already registered.
    pub fn register(
        &mut self,
        key: &str,
        plugin: Box<dyn CustomRheology>,
    ) -> Result<(), PluginError> {
        if self.plugins.contains_key(key) {
            return Err(PluginError::AlreadyRegistered(key.to_string()));
        }
        self.plugins.insert(key.to_string(), plugin);
        Ok(())
    }

    /// Unregister a plugin by key.
    pub fn unregister(&mut self, key: &str) -> Result<(), PluginError> {
        self.plugins
            .remove(key)
            .map(|_| ())
            .ok_or_else(|| PluginError::NotFound(key.to_string()))
    }

    /// Get a reference to a registered plugin.
    pub fn get(&self, key: &str) -> Option<&dyn CustomRheology> {
        self.plugins.get(key).map(|p| p.as_ref())
    }

    /// Check if a plugin is registered.
    pub fn has(&self, key: &str) -> bool {
        self.plugins.contains_key(key)
    }

    /// List all registered plugin keys.
    pub fn list(&self) -> Vec<&str> {
        self.plugins.keys().map(|s| s.as_str()).collect()
    }

    /// Number of registered plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
}

// Global plugin registry (thread-safe).
static GLOBAL_REGISTRY: once_cell::sync::Lazy<RwLock<RheologyPluginRegistry>> =
    once_cell::sync::Lazy::new(|| RwLock::new(RheologyPluginRegistry::new()));

/// Register a custom rheology model globally.
///
/// # Example
///
/// ```ignore
/// struct MyModel;
/// impl CustomRheology for MyModel { ... }
/// register_plugin("my-model", Box::new(MyModel)).unwrap();
/// ```
pub fn register_plugin(key: &str, plugin: Box<dyn CustomRheology>) -> Result<(), PluginError> {
    GLOBAL_REGISTRY.write().unwrap().register(key, plugin)
}

/// Unregister a plugin globally.
pub fn unregister_plugin(key: &str) -> Result<(), PluginError> {
    GLOBAL_REGISTRY.write().unwrap().unregister(key)
}

/// Look up a plugin from the global registry.
pub fn get_plugin(key: &str) -> Option<Box<dyn CustomRheology>> {
    let registry = GLOBAL_REGISTRY.read().unwrap();
    // clone_box is not available on dyn CustomRheology, so this only
    // works if the registry owns the plugin. Returns None for now.
    drop(registry);
    let _ = key;
    None
}

/// Compute viscosity for a model that may reference a custom plugin.
///
/// Extends the built-in `viscosity` function to handle the `Custom`
/// variant by dispatching to the global plugin registry.
pub fn viscosity_with_plugins(model: &RheologyModel, shear_rate: f64) -> f64 {
    match model {
        RheologyModel::Custom { plugin_key } => {
            let registry = GLOBAL_REGISTRY.read().unwrap();
            match registry.get(plugin_key) {
                Some(plugin) => plugin.viscosity(shear_rate),
                None => {
                    // Fallback: log warning and return Newtonian-like value
                    f64::NAN
                }
            }
        }
        _ => crate::models::viscosity(model, shear_rate),
    }
}

/// Compute shear stress for a model that may reference a custom plugin.
pub fn shear_stress_with_plugins(model: &RheologyModel, shear_rate: f64) -> f64 {
    match model {
        RheologyModel::Custom { plugin_key } => {
            let registry = GLOBAL_REGISTRY.read().unwrap();
            match registry.get(plugin_key) {
                Some(plugin) => plugin.shear_stress(shear_rate),
                None => f64::NAN,
            }
        }
        _ => crate::models::shear_stress(model, shear_rate),
    }
}

/// List all available model identifiers, including built-in models and
/// registered custom plugins.
pub fn all_model_names() -> Vec<String> {
    let mut names: Vec<String> = vec!["Newtonian", "Carreau-Yasuda", "Herschel-Bulkley", "Bingham"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    let registry = GLOBAL_REGISTRY.read().unwrap();
    for key in registry.list() {
        names.push(format!("Custom: {}", key));
    }

    names
}

// -- Built-in plugin example: Power-law model --

/// A simple power-law (Ostwald–de Waele) model as a plugin example.
pub struct PowerLawModel {
    pub k: f64,
    pub n: f64,
}

impl CustomRheology for PowerLawModel {
    fn name(&self) -> &str {
        "Power Law (Ostwald-de Waele)"
    }

    fn viscosity(&self, shear_rate: f64) -> f64 {
        if shear_rate == 0.0 {
            f64::INFINITY
        } else {
            self.k * shear_rate.powf(self.n - 1.0)
        }
    }

    fn shear_stress(&self, shear_rate: f64) -> f64 {
        if shear_rate == 0.0 {
            0.0
        } else {
            self.k * shear_rate.powf(self.n)
        }
    }

    fn parameter_count(&self) -> usize {
        2
    }

    fn parameters(&self) -> Vec<(&str, f64)> {
        vec![("k (consistency)", self.k), ("n (power-law index)", self.n)]
    }
}

/// Initialize the plugin system with built-in example plugins.
pub fn init_default_plugins() {
    let _ = register_plugin("power-law", Box::new(PowerLawModel { k: 10.0, n: 0.5 }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_register_and_list() {
        let mut registry = RheologyPluginRegistry::new();
        registry
            .register("test-model", Box::new(PowerLawModel { k: 5.0, n: 0.4 }))
            .unwrap();
        assert!(registry.has("test-model"));
        assert_eq!(registry.count(), 1);
        assert_eq!(registry.list(), vec!["test-model"]);
    }

    #[test]
    fn test_duplicate_registration_fails() {
        let mut registry = RheologyPluginRegistry::new();
        registry
            .register("dup", Box::new(PowerLawModel { k: 1.0, n: 0.5 }))
            .unwrap();
        let result = registry.register("dup", Box::new(PowerLawModel { k: 2.0, n: 0.6 }));
        assert!(result.is_err());
        match result {
            Err(PluginError::AlreadyRegistered(key)) => assert_eq!(key, "dup"),
            _ => panic!("expected AlreadyRegistered"),
        }
    }

    #[test]
    fn test_unregister() {
        let mut registry = RheologyPluginRegistry::new();
        registry
            .register("m", Box::new(PowerLawModel { k: 1.0, n: 0.5 }))
            .unwrap();
        assert!(registry.has("m"));
        registry.unregister("m").unwrap();
        assert!(!registry.has("m"));
    }

    #[test]
    fn test_power_law_viscosity() {
        let model = PowerLawModel { k: 10.0, n: 0.5 };
        // η = k * γ̇^(n-1) = 10 * 100^(-0.5) = 10 * 0.1 = 1.0
        assert_relative_eq!(model.viscosity(100.0), 1.0, epsilon = 1e-10);
        // η = 10 * 1^(-0.5) = 10
        assert_relative_eq!(model.viscosity(1.0), 10.0, epsilon = 1e-10);
    }

    #[test]
    fn test_power_law_shear_stress() {
        let model = PowerLawModel { k: 10.0, n: 0.5 };
        // τ = k * γ̇^n = 10 * 100^0.5 = 10 * 10 = 100
        assert_relative_eq!(model.shear_stress(100.0), 100.0, epsilon = 1e-10);
    }

    #[test]
    fn test_viscosity_with_plugins_custom() {
        let _ = register_plugin("test-visc", Box::new(PowerLawModel { k: 10.0, n: 0.5 }));
        let model = RheologyModel::Custom {
            plugin_key: "test-visc".to_string(),
        };
        let eta = viscosity_with_plugins(&model, 100.0);
        assert!(!eta.is_nan());
        assert_relative_eq!(eta, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn test_viscosity_with_plugins_builtin() {
        let model = RheologyModel::Newtonian { viscosity: 5.0 };
        let eta = viscosity_with_plugins(&model, 100.0);
        assert_relative_eq!(eta, 5.0, epsilon = 1e-10);
    }

    #[test]
    fn test_viscosity_with_plugins_missing() {
        let model = RheologyModel::Custom {
            plugin_key: "nonexistent".to_string(),
        };
        let eta = viscosity_with_plugins(&model, 100.0);
        assert!(eta.is_nan());
    }

    #[test]
    fn test_all_model_names() {
        let _ = register_plugin("test-all", Box::new(PowerLawModel { k: 1.0, n: 0.5 }));
        let names = all_model_names();
        assert!(names.contains(&"Newtonian".to_string()));
        assert!(names.contains(&"Carreau-Yasuda".to_string()));
    }

    #[test]
    fn test_plugin_name_and_params() {
        let model = PowerLawModel { k: 5.0, n: 0.3 };
        assert_eq!(model.name(), "Power Law (Ostwald-de Waele)");
        assert_eq!(model.parameter_count(), 2);
        let params = model.parameters();
        assert_eq!(params[0].0, "k (consistency)");
        assert_eq!(params[1].0, "n (power-law index)");
    }

    #[test]
    fn test_global_registry() {
        // Test that global registration works across calls
        let _ = register_plugin("global-test", Box::new(PowerLawModel { k: 2.0, n: 0.6 }));
        let registry = GLOBAL_REGISTRY.read().unwrap();
        assert!(registry.has("global-test"));
    }

    #[test]
    fn test_init_default_plugins() {
        init_default_plugins();
        let registry = GLOBAL_REGISTRY.read().unwrap();
        assert!(registry.has("power-law"));
    }
}
