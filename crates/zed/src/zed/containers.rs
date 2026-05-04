use anyhow::{Result, anyhow};
use paths::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContainerConfig {
    pub name: String,
    pub description: String,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub cache_dir: PathBuf,
}

/// Returns the containers directory path.
pub fn containers_dir() -> PathBuf {
    home_dir().join(".config/zed/Containers")
}

/// Lists all available containers.
pub fn list_containers() -> Vec<ContainerConfig> {
    let dir = containers_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut containers = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    if let Ok(config) = load_container_config(entry.path()) {
                        containers.push(config);
                    }
                }
            }
        }
    }

    containers.sort_by(|a, b| a.name.cmp(&b.name));
    containers
}

/// Loads a container configuration from its directory.
fn load_container_config(container_dir: PathBuf) -> Result<ContainerConfig> {
    let config_path = container_dir.join("config.json");
    let content = fs::read_to_string(&config_path)?;
    let config: ContainerConfig = serde_json::from_str(&content)?;
    Ok(config)
}

/// Creates a new container with the given name and description.
pub fn create_container(name: &str, description: &str) -> Result<ContainerConfig> {
    if name.is_empty() {
        return Err(anyhow!("Container name cannot be empty"));
    }

    let containers_dir = containers_dir();
    fs::create_dir_all(&containers_dir)?;

    let container_dir = containers_dir.join(name);
    if container_dir.exists() {
        return Err(anyhow!("Container '{}' already exists", name));
    }

    // Create subdirectories
    let data_dir = container_dir.join("data");
    let config_dir = container_dir.join("config");
    let logs_dir = container_dir.join("logs");
    let cache_dir = container_dir.join("cache");

    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&logs_dir)?;
    fs::create_dir_all(&cache_dir)?;

    let config = ContainerConfig {
        name: name.to_string(),
        description: description.to_string(),
        data_dir,
        config_dir,
        logs_dir,
        cache_dir,
    };

    // Write config.json
    let config_path = container_dir.join("config.json");
    let config_json = serde_json::to_string_pretty(&config)?;
    fs::write(config_path, config_json)?;

    Ok(config)
}

/// Applies container paths from a config.json file.
/// This reads the container config and sets the appropriate Zed path overrides.
pub fn apply_container_paths(config_path: &str) -> Result<()> {
    let content = fs::read_to_string(config_path)?;
    let config: ContainerConfig = serde_json::from_str(&content)?;

    // Expand ~ in paths
    let data_dir_expanded = expand_tilde(&config.data_dir);
    let config_dir_expanded = expand_tilde(&config.config_dir);
    let logs_dir_expanded = expand_tilde(&config.logs_dir);
    let cache_dir_expanded = expand_tilde(&config.cache_dir);

    // Set custom paths
    paths::set_custom_data_dir(&data_dir_expanded.to_string_lossy());
    paths::set_custom_config_dir(&config_dir_expanded.to_string_lossy());
    paths::set_custom_logs_dir(&logs_dir_expanded.to_string_lossy());
    paths::set_custom_cache_dir(&cache_dir_expanded.to_string_lossy());

    Ok(())
}

/// Launches a new Zed window with the specified container.
pub fn launch_container(config: &ContainerConfig) -> Result<()> {
    let config_path = containers_dir().join(&config.name).join("config.json");

    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(&exe);

    // Pass the container config path as an environment variable
    cmd.env("ZED_CONTAINER_CONFIG", &config_path);

    // Spawn the process
    cmd.spawn()?;

    Ok(())
}

/// Deletes a container and all its associated files/directories.
pub fn delete_container(name: &str) -> Result<()> {
    let container_dir = containers_dir().join(name);
    if container_dir.exists() {
        fs::remove_dir_all(&container_dir)?;
    }
    Ok(())
}

/// Expands ~ in a path to the home directory.
fn expand_tilde(path: &Path) -> PathBuf {
    if let Some(path_str) = path.to_str() {
        if path_str.starts_with("~") {
            return home_dir().join(&path_str[2..]);
        }
    }
    path.to_path_buf()
}
