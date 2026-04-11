import React, { useState, useEffect } from 'react';
import {
  X,
  ToggleLeft,
  ToggleRight,
  Trash2,
  AlertTriangle,
  FileText,
  Box,
  Code,
  Languages,
  ExternalLink,
  Shield,
  Gamepad2,
  Palette,
  Download,
  Star,
} from 'lucide-react';
import { formatCompactNumber } from './nexusShared';

function formatSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

function isChinese(text) {
  if (!text) return false;
  return /[\u4e00-\u9fff]/.test(text);
}

function getModCategory(mod, allMods) {
  const isDepForOthers = allMods.some(m => m.id !== mod.id && m.dependencies && m.dependencies.includes(mod.id));
  if (isDepForOthers) return { label: '框架前置', color: 'bg-indigo-50 text-indigo-600', icon: Shield };
  if (mod.affects_gameplay || mod.has_dll) return { label: '玩法改动', color: 'bg-amber-50 text-amber-700', icon: Gamepad2 };
  return { label: '资源类', color: 'bg-teal-50 text-teal-600', icon: Palette };
}

function parseVersionTokens(version) {
  if (!version) return null;
  const normalized = String(version)
    .trim()
    .toLowerCase()
    .replace(/^v(?=\d)/, '');
  const tokens = normalized.match(/\d+/g);
  if (!tokens || tokens.length === 0) return null;
  return tokens.map(Number);
}

function getVersionStabilityRank(version) {
  const normalized = String(version || '')
    .trim()
    .toLowerCase()
    .replace(/^v(?=\d)/, '');

  if (!normalized) {
    return 0;
  }

  if (/(alpha|beta|rc|pre|preview|dev|test)/.test(normalized)) {
    return -1;
  }

  if (/\d+[a-z]+/.test(normalized)) {
    return -1;
  }

  return 0;
}

function compareVersions(localVersion, nexusVersion) {
  const left = parseVersionTokens(localVersion);
  const right = parseVersionTokens(nexusVersion);

  if (!left || !right) {
    return null;
  }

  const length = Math.max(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    const leftToken = left[index] ?? 0;
    const rightToken = right[index] ?? 0;
    if (leftToken === rightToken) {
      continue;
    }
    return leftToken < rightToken ? -1 : 1;
  }

  const stabilityDiff = getVersionStabilityRank(localVersion) - getVersionStabilityRank(nexusVersion);
  if (stabilityDiff !== 0) {
    return stabilityDiff < 0 ? -1 : 1;
  }

  return 0;
}

