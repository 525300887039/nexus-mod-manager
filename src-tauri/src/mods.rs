use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri_plugin_dialog::DialogExt;

const DISABLED_DIR: &str = "mods_disabled";
const LEGACY_DISABLED_DIR: &str = "_disabled";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ModInfo {
    pub id: Option<String>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub nexus_id: Option<u64>,
    pub description: Option<String>,
    pub dependencies: Option<Vec<String>>,
    pub affects_gameplay: Option<bool>,
    pub has_dll: Option<bool>,
    pub has_pck: Option<bool>,
    pub enabled: bool,
    #[serde(rename = "instanceKey")]
    pub instance_key: String,
    #[serde(rename = "folderName")]
    pub folder_name: String,
    #[serde(rename = "isFolder")]
    pub is_folder: bool,
    pub path: String,
    pub files: Vec<String>,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ToggleModInfo {
    #[serde(rename = "isFolder")]
    pub is_folder: bool,
    #[serde(rename = "folderName")]
    pub folder_name: String,
    pub files: Option<Vec<String>>,
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct ModResult {
    pub success: bool,
    pub error: Option<String>,
    pub mods: Option<Vec<ModInfo>>,
    pub installed: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArchiveFormat {
    Zip,
    Rar,
    SevenZip,
}

fn get_mods_dir(game_path: &str) -> PathBuf {
    Path::new(game_path).join("mods")
}

fn get_disabled_dir(game_path: &str) -> PathBuf {
    let dir = Path::new(game_path).join(DISABLED_DIR);
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir
}

fn get_legacy_disabled_dir(game_path: &str) -> PathBuf {
    get_mods_dir(game_path).join(LEGACY_DISABLED_DIR)
}

fn migrate_legacy_disabled(game_path: &str) {
    let legacy = get_legacy_disabled_dir(game_path);
    let disabled = get_disabled_dir(game_path);
    if !legacy.exists() {
        return;
    }
    if let Ok(entries) = fs::read_dir(&legacy) {
        for entry in entries.flatten() {
            let dst = disabled.join(entry.file_name());
            if !dst.exists() {
                let _ = fs::rename(entry.path(), dst);
            }
        }
    }
    if legacy.exists() {
        if let Ok(entries) = fs::read_dir(&legacy) {
            if entries.count() == 0 {
                let _ = fs::remove_dir(&legacy);
            }
        }
    }
}

fn read_json_file(path: &Path) -> Option<serde_json::Value> {
    let mut content = fs::read_to_string(path).ok()?;
    // Strip BOM
    if content.starts_with('\u{feff}') {
        content = content[3..].to_string();
    }
    serde_json::from_str(&content).ok()
}

fn dir_size(path: &Path) -> u64 {
    let mut size = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                size += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                size += meta.len();
            }
        }
    }
    size
}

fn parse_optional_u64(value: Option<&serde_json::Value>) -> Option<u64> {
    value.and_then(|raw| {
        raw.as_u64().or_else(|| {
            raw.as_str()
                .and_then(|text| text.trim().parse::<u64>().ok())
        })
    })
}

