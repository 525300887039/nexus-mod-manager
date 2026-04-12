use crate::{config, mods, nexus_api::NexusModInfo, AppState};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SQLITE_BATCH_SIZE: usize = 900;

#[derive(Clone, Debug)]
pub(crate) struct SavedTranslationRow {
    pub name_translated: Option<String>,
    pub desc_translated: Option<String>,
    pub source_name: Option<String>,
    pub source_desc: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct NexusSavedTranslationRow {
    pub name_translated: Option<String>,
    pub desc_translated: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
#[serde(default, rename_all = "snake_case")]
struct NexusModCacheRow {
    #[serde(alias = "modId")]
    mod_id: u64,
    name: String,
    summary: String,
    description: Option<String>,
    #[serde(alias = "pictureUrl")]
    picture_url: Option<String>,
    #[serde(alias = "modDownloads")]
    mod_downloads: u64,
    #[serde(alias = "modUniqueDownloads")]
    mod_unique_downloads: u64,
    #[serde(alias = "endorsementCount")]
    endorsement_count: u64,
    version: String,
    author: String,
    #[serde(alias = "uploadedBy")]
    uploaded_by: String,
    #[serde(alias = "categoryId")]
    category_id: u64,
    #[serde(alias = "createdTimestamp")]
    created_timestamp: u64,
    #[serde(alias = "updatedTimestamp")]
    updated_timestamp: u64,
    available: bool,
    status: String,
}

impl From<&NexusModInfo> for NexusModCacheRow {
    fn from(value: &NexusModInfo) -> Self {
        Self {
            mod_id: value.mod_id,
            name: value.name.clone(),
            summary: value.summary.clone(),
            description: value.description.clone(),
            picture_url: value.picture_url.clone(),
            mod_downloads: value.mod_downloads,
            mod_unique_downloads: value.mod_unique_downloads,
            endorsement_count: value.endorsement_count,
            version: value.version.clone(),
            author: value.author.clone(),
            uploaded_by: value.uploaded_by.clone(),
            category_id: value.category_id,
            created_timestamp: value.created_timestamp,
            updated_timestamp: value.updated_timestamp,
            available: value.available,
            status: value.status.clone(),
        }
    }
}

impl From<NexusModCacheRow> for NexusModInfo {
    fn from(value: NexusModCacheRow) -> Self {
        Self {
            mod_id: value.mod_id,
            name: value.name,
            summary: value.summary,
            description: value.description,
            picture_url: value.picture_url,
            mod_downloads: value.mod_downloads,
            mod_unique_downloads: value.mod_unique_downloads,
            endorsement_count: value.endorsement_count,
            version: value.version,
            author: value.author,
            uploaded_by: value.uploaded_by,
            category_id: value.category_id,
            created_timestamp: value.created_timestamp,
            updated_timestamp: value.updated_timestamp,
            available: value.available,
            status: value.status,
        }
    }
}

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

fn normalize_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(String::from)
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

pub(crate) fn saved_translation_upsert_db(
    db: &Connection,
    mod_id: &str,
    name_translated: Option<&str>,
    desc_translated: Option<&str>,
    source_name: Option<&str>,
    source_desc: Option<&str>,
) -> Result<(), String> {
    let mod_id = mod_id.trim();
    if mod_id.is_empty() {
        return Err("mod_id 不能为空".to_string());
    }

    let name_translated = normalize_optional_text(name_translated);
    let desc_translated = normalize_optional_text(desc_translated);
    let source_name = normalize_optional_text(source_name);
    let source_desc = normalize_optional_text(source_desc);

    if name_translated.is_none() && desc_translated.is_none() {
        return Ok(());
    }

    let now = now_millis();
    db.execute(
        "INSERT INTO saved_translations (
           mod_id,
           name_translated,
           desc_translated,
           source_name,
           source_desc,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(mod_id) DO UPDATE SET
           name_translated = COALESCE(excluded.name_translated, saved_translations.name_translated),
           desc_translated = COALESCE(excluded.desc_translated, saved_translations.desc_translated),
           source_name = COALESCE(excluded.source_name, saved_translations.source_name),
           source_desc = COALESCE(excluded.source_desc, saved_translations.source_desc),
           updated_at = excluded.updated_at",
        params![
            mod_id,
            name_translated.as_deref(),
            desc_translated.as_deref(),
            source_name.as_deref(),
            source_desc.as_deref(),
            now
        ],
    )
    .map_err(|e| format!("写入已保存翻译失败: {}", e))?;

    Ok(())
}

pub(crate) fn saved_translations_load_db(
    db: &Connection,
) -> Result<HashMap<String, SavedTranslationRow>, String> {
    let mut stmt = db
        .prepare(
            "SELECT mod_id, name_translated, desc_translated, source_name, source_desc
             FROM saved_translations",
        )
        .map_err(|e| format!("准备已保存翻译查询失败: {}", e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                SavedTranslationRow {
                    name_translated: row.get::<_, Option<String>>(1)?,
                    desc_translated: row.get::<_, Option<String>>(2)?,
                    source_name: row.get::<_, Option<String>>(3)?,
                    source_desc: row.get::<_, Option<String>>(4)?,
                },
            ))
        })
        .map_err(|e| format!("读取已保存翻译失败: {}", e))?;