export default function ModDetail({
  mod,
  allMods,
  onClose,
  onToggle,
  onUninstall,
  onSelectMod,
  onTranslationSaved,
  onShowToast,
}) {
  const enabledIds = allMods.filter(m => m.enabled).map(m => m.id);
  const missingDeps = (mod.dependencies || []).filter(d => !enabledIds.includes(d));
  const dependents = allMods.filter(m => m.dependencies && m.dependencies.includes(mod.id) && m.enabled);
  const category = getModCategory(mod, allMods);
  const CategoryIcon = category.icon;

  const [translatedDesc, setTranslatedDesc] = useState(null);
  const [translatedName, setTranslatedName] = useState(null);
  const [translating, setTranslating] = useState(false);
  const [translateError, setTranslateError] = useState(null);
  const [nexusMatch, setNexusMatch] = useState(null);

  // Load saved translations when mod changes
  useEffect(() => {
    setTranslateError(null);
    if (window.api.loadTranslations) {
      window.api.loadTranslations().then(saved => {
        const t = saved[mod.id];
        if (t) {
          setTranslatedName(t.name || null);
          setTranslatedDesc(t.desc || null);
        } else {
          setTranslatedName(null);
          setTranslatedDesc(null);
        }
      }).catch(() => {
        setTranslatedName(null);
        setTranslatedDesc(null);
      });
    } else {
      setTranslatedName(null);
      setTranslatedDesc(null);
    }
  }, [mod.id, mod.instanceKey]);

  useEffect(() => {
    let cancelled = false;
    const lookupValue = (mod.name || mod.id || '').trim();

    if (!lookupValue || typeof window.api.nexusFindModByName !== 'function') {
      setNexusMatch(null);
      return undefined;
    }

    setNexusMatch(null);

    window.api.nexusFindModByName(lookupValue)
      .then((result) => {
        if (!cancelled) {
          setNexusMatch(result || null);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setNexusMatch(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [mod.id, mod.instanceKey, mod.name]);

  const handleTranslate = async () => {
    setTranslating(true);
    setTranslateError(null);
    try {
      const descText = mod.description || '';
      const nameText = mod.name || '';
      const results = await Promise.all([
        !isChinese(descText) && descText ? window.api.translateText(descText) : null,
        !isChinese(nameText) && nameText ? window.api.translateText(nameText) : null,
      ]);
      let newName = translatedName, newDesc = translatedDesc;
      if (results[0]?.success) { newDesc = results[0].translated; setTranslatedDesc(newDesc); }
      if (results[1]?.success) { newName = results[1].translated; setTranslatedName(newName); }
      if (results[0] && !results[0].success) setTranslateError(results[0].error);
      // Persist
      if (window.api.saveTranslations && (newName || newDesc)) {
        const saved = await window.api.loadTranslations();
        saved[mod.id] = { name: newName, desc: newDesc };
        await window.api.saveTranslations(saved);
        if (onTranslationSaved) onTranslationSaved();
      }
    } catch (e) {
      setTranslateError(e.message);
    }
    setTranslating(false);
  };

  const hasEnglishContent = !isChinese(mod.description) || !isChinese(mod.name);
  const versionComparison = compareVersions(mod.version || '', nexusMatch?.version || '');
  const hasUpdate = versionComparison === -1;

  const openNexusPage = () => {
    if (nexusMatch?.modId) {
      window.api.openUrl(`https://www.nexusmods.com/slaythespire2/mods/${nexusMatch.modId}`);
    }
  };

  const handleUpdateFromNexus = async () => {
    if (!nexusMatch?.modId) {
      return;
    }

    try {
      await window.api.openNexusDownload(nexusMatch.modId, null);
    } catch (error) {
      onShowToast?.(`打开 Nexus 下载窗口失败: ${error?.message || String(error)}`, 'error');
    }
  };

  return (
    <div className="w-80 bg-white border-l border-gray-100 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-gray-50">
        <div className="min-w-0 flex-1 mr-2">
          <h2 className="font-bold text-base truncate">{translatedName || mod.name}</h2>
          {translatedName && <p className="text-[11px] text-gray-400 truncate">{mod.name}</p>}
        </div>
        <button onClick={onClose} className="text-gray-400 hover:text-gray-600 transition-colors flex-shrink-0">
          <X size={18} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">
        {/* Status */}
        <div className="flex items-center justify-between">
          <span className="text-sm text-gray-500">状态</span>
          <button onClick={onToggle} className="flex items-center gap-2">
            {mod.enabled
              ? <><span className="text-sm text-emerald-600 font-medium">已启用</span><ToggleRight size={24} className="text-emerald-500" /></>
              : <><span className="text-sm text-gray-400 font-medium">已禁用</span><ToggleLeft size={24} className="text-gray-300" /></>
            }
          </button>
        </div>

        {/* Category badge */}
        <div className="flex items-center gap-2">
          <span className={`inline-flex items-center gap-1 px-2.5 py-1 rounded-lg text-xs font-medium ${category.color}`}>
            <CategoryIcon size={13} /> {category.label}
          </span>
          {missingDeps.length > 0 && mod.enabled && (
            <span className="inline-flex items-center gap-1 px-2.5 py-1 rounded-lg text-xs font-medium bg-red-50 text-red-600">
              <AlertTriangle size={13} /> 缺失依赖
            </span>
          )}
        </div>

        {/* Info rows */}
        <div className="space-y-3">
          {[
            ['ID', mod.id],
            ['作者', mod.author || '未知'],
            ['版本', mod.version || '未知'],
            ['大小', formatSize(mod.size)],
            ['类型', mod.isFolder ? '文件夹 MOD' : '独立文件 MOD'],
          ].map(([label, value]) => (
            <div key={label} className="flex items-center justify-between">
              <span className="text-xs text-gray-400">{label}</span>
              <span className="text-xs text-gray-700 font-medium">{value}</span>
            </div>
          ))}
        </div>

        {nexusMatch && (
          <div className="rounded-2xl border border-gray-100 bg-white p-4 shadow-sm">
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">Nexus Mods</p>
                <p className="mt-2 truncate text-sm font-semibold text-gray-900">{nexusMatch.name}</p>
              </div>
              <button
                type="button"
                onClick={openNexusPage}
                className="inline-flex flex-shrink-0 items-center gap-1 rounded-lg border border-gray-200 px-3 py-2 text-xs font-medium text-gray-600 transition-colors hover:bg-gray-50 hover:text-gray-900"
              >
                <ExternalLink size={13} />
                在 Nexus 查看
              </button>
            </div>

            <div className="mt-4 flex flex-wrap gap-2">
              <span className="inline-flex items-center gap-1 rounded-full bg-sky-50 px-2.5 py-1 text-xs font-medium text-sky-700">
                <Download size={12} />
                下载 {formatCompactNumber(nexusMatch.modDownloads)}
              </span>
              <span className="inline-flex items-center gap-1 rounded-full bg-amber-50 px-2.5 py-1 text-xs font-medium text-amber-700">
                <Star size={12} />
                赞同 {formatCompactNumber(nexusMatch.endorsementCount)}
              </span>
              {hasUpdate && (
                <span className="inline-flex items-center rounded-full border border-amber-200 bg-amber-50 px-2.5 py-1 text-xs font-medium text-amber-700">
                  有更新
                </span>
              )}
            </div>

            <div className="mt-4 grid grid-cols-2 gap-3">
              <div className="rounded-xl bg-gray-50 px-3 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-gray-400">本地版本</p>
                <p className="mt-2 text-sm font-medium text-gray-800">{mod.version || '未知'}</p>
              </div>
              <div className="rounded-xl bg-gray-50 px-3 py-3">
                <p className="text-[11px] uppercase tracking-[0.18em] text-gray-400">Nexus 最新</p>
                <p className="mt-2 text-sm font-medium text-gray-800">{nexusMatch.version || '未知'}</p>
              </div>
            </div>

            <div className="mt-4 flex items-center justify-between gap-3 rounded-xl bg-gray-50 px-3 py-3">
              <div>
                <p className="text-xs text-gray-400">版本状态</p>
                <p className="mt-1 text-sm font-medium text-gray-800">
                  {hasUpdate
                    ? 'Nexus 上有更高版本可用'
                    : versionComparison === 0
                      ? '当前已是最新版本'
                      : '已找到 Nexus 对应页面'}
                </p>
              </div>
              {hasUpdate && (
                <button
                  type="button"
                  onClick={handleUpdateFromNexus}
                  className="inline-flex items-center gap-2 rounded-lg bg-emerald-600 px-3 py-2 text-xs font-medium text-white transition-colors hover:bg-emerald-500"
                >
                  <Download size={13} />
                  更新
                </button>
              )}
            </div>
          </div>
        )}

        {/* Description */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <p className="text-xs text-gray-400">描述</p>
            {hasEnglishContent && (
              <button onClick={handleTranslate} disabled={translating}
                className="flex items-center gap-1 text-[11px] text-blue-500 hover:text-blue-700 disabled:text-gray-300 transition-colors">
                <Languages size={12} />
                {translating ? '翻译中...' : translatedDesc ? '重新翻译' : '翻译'}
              </button>
            )}
          </div>
          {translatedDesc ? (
            <>
              <p className="text-sm text-gray-700 leading-relaxed">{translatedDesc}</p>
              <p className="text-[11px] text-gray-400 mt-1.5 leading-relaxed">{mod.description}</p>
            </>
          ) : (
            <p className="text-sm text-gray-600 leading-relaxed">{mod.description || '暂无描述'}</p>
          )}
          {translateError && (
            <p className="mt-1 text-xs text-red-400">翻译失败: {translateError}</p>
          )}
        </div>

        {/* Dependencies */}
        {mod.dependencies && mod.dependencies.length > 0 && (
          <div>
            <p className="text-xs text-gray-400 mb-2">依赖项</p>
            {mod.dependencies.map(dep => {
              const isMissing = missingDeps.includes(dep);
              const depMod = allMods.find(m => m.id === dep);
              const canJump = depMod && onSelectMod;
              return (
                <div key={dep}
                  onClick={canJump ? () => onSelectMod(depMod) : undefined}
                  className={`flex items-center gap-2 py-1.5 px-3 rounded-lg text-sm mb-1 ${
                    isMissing ? 'bg-red-50 text-red-600' : 'bg-emerald-50 text-emerald-600'
                  } ${canJump ? 'cursor-pointer hover:ring-1 hover:ring-current/20 transition-all' : ''}`}>
                  {isMissing ? <AlertTriangle size={14} /> : <Box size={14} />}
                  <span className="flex-1 truncate">{depMod ? depMod.name : dep}</span>
                  {isMissing && !depMod && <span className="text-[10px] ml-auto">未安装</span>}
                  {isMissing && depMod && <span className="text-[10px] ml-auto">未启用</span>}
                  {canJump && <ExternalLink size={12} className="flex-shrink-0 opacity-50" />}
                </div>
              );
            })}
          </div>
        )}

        {/* Dependents warning */}
        {dependents.length > 0 && (
          <div className="bg-amber-50 rounded-lg p-3">
            <p className="text-xs text-amber-700 font-medium mb-1">⚠ 以下 MOD 依赖此 MOD</p>
            {dependents.map(d => (
              <p key={d.id} className="text-xs text-amber-600">{d.name}</p>
            ))}
          </div>
        )}

        {/* Files */}
        <div>
          <p className="text-xs text-gray-400 mb-2">文件列表</p>
          <div className="space-y-1">
            {(mod.files || []).map(f => (
              <div key={f} className="flex items-center gap-2 text-xs text-gray-500 py-1">
                {f.endsWith('.dll') ? <Code size={12} /> :
                 f.endsWith('.json') ? <FileText size={12} /> :
                 <Box size={12} />}
                {f}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Footer actions */}
      <div className="p-4 border-t border-gray-50 space-y-2">
        <button onClick={onToggle}
          className={`w-full py-2 rounded-lg text-sm font-medium transition-colors ${
            mod.enabled
              ? 'bg-gray-100 text-gray-700 hover:bg-gray-200'
              : 'bg-gray-900 text-white hover:bg-gray-800'
          }`}>
          {mod.enabled ? '禁用 MOD' : '启用 MOD'}
        </button>
        <button onClick={onUninstall}
          className="w-full py-2 rounded-lg text-sm font-medium text-red-600 bg-red-50 hover:bg-red-100 transition-colors flex items-center justify-center gap-2">
          <Trash2 size={14} /> 卸载 MOD
        </button>
      </div>
    </div>
  );
}