fn try_parse_mod(full_path: &Path, item_name: &str, enabled: bool) -> Option<ModInfo> {
    let meta = fs::metadata(full_path).ok()?;

    if meta.is_dir() {
        // Folder mod
        let entries: Vec<String> = fs::read_dir(full_path)
            .ok()?
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .collect();

        let json_files: Vec<&String> = entries.iter().filter(|f| f.ends_with(".json")).collect();
        for jf in json_files {
            let json_path = full_path.join(jf);
            if let Some(data) = read_json_file(&json_path) {
                if data.get("id").and_then(|v| v.as_str()).is_some()
                    && data.get("name").and_then(|v| v.as_str()).is_some()
                {
                    let has_dll = entries.iter().any(|f| f.ends_with(".dll"));
                    let has_pck = entries.iter().any(|f| f.ends_with(".pck"));
                    return Some(ModInfo {
                        id: data.get("id").and_then(|v| v.as_str()).map(String::from),
                        name: data.get("name").and_then(|v| v.as_str()).map(String::from),
                        author: data
                            .get("author")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        version: data
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        nexus_id: parse_optional_u64(data.get("nexus_id")),
                        description: data
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        dependencies: data.get("dependencies").and_then(|v| {
                            v.as_array().map(|arr| {
                                arr.iter()
                                    .filter_map(|x| x.as_str().map(String::from))
                                    .collect()
                            })
                        }),
                        affects_gameplay: data.get("affects_gameplay").and_then(|v| v.as_bool()),
                        has_dll: Some(has_dll),
                        has_pck: Some(has_pck),
                        enabled,
                        instance_key: full_path.to_string_lossy().to_string(),
                        folder_name: item_name.to_string(),
                        is_folder: true,
                        path: full_path.to_string_lossy().to_string(),
                        files: entries,
                        size: dir_size(full_path),
                    });
                }
            }
        }
    } else if item_name.ends_with(".json") && !item_name.starts_with('.') {
        // Flat mod
        if let Some(data) = read_json_file(full_path) {
            if data.get("id").and_then(|v| v.as_str()).is_some()
                && data.get("name").and_then(|v| v.as_str()).is_some()
            {
                let base_name = item_name.trim_end_matches(".json");
                let parent = full_path.parent()?;
                let mut related_files = Vec::new();
                let mut total_size = meta.len();

                if let Ok(entries) = fs::read_dir(parent) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if fname.starts_with(&format!("{}.", base_name)) && fname != item_name {
                            related_files.push(fname);
                            if let Ok(m) = entry.metadata() {
                                total_size += m.len();
                            }
                        }
                    }
                }

                let has_dll = related_files.iter().any(|f| f.ends_with(".dll"));
                let has_pck = related_files.iter().any(|f| f.ends_with(".pck"));

                let mut files = vec![item_name.to_string()];
                files.extend(related_files);

                return Some(ModInfo {
                    id: data.get("id").and_then(|v| v.as_str()).map(String::from),
                    name: data.get("name").and_then(|v| v.as_str()).map(String::from),
                    author: data
                        .get("author")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    version: data
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    nexus_id: parse_optional_u64(data.get("nexus_id")),
                    description: data
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    dependencies: data.get("dependencies").and_then(|v| {
                        v.as_array().map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                    }),
                    affects_gameplay: data.get("affects_gameplay").and_then(|v| v.as_bool()),
                    has_dll: Some(has_dll),
                    has_pck: Some(has_pck),
                    enabled,
                    instance_key: full_path.to_string_lossy().to_string(),
                    folder_name: base_name.to_string(),
                    is_folder: false,
                    path: parent.to_string_lossy().to_string(),
                    files,
                    size: total_size,
                });
            }
        }
    }
    None
}