    let mut result = HashMap::new();
    for row in rows {
        let (mod_id, entry) = row.map_err(|e| format!("解析已保存翻译失败: {}", e))?;
        result.insert(mod_id, entry);
    }

    Ok(result)
}

pub(crate) fn nexus_saved_translation_upsert_db(
    db: &Connection,
    mod_key: &str,
    name_translated: Option<&str>,
    desc_translated: Option<&str>,
) -> Result<(), String> {
    let mod_key = mod_key.trim();
    if mod_key.is_empty() {
        return Err("nexus translation key 不能为空".to_string());
    }

    let name_translated = normalize_optional_text(name_translated);
    let desc_translated = normalize_optional_text(desc_translated);

    if name_translated.is_none() && desc_translated.is_none() {
        return Ok(());
    }

    let now = now_millis();
    db.execute(
        "INSERT INTO nexus_saved_translations (
           mod_key,
           name_translated,
           desc_translated,
           updated_at
         )
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(mod_key) DO UPDATE SET
           name_translated = COALESCE(excluded.name_translated, nexus_saved_translations.name_translated),
           desc_translated = COALESCE(excluded.desc_translated, nexus_saved_translations.desc_translated),
           updated_at = excluded.updated_at",
        params![
            mod_key,
            name_translated.as_deref(),
            desc_translated.as_deref(),
            now
        ],
    )
    .map_err(|e| format!("写入 Nexus 已保存翻译失败: {}", e))?;

    Ok(())
}

pub(crate) fn nexus_saved_translations_load_db(
    db: &Connection,
) -> Result<HashMap<String, NexusSavedTranslationRow>, String> {
    let mut stmt = db
        .prepare(
            "SELECT mod_key, name_translated, desc_translated
             FROM nexus_saved_translations",
        )
        .map_err(|e| format!("准备 Nexus 已保存翻译查询失败: {}", e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                NexusSavedTranslationRow {
                    name_translated: row.get::<_, Option<String>>(1)?,
                    desc_translated: row.get::<_, Option<String>>(2)?,
                },
            ))
        })
        .map_err(|e| format!("读取 Nexus 已保存翻译失败: {}", e))?;

    let mut result = HashMap::new();
    for row in rows {
        let (mod_key, entry) = row.map_err(|e| format!("解析 Nexus 已保存翻译失败: {}", e))?;
        result.insert(mod_key, entry);
    }

    Ok(result)
}

pub(crate) fn nexus_mod_cache_upsert_db(
    db: &Connection,
    mods: &[NexusModInfo],
) -> Result<(), String> {
    if mods.is_empty() {
        return Ok(());
    }

    let fetched_at = now_millis();

    for mod_info in mods {
        let data_json = serde_json::to_string(&NexusModCacheRow::from(mod_info))
            .map_err(|e| format!("序列化 Nexus Mod 缓存失败 ({}): {}", mod_info.mod_id, e))?;
        db.execute(
            "INSERT INTO nexus_mod_cache (mod_id, data_json, fetched_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(mod_id) DO UPDATE SET
               data_json = excluded.data_json,
               fetched_at = excluded.fetched_at",
            params![mod_info.mod_id, data_json, fetched_at],
        )
        .map_err(|e| format!("写入 Nexus Mod 缓存失败 ({}): {}", mod_info.mod_id, e))?;
    }

    Ok(())
}

pub(crate) fn nexus_mod_cache_load_db(db: &Connection) -> Result<Vec<NexusModInfo>, String> {
    let mut stmt = db
        .prepare("SELECT data_json FROM nexus_mod_cache ORDER BY fetched_at DESC, mod_id DESC")
        .map_err(|e| format!("准备 Nexus Mod 缓存查询失败: {}", e))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("读取 Nexus Mod 缓存失败: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        let data_json = row.map_err(|e| format!("读取 Nexus Mod 缓存行失败: {}", e))?;
        let mod_info = serde_json::from_str::<NexusModCacheRow>(&data_json)
            .map_err(|e| format!("解析 Nexus Mod 缓存失败: {}", e))?;
        result.push(mod_info.into());
    }

    Ok(result)
}

