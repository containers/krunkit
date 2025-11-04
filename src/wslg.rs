// SPDX-License-Identifier: Apache-2.0

//! WSLg-like functionality for macOS
//!
//! This module provides WSLg-inspired features for running Linux GUI applications
//! on macOS hosts using Wayland, Weston, and PulseAudio.

use std::path::PathBuf;

/// Default GPU width in pixels for WSLg GUI mode
pub const DEFAULT_GPU_WIDTH: u32 = 1920;

/// Default GPU height in pixels for WSLg GUI mode
pub const DEFAULT_GPU_HEIGHT: u32 = 1080;

/// Configuration for WSLg-like features
#[derive(Clone, Debug, Default)]
pub struct WslgConfig {
    /// Enable GUI application support
    pub gui_enabled: bool,

    /// Enable audio support
    pub audio_enabled: bool,

    /// Path to Wayland socket (forwarded via vsock)
    pub wayland_socket: Option<PathBuf>,

    /// Path to PulseAudio socket (forwarded via vsock)
    pub pulseaudio_socket: Option<PathBuf>,

    /// GPU width in pixels
    pub gpu_width: Option<u32>,

    /// GPU height in pixels
    pub gpu_height: Option<u32>,

    /// Shared directory for socket communication
    pub socket_dir: Option<PathBuf>,
}

impl WslgConfig {
    /// Create a new WSLg configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable GUI support with specified resolution
    pub fn with_gui(mut self, width: u32, height: u32) -> Self {
        self.gui_enabled = true;
        self.gpu_width = Some(width);
        self.gpu_height = Some(height);
        self
    }

    /// Enable audio support
    pub fn with_audio(mut self) -> Self {
        self.audio_enabled = true;
        self
    }

    /// Set the socket directory for communication
    pub fn with_socket_dir(mut self, dir: PathBuf) -> Self {
        self.socket_dir = Some(dir);
        self
    }

    /// Check if any WSLg-like features are enabled
    pub fn is_enabled(&self) -> bool {
        self.gui_enabled || self.audio_enabled
    }

    /// Get the required virtio devices for WSLg features
    pub fn required_devices(&self) -> Vec<String> {
        let mut devices = Vec::new();

        if self.gui_enabled {
            // Add virtio-gpu device with resolution if specified, otherwise use defaults
            match (self.gpu_width, self.gpu_height) {
                (Some(width), Some(height)) => {
                    devices.push(format!("virtio-gpu,width={},height={}", width, height));
                }
                _ => {
                    // Use default resolution if dimensions not specified
                    devices.push(format!(
                        "virtio-gpu,width={},height={}",
                        DEFAULT_GPU_WIDTH, DEFAULT_GPU_HEIGHT
                    ));
                }
            }

            // Add virtio-input devices for keyboard and mouse
            devices.push("virtio-input,keyboard".to_string());
            devices.push("virtio-input,pointing".to_string());
        }

        if self.gui_enabled || self.audio_enabled {
            // Add virtio-fs for socket sharing
            if let Some(ref socket_dir) = self.socket_dir {
                devices.push(format!(
                    "virtio-fs,sharedDir={},mountTag=wslg",
                    socket_dir.display()
                ));
            }
        }

        devices
    }

    /// Get environment variables to set in the guest
    pub fn guest_environment(&self) -> Vec<(String, String)> {
        let mut env = Vec::new();

        if self.gui_enabled {
            env.push(("WAYLAND_DISPLAY".to_string(), "wayland-0".to_string()));
            env.push(("XDG_RUNTIME_DIR".to_string(), "/run/user/1000".to_string()));
            env.push(("XDG_SESSION_TYPE".to_string(), "wayland".to_string()));
        }

        if self.audio_enabled {
            env.push((
                "PULSE_SERVER".to_string(),
                "unix:/run/user/1000/pulse/native".to_string(),
            ));
        }

        env
    }
}

/// Application discovery for finding Linux GUI applications
#[derive(Clone, Debug)]
pub struct ApplicationDiscovery {
    /// Paths to search for .desktop files
    search_paths: Vec<PathBuf>,
}

