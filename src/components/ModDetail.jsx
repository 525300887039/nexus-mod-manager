import React, { useState, useEffect } from 'react';
import { X, ToggleLeft, ToggleRight, Trash2, AlertTriangle, FileText, Box, Code, Languages } from 'lucide-react';

function formatSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

function isChinese(text) {
  if (!text) return false;
  return /[\u4e00-\u9fff]/.test(text);
}

export default function ModDetail({ mod, allMods, onClose, onToggle, onUninstall }) {
  const enabledIds = allMods.filter(m => m.enabled).map(m => m.id);
  const missingDeps = (mod.dependencies || []).filter(d => !enabledIds.includes(d));
  const dependents = allMods.filter(m => m.dependencies && m.dependencies.includes(mod.id) && m.enabled);

  const [translatedDesc, setTranslatedDesc] = useState(null);
  const [translatedName, setTranslatedName] = useState(null);
  const [translating, setTranslating] = useState(false);
  const [translateError, setTranslateError] = useState(null);

  // Reset translation when mod changes
  useEffect(() => {
    setTranslatedDesc(null);
    setTranslatedName(null);
    setTranslateError(null);
  }, [mod.id, mod.instanceKey]);

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
      if (results[0]?.success) setTranslatedDesc(results[0].translated);
      if (results[1]?.success) setTranslatedName(results[1].translated);
      if (results[0] && !results[0].success) setTranslateError(results[0].error);
    } catch (e) {
      setTranslateError(e.message);
    }
    setTranslating(false);
  };

  const hasEnglishContent = !isChinese(mod.description) || !isChinese(mod.name);

  return (
    <div className="w-80 bg-white border-l border-gray-100 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-gray-50">
        <h2 className="font-bold text-base truncate">{mod.name}</h2>
        <button onClick={onClose} className="text-gray-400 hover:text-gray-600 transition-colors">
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

        {/* Info rows */}
        <div className="space-y-3">
          {[
            ['ID', mod.id],
            ['作者', mod.author || '未知'],
            ['版本', mod.version || '未知'],
            ['大小', formatSize(mod.size)],
            ['影响玩法', mod.affects_gameplay ? '是' : '否'],
            ['类型', mod.isFolder ? '文件夹 MOD' : '独立文件 MOD'],
          ].map(([label, value]) => (
            <div key={label} className="flex items-center justify-between">
              <span className="text-xs text-gray-400">{label}</span>
              <span className="text-xs text-gray-700 font-medium">{value}</span>
            </div>
          ))}
        </div>

        {/* Translated name */}
        {translatedName && (
          <div className="bg-blue-50 rounded-lg px-3 py-2">
            <p className="text-[10px] text-blue-400 font-semibold mb-0.5">名称翻译</p>
            <p className="text-sm text-blue-700 font-medium">{translatedName}</p>
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
          <p className="text-sm text-gray-600 leading-relaxed">{mod.description || '暂无描述'}</p>
          {translatedDesc && (
            <div className="mt-2 bg-blue-50 rounded-lg px-3 py-2">
              <p className="text-[10px] text-blue-400 font-semibold mb-0.5">中文翻译</p>
              <p className="text-sm text-blue-700 leading-relaxed">{translatedDesc}</p>
            </div>
          )}
          {translateError && (
            <p className="mt-1 text-xs text-red-400">翻译失败: {translateError}</p>
          )}
        </div>

        {/* Dependencies */}
        {mod.dependencies && mod.dependencies.length > 0 && (
          <div>
            <p className="text-xs text-gray-400 mb-2">依赖</p>
            {mod.dependencies.map(dep => {
              const isMissing = missingDeps.includes(dep);
              return (
                <div key={dep} className={`flex items-center gap-2 py-1.5 px-3 rounded-lg text-sm mb-1 ${
                  isMissing ? 'bg-red-50 text-red-600' : 'bg-emerald-50 text-emerald-600'
                }`}>
                  {isMissing ? <AlertTriangle size={14} /> : <Box size={14} />}
                  {dep}
                  {isMissing && <span className="text-xs ml-auto">未安装/未启用</span>}
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
