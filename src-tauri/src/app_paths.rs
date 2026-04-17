use std::path::PathBuf;

pub const CURRENT_APP_DIR_NAME: &str = "NexusModManager";
pub const LEGACY_APP_DIR_NAME: &str = concat!("ST", "S2ModManager");

fn with_app_name(base: Option<PathBuf>, app_name: &str) -> Option<PathBuf> {
    base.map(|dir| dir.join(app_name))
}

pub fn current_config_dir() -> Option<PathBuf> {
    with_app_name(dirs::config_dir(), CURRENT_APP_DIR_NAME)
}

pub fn legacy_config_dir() -> Option<PathBuf> {
    with_app_name(dirs::config_dir(), LEGACY_APP_DIR_NAME)
}

pub fn writable_config_dir() -> PathBuf {
    current_config_dir().unwrap_or_else(|| PathBuf::from(".").join(CURRENT_APP_DIR_NAME))
}

pub fn current_data_dir() -> Option<PathBuf> {
    with_app_name(dirs::data_dir(), CURRENT_APP_DIR_NAME)
}

pub fn legacy_data_dir() -> Option<PathBuf> {
    with_app_name(dirs::data_dir(), LEGACY_APP_DIR_NAME)
}

pub fn writable_data_dir() -> PathBuf {
    current_data_dir().unwrap_or_else(|| PathBuf::from(".").join(CURRENT_APP_DIR_NAME))
}

pub fn current_config_file(file_name: &str) -> PathBuf {
    writable_config_dir().join(file_name)
}

pub fn current_data_file(file_name: &str) -> PathBuf {
    writable_data_dir().join(file_name)
}

pub fn legacy_config_file(file_name: &str) -> Option<PathBuf> {
    legacy_config_dir().map(|dir| dir.join(file_name))
}

pub fn legacy_data_file(file_name: &str) -> Option<PathBuf> {
    legacy_data_dir().map(|dir| dir.join(file_name))
}

pub fn existing_config_file(file_name: &str) -> Option<PathBuf> {
    let current = current_config_file(file_name);
    if current.exists() {
        return Some(current);
    }

    legacy_config_file(file_name).filter(|path| path.exists())
}

pub fn existing_data_file(file_name: &str) -> Option<PathBuf> {
    let current = current_data_file(file_name);
    if current.exists() {
        return Some(current);
    }

    legacy_data_file(file_name).filter(|path| path.exists())
}