impl Default for ApplicationDiscovery {
    fn default() -> Self {
        Self {
            search_paths: vec![
                PathBuf::from("/usr/share/applications"),
                PathBuf::from("/usr/local/share/applications"),
                // Note: User-specific paths like ~/.local/share/applications
                // should be added by calling add_search_path() after construction,
                // as PathBuf does not automatically expand ~ or environment variables.
                // Use std::env::var("HOME") to construct user-specific paths.
            ],
        }
    }
}

impl ApplicationDiscovery {
    /// Create a new application discovery instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a custom search path
    pub fn add_search_path(&mut self, path: PathBuf) {
        self.search_paths.push(path);
    }

    /// Get the list of search paths
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }
}

/// Represents a Linux GUI application found via .desktop files
#[derive(Clone, Debug)]
pub struct GuiApplication {
    /// Application name
    pub name: String,

    /// Application description
    pub description: Option<String>,

    /// Executable command
    pub exec: String,

    /// Icon path
    pub icon: Option<PathBuf>,

    /// Desktop file path
    pub desktop_file: PathBuf,

    /// Categories
    pub categories: Vec<String>,
}

impl GuiApplication {
    /// Create a new GUI application descriptor
    pub fn new(name: String, exec: String, desktop_file: PathBuf) -> Self {
        Self {
            name,
            description: None,
            exec,
            icon: None,
            desktop_file,
            categories: Vec::new(),
        }
    }

    /// Set the application description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the application icon
    pub fn with_icon(mut self, icon: PathBuf) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Add a category
    pub fn add_category(&mut self, category: String) {
        self.categories.push(category);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wslg_config_default() {
        let config = WslgConfig::new();
        assert!(!config.is_enabled());
        assert!(!config.gui_enabled);
        assert!(!config.audio_enabled);
    }

    #[test]
    fn test_wslg_config_with_gui() {
        let config = WslgConfig::new().with_gui(1920, 1080);
        assert!(config.is_enabled());
        assert!(config.gui_enabled);
        assert_eq!(config.gpu_width, Some(1920));
        assert_eq!(config.gpu_height, Some(1080));
    }

    #[test]
    fn test_wslg_config_with_audio() {
        let config = WslgConfig::new().with_audio();
        assert!(config.is_enabled());
        assert!(config.audio_enabled);
    }

    #[test]
    fn test_wslg_config_required_devices() {
        let config = WslgConfig::new().with_gui(800, 600);
        let devices = config.required_devices();
        assert!(devices.iter().any(|d| d.contains("virtio-gpu")));
        assert!(devices.iter().any(|d| d.contains("virtio-input,keyboard")));
        assert!(devices.iter().any(|d| d.contains("virtio-input,pointing")));
    }

    #[test]
    fn test_wslg_config_guest_environment() {
        let config = WslgConfig::new().with_gui(800, 600).with_audio();
        let env = config.guest_environment();
        assert!(env
            .iter()
            .any(|(k, _)| k == "WAYLAND_DISPLAY"));
        assert!(env.iter().any(|(k, _)| k == "PULSE_SERVER"));
    }

    #[test]
    fn test_application_discovery_default_paths() {
        let discovery = ApplicationDiscovery::new();
        assert!(!discovery.search_paths().is_empty());
        assert!(discovery
            .search_paths()
            .iter()
            .any(|p| p.to_str().unwrap().contains("applications")));
    }

    #[test]
    fn test_gui_application_creation() {
        let app = GuiApplication::new(
            "Test App".to_string(),
            "test-app".to_string(),
            PathBuf::from("/usr/share/applications/test.desktop"),
        );
        assert_eq!(app.name, "Test App");
        assert_eq!(app.exec, "test-app");
        assert!(app.description.is_none());
    }

    #[test]
    fn test_gui_application_with_description() {
        let app = GuiApplication::new(
            "Test App".to_string(),
            "test-app".to_string(),
            PathBuf::from("/usr/share/applications/test.desktop"),
        )
        .with_description("A test application".to_string());
        assert_eq!(app.description, Some("A test application".to_string()));
    }
}