pub(crate) fn nexus_mod_cache_get_many_db(
    db: &Connection,
    mod_ids: &[u64],
    max_age_millis: Option<i64>,
) -> Result<HashMap<u64, NexusModInfo>, String> {
    if mod_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut result = HashMap::new();
    let fetched_after = max_age_millis.map(|max_age| now_millis().saturating_sub(max_age.max(0)));

    for chunk in mod_ids.chunks(SQLITE_BATCH_SIZE) {
        let placeholders = std::iter::repeat("?")
            .take(chunk.len())
            .collect::<Vec<_>>()
            .join(", ");
        let mut params = chunk
            .iter()
            .map(|mod_id| SqlValue::from(*mod_id as i64))
            .collect::<Vec<_>>();
        let sql = if let Some(fetched_after) = fetched_after {
            params.push(SqlValue::from(fetched_after));
            format!(
                "SELECT mod_id, data_json FROM nexus_mod_cache WHERE mod_id IN ({}) AND fetched_at >= ?",
                placeholders
            )
        } else {
            format!(
                "SELECT mod_id, data_json FROM nexus_mod_cache WHERE mod_id IN ({})",
                placeholders
            )
        };
        let mut stmt = db
            .prepare(&sql)
            .map_err(|e| format!("准备批量 Nexus Mod 缓存查询失败: {}", e))?;
        let rows = stmt
            .query_map(params_from_iter(params.iter()), |row| {
                Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("执行批量 Nexus Mod 缓存查询失败: {}", e))?;

        for row in rows {
            let (mod_id, data_json) =
                row.map_err(|e| format!("读取批量 Nexus Mod 缓存结果失败: {}", e))?;
            let mod_info = serde_json::from_str::<NexusModCacheRow>(&data_json)
                .map_err(|e| format!("解析批量 Nexus Mod 缓存失败 ({}): {}", mod_id, e))?;
            result.insert(mod_id, mod_info.into());
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_nexus_mod(mod_id: u64) -> NexusModInfo {
        NexusModInfo {
            mod_id,
            name: format!("mod-{mod_id}"),
            summary: "summary".to_string(),
            description: None,
            picture_url: None,
            mod_downloads: 1,
            mod_unique_downloads: 1,
            endorsement_count: 1,
            version: "1.0.0".to_string(),
            author: "author".to_string(),
            uploaded_by: "uploader".to_string(),
            category_id: 1,
            created_timestamp: 1,
            updated_timestamp: 1,
            available: true,
            status: "published".to_string(),
        }
    }

    fn init_nexus_cache_test_db() -> Connection {
        let db = Connection::open_in_memory().expect("open in-memory db");
        db.execute_batch(
            "CREATE TABLE nexus_mod_cache (
                mod_id INTEGER PRIMARY KEY,
                data_json TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
            );",
        )
        .expect("create nexus_mod_cache table");
        db
    }

    #[test]
    fn nexus_mod_cache_get_many_db_respects_fetched_at_ttl() {
        let db = init_nexus_cache_test_db();
        let fresh_mod = sample_nexus_mod(1);
        let stale_mod = sample_nexus_mod(2);
        let fresh_json = serde_json::to_string(&NexusModCacheRow::from(&fresh_mod)).unwrap();
        let stale_json = serde_json::to_string(&NexusModCacheRow::from(&stale_mod)).unwrap();
        let now = now_millis();

        db.execute(
            "INSERT INTO nexus_mod_cache (mod_id, data_json, fetched_at) VALUES (?1, ?2, ?3)",
            params![1_u64, fresh_json, now],
        )
        .unwrap();
        db.execute(
            "INSERT INTO nexus_mod_cache (mod_id, data_json, fetched_at) VALUES (?1, ?2, ?3)",
            params![2_u64, stale_json, now - 10_000],
        )
        .unwrap();

        let fresh_only = nexus_mod_cache_get_many_db(&db, &[1, 2], Some(1_000)).unwrap();
        assert!(fresh_only.contains_key(&1));
        assert!(!fresh_only.contains_key(&2));

        let all_rows = nexus_mod_cache_get_many_db(&db, &[1, 2], None).unwrap();
        assert!(all_rows.contains_key(&1));
        assert!(all_rows.contains_key(&2));
    }
}

pub(crate) fn sync_saved_translations_with_game_path_db(
    db: &mut Connection,
    game_path: &str,
) -> Result<(), String> {
    let saved_rows = saved_translations_load_db(db)?;
    if saved_rows.is_empty() {
        return Ok(());
    }

    let mod_lookup = build_mod_source_lookup(game_path);
    if mod_lookup.is_empty() {
        return Ok(());
    }

    let tx = db
        .transaction()
        .map_err(|e| format!("开启已保存翻译同步事务失败: {}", e))?;

    for (mod_id, (source_name, source_desc)) in mod_lookup {
        let Some(saved_row) = saved_rows.get(&mod_id) else {
            continue;
        };
        let source_name_changed = saved_row.source_name != source_name;
        let source_desc_changed = saved_row.source_desc != source_desc;

        if let (Some(source_text), Some(translated)) =
            (source_name.as_deref(), saved_row.name_translated.as_deref())
        {
            upsert_translation_row(&tx, source_text, translated, "compat")?;
        }

        if let (Some(source_text), Some(translated)) =
            (source_desc.as_deref(), saved_row.desc_translated.as_deref())
        {
            upsert_translation_row(&tx, source_text, translated, "compat")?;
        }

        if source_name_changed || source_desc_changed {
            saved_translation_upsert_db(
                &tx,
                &mod_id,
                saved_row.name_translated.as_deref(),
                saved_row.desc_translated.as_deref(),
                source_name.as_deref(),
                source_desc.as_deref(),
            )?;
        }
    }

    tx.commit()
        .map_err(|e| format!("提交已保存翻译同步事务失败: {}", e))?;

    Ok(())
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
         CREATE TABLE IF NOT EXISTS saved_translations (
           mod_id TEXT PRIMARY KEY,
           name_translated TEXT,
           desc_translated TEXT,
           source_name TEXT,
           source_desc TEXT,
           updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS nexus_saved_translations (
           mod_key TEXT PRIMARY KEY,
           name_translated TEXT,
           desc_translated TEXT,
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

    let mod_lookup = config::load_or_detect_game_path()
        .map(|game_path| build_mod_source_lookup(&game_path))
        .unwrap_or_default();

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
        let name_translated = legacy_entry.get("name").and_then(|value| value.as_str());
        let desc_translated = legacy_entry.get("desc").and_then(|value| value.as_str());
        let (source_name, source_desc) = mod_lookup.get(key).cloned().unwrap_or((None, None));

        saved_translation_upsert_db(
            &tx,
            key,
            name_translated,
            desc_translated,
            source_name.as_deref(),
            source_desc.as_deref(),
        )?;

        if let Some(translated_name) = name_translated {
            if let Some(source_text) = source_name.as_deref() {
                if !source_text.trim().is_empty() && !translated_name.trim().is_empty() {
                    upsert_translation_row(&tx, source_text, translated_name, "legacy")?;
                }
            }
        }

        if let Some(translated_desc) = desc_translated {
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

fn collect_nexus_translation_map(state: &tauri::State<'_, AppState>) -> Result<Value, String> {
    let db = state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))?;
    let saved_translations = nexus_saved_translations_load_db(&db)?;
    let mut result = Map::new();

    for (mod_key, saved_row) in saved_translations {
        let mut entry = Map::new();

        if let Some(translated) = saved_row.name_translated {
            entry.insert("name".to_string(), Value::String(translated));
        }

        if let Some(translated) = saved_row.desc_translated {
            entry.insert("desc".to_string(), Value::String(translated));
        }

        if !entry.is_empty() {
            result.insert(mod_key, Value::Object(entry));
        }
    }

    Ok(Value::Object(result))
}

fn persist_nexus_translation_map(
    state: &tauri::State<'_, AppState>,
    data: &Value,
) -> Result<(), String> {
    let Some(entries) = data.as_object() else {
        return Err("nexus_translations_save 需要对象格式数据".to_string());
    };

    let db = state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))?;

    for (mod_key, value) in entries {
        if !mod_key.starts_with("nexus:") {
            continue;
        }

        let Some(entry) = value.as_object() else {
            continue;
        };

        nexus_saved_translation_upsert_db(
            &db,
            mod_key,
            entry.get("name").and_then(|value| value.as_str()),
            entry.get("desc").and_then(|value| value.as_str()),
        )?;
    }

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

#[tauri::command]
pub fn nexus_translations_load(state: tauri::State<'_, AppState>) -> Value {
    match collect_nexus_translation_map(&state) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("nexus_translations_load failed: {}", err);
            serde_json::json!({})
        }
    }
}

#[tauri::command]
pub fn nexus_translations_save(state: tauri::State<'_, AppState>, data: Value) -> Value {
    match persist_nexus_translation_map(&state, &data) {
        Ok(()) => serde_json::json!({ "success": true }),
        Err(err) => {
            eprintln!("nexus_translations_save failed: {}", err);
            serde_json::json!({ "success": false, "error": err })
        }
    }
}