pub fn scan_mods_internal(game_path: &str) -> Vec<ModInfo> {
    let mods_dir = get_mods_dir(game_path);
    if !mods_dir.exists() {
        return vec![];
    }

    migrate_legacy_disabled(game_path);
    let mut mods = Vec::new();

    // Scan enabled mods
    if let Ok(entries) = fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == LEGACY_DISABLED_DIR {
                continue;
            }
            if let Some(m) = try_parse_mod(&entry.path(), &name, true) {
                mods.push(m);
            }
        }
    }

    // Scan disabled mods
    let disabled_dir = get_disabled_dir(game_path);
    if disabled_dir.exists() {
        if let Ok(entries) = fs::read_dir(&disabled_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(m) = try_parse_mod(&entry.path(), &name, false) {
                    mods.push(m);
                }
            }
        }
    }

    // Sort: enabled first, then alphabetical
    mods.sort_by(|a, b| {
        if a.enabled != b.enabled {
            return if a.enabled {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        let a_name = a.name.as_deref().unwrap_or("");
        let b_name = b.name.as_deref().unwrap_or("");
        a_name.to_lowercase().cmp(&b_name.to_lowercase())
    });

    mods
}

fn find_folder_mod_location(game_path: &str, folder_name: &str) -> Option<PathBuf> {
    let roots = vec![
        get_mods_dir(game_path),
        get_disabled_dir(game_path),
        get_legacy_disabled_dir(game_path),
    ];
    for root in roots {
        let candidate = root.join(folder_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn find_flat_mod_base_dir(game_path: &str, files: &[String]) -> Option<PathBuf> {
    let roots = vec![
        get_mods_dir(game_path),
        get_disabled_dir(game_path),
        get_legacy_disabled_dir(game_path),
    ];
    for root in roots {
        if files.iter().any(|f| root.join(f).exists()) {
            return Some(root);
        }
    }
    None
}

#[tauri::command]
pub fn mods_scan(state: tauri::State<'_, AppState>) -> Vec<ModInfo> {
    let gp = state.game_path.lock().unwrap();
    match &*gp {
        Some(p) => scan_mods_internal(p),
        None => vec![],
    }
}

#[tauri::command]
pub fn mods_toggle(state: tauri::State<'_, AppState>, mod_info: ToggleModInfo) -> ModResult {
    let gp = state.game_path.lock().unwrap();
    let game_path = match &*gp {
        Some(p) => p.clone(),
        None => {
            return ModResult {
                success: false,
                error: Some("Game path not set".into()),
                mods: None,
                installed: None,
            }
        }
    };
    drop(gp);

    let mods_dir = get_mods_dir(&game_path);
    let disabled_dir = get_disabled_dir(&game_path);

    if mod_info.is_folder {
        if let Some(src) = find_folder_mod_location(&game_path, &mod_info.folder_name) {
            let src_parent = src.parent().unwrap_or(Path::new(""));
            let dst = if src_parent == mods_dir.as_path() {
                disabled_dir.join(&mod_info.folder_name)
            } else {
                mods_dir.join(&mod_info.folder_name)
            };
            if src != dst {
                if let Err(e) = fs::rename(&src, &dst) {
                    return ModResult {
                        success: false,
                        error: Some(format!("移动失败: {}", e)),
                        mods: None,
                        installed: None,
                    };
                }
            }
        } else {
            return ModResult {
                success: false,
                error: Some(format!("找不到 MOD 文件夹: {}", mod_info.folder_name)),
                mods: None,
                installed: None,
            };
        }
    } else {
        let files = mod_info.files.unwrap_or_default();
        if let Some(src_dir) = find_flat_mod_base_dir(&game_path, &files) {
            let dst_dir = if src_dir == mods_dir {
                &disabled_dir
            } else {
                &mods_dir
            };
            for file in &files {
                let src = src_dir.join(file);
                let dst = dst_dir.join(file);
                if src.exists() {
                    let _ = fs::rename(&src, &dst);
                }
            }
        } else {
            return ModResult {
                success: false,
                error: Some("找不到 MOD 文件".into()),
                mods: None,
                installed: None,
            };
        }
    }

    ModResult {
        success: true,
        error: None,
        mods: Some(scan_mods_internal(&game_path)),
        installed: None,
    }
}

#[tauri::command]
pub fn mods_uninstall(state: tauri::State<'_, AppState>, mod_info: ToggleModInfo) -> ModResult {
    let gp = state.game_path.lock().unwrap();
    let game_path = match &*gp {
        Some(p) => p.clone(),
        None => {
            return ModResult {
                success: false,
                error: Some("Game path not set".into()),
                mods: None,
                installed: None,
            }
        }
    };
    drop(gp);

    if mod_info.is_folder {
        if let Some(mod_path) = find_folder_mod_location(&game_path, &mod_info.folder_name) {
            if let Err(e) = fs::remove_dir_all(&mod_path) {
                return ModResult {
                    success: false,
                    error: Some(format!("删除失败: {}", e)),
                    mods: None,
                    installed: None,
                };
            }
        }
    } else {
        let files = mod_info.files.unwrap_or_default();
        if let Some(base_dir) = find_flat_mod_base_dir(&game_path, &files) {
            for file in &files {
                let fp = base_dir.join(file);
                if fp.exists() {
                    let _ = fs::remove_file(&fp);
                }
            }
        }
    }

    ModResult {
        success: true,
        error: None,
        mods: Some(scan_mods_internal(&game_path)),
        installed: None,
    }
}

fn detect_archive_format_from_extension(path: &Path) -> Option<ArchiveFormat> {
    let ext = path
        .extension()
        .map(|value| value.to_string_lossy().to_lowercase())?;
    match ext.as_str() {
        "zip" => Some(ArchiveFormat::Zip),
        "rar" => Some(ArchiveFormat::Rar),
        "7z" => Some(ArchiveFormat::SevenZip),
        _ => None,
    }
}

fn detect_archive_format(path: &Path) -> Option<ArchiveFormat> {
    detect_archive_format_from_extension(path)
}

pub(crate) fn is_supported_archive_path(path: &Path) -> bool {
    detect_archive_format(path).is_some()
}

fn archive_format_label(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::Zip => "ZIP",
        ArchiveFormat::Rar => "RAR",
        ArchiveFormat::SevenZip => "7Z",
    }
}

fn unsupported_archive_message(path: &Path) -> String {
    let ext = path
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未知".to_string());
    format!(
        "不支持的格式: .{}\n\n目前支持 .zip / .rar / .7z 格式的压缩包。",
        ext
    )
}

fn archive_base_name(path: &Path) -> String {
    path.file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown_mod".to_string())
}

fn unique_work_dir(prefix: &str, base_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(base_dir)
        .map_err(|e| format!("无法创建临时目录 {}: {}", base_dir.display(), e))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();

    for attempt in 0..32 {
        let candidate = base_dir.join(format!("{prefix}-{pid}-{now}-{attempt}"));
        if !candidate.exists() {
            fs::create_dir_all(&candidate)
                .map_err(|e| format!("无法创建临时目录 {}: {}", candidate.display(), e))?;
            return Ok(candidate);
        }
    }

    Err("无法创建唯一临时目录，请稍后重试".to_string())
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn sanitize_archive_path(raw: &str) -> Option<PathBuf> {
    let normalized = raw.replace('\\', "/");
    let mut cleaned = PathBuf::new();

    for component in Path::new(&normalized).components() {
        match component {
            std::path::Component::Normal(part) => cleaned.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::ParentDir => return None,
        }
    }

    if cleaned.as_os_str().is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| format!("无法删除目录 {}: {}", path.display(), e))
    } else {
        fs::remove_file(path).map_err(|e| format!("无法删除文件 {}: {}", path.display(), e))
    }
}

fn copy_file_to_path(src: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("无法创建目录 {}: {}", parent.display(), e))?;
    }
    fs::copy(src, dest).map_err(|e| {
        format!(
            "复制文件失败 {} -> {}: {}",
            src.display(),
            dest.display(),
            e
        )
    })?;
    Ok(())
}

