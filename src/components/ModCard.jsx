import React from 'react';
import { ToggleLeft, ToggleRight, AlertTriangle } from 'lucide-react';

function formatSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

function getTagColor(mod) {
  if (!mod.enabled) return 'bg-gray-100 text-gray-500';
  if (mod.affects_gameplay) return 'bg-amber-50 text-amber-700';
  return 'bg-emerald-50 text-emerald-700';
}

function getTagLabel(mod) {
  if (!mod.enabled) return '已禁用';
  if (mod.affects_gameplay) return '影响玩法';
  return '已启用';
}

function hasMissingDeps(mod, allMods) {
  if (!mod.dependencies || mod.dependencies.length === 0) return false;
  const enabledIds = allMods.filter(m => m.enabled).map(m => m.id);
  return mod.dependencies.some(dep => !enabledIds.includes(dep));
}

export default function ModCard({ mod, allMods, onToggle, onClick, selected }) {
  const missingDeps = hasMissingDeps(mod, allMods);

  return (
    <div
      onClick={onClick}
      className={`relative bg-white rounded-xl border p-4 cursor-pointer transition-all hover:shadow-md ${
        selected ? 'border-gray-900 shadow-md' : 'border-gray-100 hover:border-gray-200'
      } ${!mod.enabled ? 'opacity-60' : ''}`}
    >
      {/* Header */}
      <div className="flex items-start justify-between mb-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="font-semibold text-sm truncate">{mod.name}</h3>
            {missingDeps && (
              <AlertTriangle size={14} className="text-red-500 flex-shrink-0" title="缺少依赖" />
            )}
          </div>
          <p className="text-xs text-gray-400 mt-0.5">
            {mod.author} · v{mod.version}
          </p>
        </div>
        <button
          onClick={(e) => { e.stopPropagation(); onToggle(); }}
          className="flex-shrink-0 ml-2"
          title={mod.enabled ? '点击禁用' : '点击启用'}
        >
          {mod.enabled
            ? <ToggleRight size={28} className="text-emerald-500" />
            : <ToggleLeft size={28} className="text-gray-300" />
          }
        </button>
      </div>

      {/* Description */}
      <p className="text-xs text-gray-500 line-clamp-2 mb-3 leading-relaxed">
        {mod.description || '暂无描述'}
      </p>

      {/* Tags */}
      <div className="flex items-center gap-2 flex-wrap">
        <span className={`inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-medium ${getTagColor(mod)}`}>
          {getTagLabel(mod)}
        </span>
        {mod.has_dll && (
          <span className="inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-medium bg-blue-50 text-blue-600">
            DLL
          </span>
        )}
        {mod.has_pck && (
          <span className="inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-medium bg-purple-50 text-purple-600">
            PCK
          </span>
        )}
        {mod.dependencies && mod.dependencies.length > 0 && (
          <span className="inline-flex items-center px-2 py-0.5 rounded-md text-[11px] font-medium bg-orange-50 text-orange-600">
            依赖: {mod.dependencies.join(', ')}
          </span>
        )}
        <span className="ml-auto text-[11px] text-gray-300">{formatSize(mod.size)}</span>
      </div>
    </div>
  );
}
