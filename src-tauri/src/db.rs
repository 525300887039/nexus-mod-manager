use crate::{config, mods, AppState};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SQLITE_BATCH_SIZE: usize = 900;

fn app_data_dir() -> Result<PathBuf, String> {
    let base = dirs::data_dir().ok_or_else(|| "无法解析应用数据目录".to_string())?;
    let dir = base.join("STS2ModManager");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("无法创建应用数据目录 {}: {}", dir.display(), e))?;
    Ok(dir)
}

pub(crate) fn cache_db_path() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("cache.db"))
}

fn legacy_translations_path() -> Option<PathBuf> {
    dirs::config_dir()
        .or_else(dirs::data_dir)
        .map(|base| base.join("STS2ModManager").join("translations.json"))
}

fn legacy_backup_path(path: &Path) -> PathBuf {
    let default = PathBuf::from(format!("{}.bak", path.display()));
    if !default.exists() {
        return default;
    }

    PathBuf::from(format!("{}.{}.bak", path.display(), now_millis()))
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn upsert_translation_row(
    db: &Connection,
    source_text: &str,
    translated: &str,
    provider: &str,
) -> Result<(), String> {
    let source_text = source_text.trim();
    let translated = translated.trim();
    let provider = provider.trim();

    if source_text.is_empty() {
        return Err("source_text 不能为空".to_string());
    }
    if translated.is_empty() {
        return Err("translated 不能为空".to_string());
    }
    if provider.is_empty() {
        return Err("provider 不能为空".to_string());
    }

    let now = now_millis();
    db.execute(
        "INSERT INTO translations (source_text, translated, provider, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(source_text) DO UPDATE SET
           translated = excluded.translated,
           provider = excluded.provider,
           updated_at = excluded.updated_at",
        params![source_text, translated, provider, now],
    )
    .map_err(|e| format!("写入翻译缓存失败: {}", e))?;

    Ok(())
}

fn build_mod_source_lookup(game_path: &str) -> HashMap<String, (Option<String>, Option<String>)> {
    mods::scan_mods_internal(game_path)
        .into_iter()
        .filter_map(|mod_info| {
            mod_info
                .id
                .map(|id| (id, (mod_info.name, mod_info.description)))
        })
        .collect()
}

pub(crate) fn translation_cache_get_db(
    db: &Connection,
    source_text: &str,
) -> Result<Option<String>, String> {
    let source_text = source_text.trim();
    if source_text.is_empty() {
        return Ok(None);
    }

    db.query_row(
        "SELECT translated FROM translations WHERE source_text = ?1",
        [source_text],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| format!("读取翻译缓存失败: {}", e))
}

pub(crate) fn translation_cache_set_db(
    db: &Connection,
    source_text: &str,
    translated: &str,
    provider: &str,
) -> Result<(), String> {
    upsert_translation_row(db, source_text, translated, provider)
}

pub(crate) fn translation_cache_batch_get_db(
    db: &Connection,
    texts: Vec<String>,
) -> Result<HashMap<String, String>, String> {
    let mut seen = HashSet::new();
    let mut unique_texts = Vec::new();

    for text in texts {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let normalized = trimmed.to_string();
        if seen.insert(normalized.clone()) {
            unique_texts.push(normalized);
        }
    }

    if unique_texts.is_empty() {
        return Ok(HashMap::new());
    }

    let mut results = HashMap::new();

    for chunk in unique_texts.chunks(SQLITE_BATCH_SIZE) {
        let placeholders = std::iter::repeat("?")
            .take(chunk.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT source_text, translated FROM translations WHERE source_text IN ({})",
            placeholders
        );
        let mut stmt = db
            .prepare(&sql)
            .map_err(|e| format!("准备批量查询失败: {}", e))?;
        let rows = stmt
            .query_map(params_from_iter(chunk.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("执行批量查询失败: {}", e))?;

        for row in rows {
            let (source_text, translated) =
                row.map_err(|e| format!("读取批量查询结果失败: {}", e))?;
            results.insert(source_text, translated);
        }
    }

    Ok(results)
}

pub(crate) fn translation_cache_count_db(db: &Connection) -> Result<u64, String> {
    let count: i64 = db
        .query_row("SELECT COUNT(*) FROM translations", [], |row| row.get(0))
        .map_err(|e| format!("统计翻译缓存失败: {}", e))?;

    u64::try_from(count).map_err(|_| format!("缓存条目数量异常: {}", count))
}

pub(crate) fn translation_cache_clear_db(db: &Connection) -> Result<(), String> {
    db.execute("DELETE FROM translations", [])
        .map_err(|e| format!("清空翻译缓存失败: {}", e))?;
    Ok(())
}

pub fn init_db(_app_handle: &tauri::AppHandle) -> Result<Connection, String> {
    let db_path = cache_db_path()?;
    let db = Connection::open(&db_path)
        .map_err(|e| format!("无法打开数据库 {}: {}", db_path.display(), e))?;

    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS translations (
           source_text TEXT PRIMARY KEY,
           translated TEXT NOT NULL,
           provider TEXT NOT NULL,
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS nexus_mod_cache (
           mod_id INTEGER PRIMARY KEY,
           data_json TEXT NOT NULL,
           fetched_at INTEGER NOT NULL
         );",
    )
    .map_err(|e| format!("初始化数据库表失败: {}", e))?;

    Ok(db)
}

pub fn translations_migrate_json_to_db(db: &mut Connection) -> Result<(), String> {
    let Some(legacy_path) = legacy_translations_path() else {
        return Ok(());
    };

    if !legacy_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&legacy_path)
        .map_err(|e| format!("读取旧 translations.json 失败: {}", e))?;
    let parsed: Value = serde_json::from_str(&content)
        .map_err(|e| format!("解析旧 translations.json 失败: {}", e))?;
    let Some(entries) = parsed.as_object() else {
        return Err("旧 translations.json 根节点必须是对象".to_string());
    };

    let requires_mod_lookup = entries.values().any(|value| value.is_object());
    let mod_lookup = if requires_mod_lookup {
        let Some(game_path) = config::load_or_detect_game_path() else {
            return Ok(());
        };
        build_mod_source_lookup(&game_path)
    } else {
        HashMap::new()
    };

    let tx = db
        .transaction()
        .map_err(|e| format!("开启翻译迁移事务失败: {}", e))?;

    for (key, value) in entries {
        if let Some(translated) = value.as_str() {
            if !key.trim().is_empty() && !translated.trim().is_empty() {
                upsert_translation_row(&tx, key, translated, "legacy")?;
            }
            continue;
        }

        let Some(legacy_entry) = value.as_object() else {
            continue;
        };
        let Some((source_name, source_desc)) = mod_lookup.get(key) else {
            eprintln!(
                "Skipping unresolved legacy translation entry for mod id {}",
                key
            );
            continue;
        };

        if let Some(translated_name) = legacy_entry.get("name").and_then(|value| value.as_str()) {
            if let Some(source_text) = source_name.as_deref() {
                if !source_text.trim().is_empty() && !translated_name.trim().is_empty() {
                    upsert_translation_row(&tx, source_text, translated_name, "legacy")?;
                }
            }
        }

        if let Some(translated_desc) = legacy_entry.get("desc").and_then(|value| value.as_str()) {
            if let Some(source_text) = source_desc.as_deref() {
                if !source_text.trim().is_empty() && !translated_desc.trim().is_empty() {
                    upsert_translation_row(&tx, source_text, translated_desc, "legacy")?;
                }
            }
        }
    }

    tx.commit()
        .map_err(|e| format!("提交翻译迁移事务失败: {}", e))?;

    let backup_path = legacy_backup_path(&legacy_path);
    fs::rename(&legacy_path, &backup_path).map_err(|e| {
        format!(
            "迁移完成后重命名旧 translations.json 失败 ({} -> {}): {}",
            legacy_path.display(),
            backup_path.display(),
            e
        )
    })?;

    Ok(())
}

fn lock_db<'a>(
    state: &'a tauri::State<'_, AppState>,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))
}

#[tauri::command]
pub fn translation_cache_get(
    state: tauri::State<'_, AppState>,
    source_text: String,
) -> Result<Option<String>, String> {
    let db = lock_db(&state)?;
    translation_cache_get_db(&db, &source_text)
}

#[tauri::command]
pub fn translation_cache_set(
    state: tauri::State<'_, AppState>,
    source_text: String,
    translated: String,
    provider: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    translation_cache_set_db(&db, &source_text, &translated, &provider)
}

#[tauri::command]
pub fn translation_cache_batch_get(
    state: tauri::State<'_, AppState>,
    texts: Vec<String>,
) -> Result<HashMap<String, String>, String> {
    let db = lock_db(&state)?;
    translation_cache_batch_get_db(&db, texts)
}

#[tauri::command]
pub fn translation_cache_count(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let db = lock_db(&state)?;
    translation_cache_count_db(&db)
}

#[tauri::command]
pub fn translation_cache_clear(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = lock_db(&state)?;
    translation_cache_clear_db(&db)
}