fn copy_children_to_dir(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("无法创建目录 {}: {}", dest.display(), e))?;
    for entry in fs::read_dir(src).map_err(|e| format!("无法读取目录 {}: {}", src.display(), e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            copy_file_to_path(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

fn copy_root_files_to_dir(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("无法创建目录 {}: {}", dest.display(), e))?;
    for entry in fs::read_dir(src).map_err(|e| format!("无法读取目录 {}: {}", src.display(), e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        if src_path.is_file() {
            let dest_path = dest.join(entry.file_name());
            copy_file_to_path(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

fn dir_has_entries(path: &Path) -> Result<bool, String> {
    Ok(fs::read_dir(path)
        .map_err(|e| format!("无法读取目录 {}: {}", path.display(), e))?
        .next()
        .is_some())
}

fn is_mod_manifest_file(path: &Path) -> bool {
    read_json_file(path)
        .map(|value| value.get("id").is_some() && value.get("name").is_some())
        .unwrap_or(false)
}

fn collect_manifest_roots(root: &Path) -> Result<(Vec<PathBuf>, bool), String> {
    fn visit(
        current: &Path,
        root: &Path,
        mod_roots: &mut HashSet<PathBuf>,
        flat_mod: &mut bool,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current)
            .map_err(|e| format!("无法读取目录 {}: {}", current.display(), e))?
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                visit(&path, root, mod_roots, flat_mod)?;
                continue;
            }

            let is_json = path
                .extension()
                .map(|value| value.to_string_lossy().eq_ignore_ascii_case("json"))
                .unwrap_or(false);
            if !is_json || !is_mod_manifest_file(&path) {
                continue;
            }

            let parent = path.parent().unwrap_or(root);
            if parent == root {
                *flat_mod = true;
            } else {
                let relative = parent
                    .strip_prefix(root)
                    .map_err(|e| format!("计算目录相对路径失败: {}", e))?;
                mod_roots.insert(relative.to_path_buf());
            }
        }
        Ok(())
    }

    let mut mod_roots = HashSet::new();
    let mut flat_mod = false;
    visit(root, root, &mut mod_roots, &mut flat_mod)?;

    let mut mod_roots = mod_roots.into_iter().collect::<Vec<_>>();
    mod_roots.sort();
    Ok((mod_roots, flat_mod))
}

fn prepare_archive_install_tree(
    extracted_root: &Path,
    prepared_root: &Path,
    archive_path: &Path,
) -> Result<(), String> {
    let (mod_roots, flat_mod) = collect_manifest_roots(extracted_root)?;

    if !mod_roots.is_empty() || flat_mod {
        for mod_root in &mod_roots {
            let src_dir = extracted_root.join(mod_root);
            let folder_name = mod_root
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown_mod".to_string());
            let dest_dir = prepared_root.join(folder_name);
            copy_dir_recursive(&src_dir, &dest_dir)?;
        }

        if flat_mod && mod_roots.is_empty() {
            copy_root_files_to_dir(extracted_root, prepared_root)?;
        }
        return Ok(());
    }

    let mut top_dirs = HashSet::new();
    let mut has_root_file = false;
    for entry in fs::read_dir(extracted_root)
        .map_err(|e| format!("无法读取目录 {}: {}", extracted_root.display(), e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            top_dirs.insert(entry.file_name().to_string_lossy().to_string());
        } else {
            has_root_file = true;
        }
    }

    if !has_root_file && top_dirs.len() == 1 {
        copy_children_to_dir(extracted_root, prepared_root)?;
    } else {
        let dest_dir = prepared_root.join(archive_base_name(archive_path));
        copy_children_to_dir(extracted_root, &dest_dir)?;
    }

    Ok(())
}

fn promote_prepared_tree(prepared_root: &Path, mods_dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(prepared_root)
        .map_err(|e| format!("无法读取临时安装目录 {}: {}", prepared_root.display(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    if entries.is_empty() {
        return Err("压缩包中没有可安装的内容。".to_string());
    }

    for entry in entries {
        let src_path = entry.path();
        let dest_path = mods_dir.join(entry.file_name());
        remove_path_if_exists(&dest_path)?;
        fs::rename(&src_path, &dest_path)
            .map_err(|e| format!("无法安装到 {}: {}", dest_path.display(), e))?;
    }

    Ok(())
}

fn extract_zip_to_dir(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| {
        format!(
            "无法读取 ZIP 压缩包: {}\n\n该文件可能已损坏或不是有效的 ZIP 格式。",
            e
        )
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        format!(
            "无法读取 ZIP 压缩包: {}\n\n该文件可能已损坏或不是有效的 ZIP 格式。",
            e
        )
    })?;

    if archive.len() == 0 {
        return Err(format!(
            "压缩包为空: {}",
            zip_path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let raw_name = entry.name().replace('\\', "/");
        let relative = sanitize_archive_path(&raw_name)
            .ok_or_else(|| format!("ZIP 压缩包包含不安全路径: {}", raw_name))?;
        let out_path = dest_dir.join(relative);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("无法创建目录 {}: {}", out_path.display(), e))?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("无法创建目录 {}: {}", parent.display(), e))?;
        }
        let mut outfile = fs::File::create(&out_path)
            .map_err(|e| format!("无法创建文件 {}: {}", out_path.display(), e))?;
        std::io::copy(&mut entry, &mut outfile)
            .map_err(|e| format!("无法写入文件 {}: {}", out_path.display(), e))?;
    }

    Ok(())
}

fn extract_rar_to_dir(rar_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let mut archive = unrar::Archive::new(rar_path)
        .as_first_part()
        .open_for_processing()
        .map_err(|e| format!("无法读取 RAR 压缩包: {}", e))?;
    let mut extracted_any = false;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| format!("读取 RAR 条目失败: {}", e))?
    {
        let raw_name = header.entry().filename.to_string_lossy().to_string();
        let relative = sanitize_archive_path(&raw_name)
            .ok_or_else(|| format!("RAR 压缩包包含不安全路径: {}", raw_name))?;
        let out_path = dest_dir.join(relative);

        if header.entry().is_directory() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("无法创建目录 {}: {}", out_path.display(), e))?;
            archive = header
                .skip()
                .map_err(|e| format!("跳过 RAR 目录失败: {}", e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("无法创建目录 {}: {}", parent.display(), e))?;
            }
            archive = header
                .extract_to(&out_path)
                .map_err(|e| format!("解压 RAR 文件失败 {}: {}", out_path.display(), e))?;
        }

        extracted_any = true;
    }

    if !extracted_any {
        return Err(format!(
            "压缩包为空: {}",
            rar_path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    Ok(())
}

fn extract_7z_to_dir(sevenz_path: &Path, dest_dir: &Path) -> Result<(), String> {
    let mut extracted_any = false;
    sevenz_rust::decompress_file_with_extract_fn(sevenz_path, dest_dir, |entry, reader, _| {
        let raw_name = entry.name().replace('\\', "/");
        let relative = sanitize_archive_path(&raw_name).ok_or_else(|| {
            sevenz_rust::Error::other(format!("7Z 压缩包包含不安全路径: {}", raw_name))
        })?;
        let out_path = dest_dir.join(relative);

        if entry.is_directory() {
            fs::create_dir_all(&out_path).map_err(sevenz_rust::Error::from)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(sevenz_rust::Error::from)?;
            }
            let mut outfile = fs::File::create(&out_path).map_err(sevenz_rust::Error::from)?;
            std::io::copy(reader, &mut outfile).map_err(sevenz_rust::Error::from)?;
        }

        extracted_any = true;
        Ok(true)
    })
    .map_err(|e| format!("无法读取 7Z 压缩包: {}", e))?;

    if !extracted_any {
        return Err(format!(
            "压缩包为空: {}",
            sevenz_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ));
    }

    Ok(())
}

fn extract_archive_to_dir(
    archive_path: &Path,
    dest_dir: &Path,
    format: ArchiveFormat,
) -> Result<(), String> {
    match format {
        ArchiveFormat::Zip => extract_zip_to_dir(archive_path, dest_dir),
        ArchiveFormat::Rar => extract_rar_to_dir(archive_path, dest_dir),
        ArchiveFormat::SevenZip => extract_7z_to_dir(archive_path, dest_dir),
    }
}

pub(crate) fn smart_extract_archive(archive_path: &str, mods_dir: &Path) -> Result<(), String> {
    let archive_path = Path::new(archive_path);
    let Some(format) = detect_archive_format(archive_path) else {
        return Err(unsupported_archive_message(archive_path));
    };

    fs::create_dir_all(mods_dir)
        .map_err(|e| format!("无法创建 Mod 安装目录 {}: {}", mods_dir.display(), e))?;

    let extraction_temp =
        TempDirGuard::new(unique_work_dir("sts2-mod-extract", &std::env::temp_dir())?);
    extract_archive_to_dir(archive_path, extraction_temp.path(), format)
        .map_err(|error| format!("{} 解压失败: {}", archive_format_label(format), error))?;

    if !dir_has_entries(extraction_temp.path())? {
        return Err("压缩包中没有可提取的内容。".to_string());
    }

    let prepared_root = TempDirGuard::new(unique_work_dir("sts2-mod-install", mods_dir)?);
    prepare_archive_install_tree(extraction_temp.path(), prepared_root.path(), archive_path)?;

    if !dir_has_entries(prepared_root.path())? {
        return Err("压缩包中没有可安装的内容。".to_string());
    }

    promote_prepared_tree(prepared_root.path(), mods_dir)
}

fn install_folder(folder_path: &str, mods_dir: &Path) -> Result<(), String> {
    let src = Path::new(folder_path);
    if !src.is_dir() {
        return Err(format!("不是有效的文件夹: {}", folder_path));
    }
    let folder_name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown_mod".to_string());
    let dest = mods_dir.join(&folder_name);
    copy_dir_recursive(src, &dest)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(dest);
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn mods_install(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ModResult, String> {
    let gp = state.game_path.lock().unwrap().clone();
    let game_path = match gp {
        Some(p) => p,
        None => {
            return Ok(ModResult {
                success: false,
                error: Some("Game path not set".into()),
                mods: None,
                installed: None,
            })
        }
    };

    let mods_dir = get_mods_dir(&game_path);
    let dialog = app.dialog();
    let files = dialog
        .file()
        .set_title("Select MOD Archive")
        .add_filter("Archives", &["zip", "rar", "7z"])
        .blocking_pick_files();

    let file_paths = match files {
        Some(paths) => paths,
        None => {
            return Ok(ModResult {
                success: false,
                error: Some("Cancelled".into()),
                mods: None,
                installed: None,
            })
        }
    };

    let mut installed = Vec::new();
    for fp in &file_paths {
        let path_str = fp.to_string();
        let p = Path::new(&path_str);
        let result = if p.is_dir() {
            install_folder(&path_str, &mods_dir)
        } else {
            smart_extract_archive(&path_str, &mods_dir)
        };
        if let Err(e) = result {
            return Ok(ModResult {
                success: false,
                error: Some(e),
                mods: None,
                installed: None,
            });
        }
        installed.push(
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
        );
    }

    Ok(ModResult {
        success: true,
        error: None,
        mods: Some(scan_mods_internal(&game_path)),
        installed: Some(installed),
    })
}

#[tauri::command]
pub fn mods_install_drop(state: tauri::State<'_, AppState>, file_paths: Vec<String>) -> ModResult {
    let gp = state.game_path.lock().unwrap().clone();
    let game_path = match gp {
        Some(p) => p,
        None => {
            return ModResult {
                success: false,
                error: Some("Game path not set".into()),
                mods: None,
                installed: None,
            }
        }
    };

    let mods_dir = get_mods_dir(&game_path);
    let mut installed = Vec::new();

    for fp in &file_paths {
        let p = Path::new(fp.as_str());
        let result = if p.is_dir() {
            install_folder(fp, &mods_dir)
        } else {
            smart_extract_archive(fp, &mods_dir)
        };
        if let Err(e) = result {
            return ModResult {
                success: false,
                error: Some(e),
                mods: None,
                installed: None,
            };
        }
        installed.push(
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
        );
    }

    ModResult {
        success: true,
        error: None,
        mods: Some(scan_mods_internal(&game_path)),
        installed: Some(installed),
    }
}

fn create_zip_from_dir(source: &Path, zip_path: &str) -> Result<(), String> {
    let file = fs::File::create(zip_path).map_err(|e| e.to_string())?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    fn add_dir_to_zip(
        zip_writer: &mut zip::ZipWriter<fs::File>,
        base: &Path,
        current: &Path,
        options: zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        if let Ok(entries) = fs::read_dir(current) {
            for entry in entries.flatten() {
                let path = entry.path();
                let rel = path
                    .strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .to_string();
                if path.is_dir() {
                    let _ = zip_writer.add_directory(&format!("{}/", rel), options);
                    add_dir_to_zip(zip_writer, base, &path, options)?;
                } else {
                    zip_writer
                        .start_file(&rel, options)
                        .map_err(|e| e.to_string())?;
                    let mut f = fs::File::open(&path).map_err(|e| e.to_string())?;
                    let mut buf = Vec::new();
                    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                    std::io::Write::write_all(zip_writer, &buf).map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }

    add_dir_to_zip(&mut zip_writer, source, source, options)?;
    zip_writer.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn mods_backup(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ModResult, String> {
    let gp = state.game_path.lock().unwrap().clone();
    let game_path = match gp {
        Some(p) => p,
        None => {
            return Ok(ModResult {
                success: false,
                error: Some("Game path not set".into()),
                mods: None,
                installed: None,
            })
        }
    };

    let mods_dir = get_mods_dir(&game_path);
    let dialog = app.dialog();
    let default_name = format!("sts2_mods_backup_{}.zip", chrono_timestamp());

    let save_path = dialog
        .file()
        .set_title("Save MOD Backup")
        .set_file_name(&default_name)
        .add_filter("ZIP Archive", &["zip"])
        .blocking_save_file();

    match save_path {
        Some(path) => {
            let path_str = path.to_string();
            if let Err(e) = create_zip_from_dir(&mods_dir, &path_str) {
                return Ok(ModResult {
                    success: false,
                    error: Some(e),
                    mods: None,
                    installed: None,
                });
            }
            Ok(ModResult {
                success: true,
                error: None,
                mods: None,
                installed: None,
            })
        }
        None => Ok(ModResult {
            success: false,
            error: None,
            mods: None,
            installed: None,
        }),
    }
}

#[tauri::command]
pub async fn mods_restore(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ModResult, String> {
    let gp = state.game_path.lock().unwrap().clone();
    let game_path = match gp {
        Some(p) => p,
        None => {
            return Ok(ModResult {
                success: false,
                error: Some("Game path not set".into()),
                mods: None,
                installed: None,
            })
        }
    };

    let mods_dir = get_mods_dir(&game_path);
    let dialog = app.dialog();
    let file = dialog
        .file()
        .set_title("Select MOD Backup")
        .add_filter("ZIP Archive", &["zip"])
        .blocking_pick_file();

    match file {
        Some(path) => {
            let path_str = path.to_string();
            if let Err(e) = smart_extract_archive(&path_str, &mods_dir) {
                return Ok(ModResult {
                    success: false,
                    error: Some(e),
                    mods: None,
                    installed: None,
                });
            }
            Ok(ModResult {
                success: true,
                error: None,
                mods: Some(scan_mods_internal(&game_path)),
                installed: None,
            })
        }
        None => Ok(ModResult {
            success: false,
            error: None,
            mods: None,
            installed: None,
        }),
    }
}

fn chrono_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    now.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn make_temp_dir(label: &str) -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "sts2-mod-manager-test-{label}-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            id
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn sanitize_archive_path_rejects_unsafe_components() {
        assert_eq!(
            sanitize_archive_path("mods\\Example\\file.dll"),
            Some(PathBuf::from("mods").join("Example").join("file.dll"))
        );
        assert_eq!(sanitize_archive_path("../evil.dll"), None);
        assert_eq!(sanitize_archive_path("/absolute/path.dll"), None);
        assert_eq!(sanitize_archive_path("C:/evil.dll"), None);
    }

    #[test]
    fn prepare_archive_install_tree_prefers_manifest_roots() {
        let extracted = make_temp_dir("manifest-src");
        let prepared = make_temp_dir("manifest-dest");

        write_file(
            &extracted
                .join("nested")
                .join("ExampleMod")
                .join("ExampleMod.json"),
            r#"{"id":"example","name":"Example Mod"}"#,
        );
        write_file(
            &extracted
                .join("nested")
                .join("ExampleMod")
                .join("ExampleMod.dll"),
            "binary",
        );

        prepare_archive_install_tree(&extracted, &prepared, Path::new("ExampleMod.rar")).unwrap();

        assert!(prepared.join("ExampleMod").join("ExampleMod.json").exists());
        assert!(prepared.join("ExampleMod").join("ExampleMod.dll").exists());

        let _ = fs::remove_dir_all(&extracted);
        let _ = fs::remove_dir_all(&prepared);
    }

    #[test]
    fn prepare_archive_install_tree_keeps_single_top_level_folder() {
        let extracted = make_temp_dir("single-root-src");
        let prepared = make_temp_dir("single-root-dest");

        write_file(
            &extracted.join("PackRoot").join("data").join("readme.txt"),
            "hello",
        );

        prepare_archive_install_tree(&extracted, &prepared, Path::new("pack.7z")).unwrap();

        assert!(prepared
            .join("PackRoot")
            .join("data")
            .join("readme.txt")
            .exists());

        let _ = fs::remove_dir_all(&extracted);
        let _ = fs::remove_dir_all(&prepared);
    }

    #[test]
    fn prepare_archive_install_tree_wraps_mixed_root_entries() {
        let extracted = make_temp_dir("mixed-root-src");
        let prepared = make_temp_dir("mixed-root-dest");

        write_file(&extracted.join("Example.json"), r#"{"foo":"bar"}"#);
        write_file(&extracted.join("Example.dll"), "binary");

        prepare_archive_install_tree(&extracted, &prepared, Path::new("MixedArchive.zip")).unwrap();

        assert!(prepared.join("MixedArchive").join("Example.json").exists());
        assert!(prepared.join("MixedArchive").join("Example.dll").exists());

        let _ = fs::remove_dir_all(&extracted);
        let _ = fs::remove_dir_all(&prepared);
    }
}
