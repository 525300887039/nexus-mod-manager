import React from 'react';
import {
  ChevronRight, FileText, FolderOpen, Globe, Package, Save, Settings, Shuffle, ExternalLink,
} from 'lucide-react';

export default function Sidebar({
  currentGame,
  page,
  setPage,
  gamePath,
  onSelectGamePath,
  onSwitchGame,
  enabledCount,
  totalCount,
  gameVersion,
}) {
  const navItems = [
    { id: 'mods', icon: Package, label: 'MOD 管理' },
    { id: 'nexus', icon: Globe, label: 'Nexus 浏览' },
    currentGame?.savesEnabled && { id: 'saves', icon: Save, label: '存档管理' },
    currentGame?.logsEnabled && { id: 'logs', icon: FileText, label: '游戏日志' },
    { id: 'settings', icon: Settings, label: '设置' },
  ].filter(Boolean);

  const quickLinks = [
    currentGame?.nexusDomain && {
      id: 'nexus',
      icon: ExternalLink,
      label: 'Nexus Mods',
      action: () => window.api.openUrl(`https://www.nexusmods.com/${currentGame.nexusDomain}`),
    },
    { id: 'modsDir', icon: FolderOpen, label: 'MOD 文件夹', action: () => window.api.openModsDir() },
    currentGame?.logsEnabled && { id: 'logsDir', icon: FileText, label: '日志文件夹', action: () => window.api.openLogsDir() },
    currentGame?.savesEnabled && { id: 'savesDir', icon: Save, label: '存档文件夹', action: () => window.api.openSavesDir() },
  ].filter(Boolean);

  return (
    <div className="w-56 bg-white border-r border-gray-100 flex flex-col">
      {/* Nav */}
      <nav className="flex-1 px-3 pt-4">
        <p className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider px-3 mb-2">导航</p>
        {navItems.map(item => (
          <button key={item.id}
            onClick={() => setPage(item.id)}
            className={`w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium mb-0.5 transition-colors ${
              page === item.id
                ? 'bg-gray-900 text-white'
                : 'text-gray-600 hover:bg-gray-50 hover:text-gray-900'
            }`}>
            <item.icon size={18} />
            {item.label}
          </button>
        ))}

        <div className="h-px bg-gray-100 my-4" />

        <p className="text-[10px] font-semibold text-gray-400 uppercase tracking-wider px-3 mb-2">快速访问</p>
        {quickLinks.map(item => (
          <button key={item.id}
            onClick={item.action}
            className="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm text-gray-500 hover:bg-gray-50 hover:text-gray-700 transition-colors mb-0.5">
            <item.icon size={16} />
            {item.label}
          </button>
        ))}
      </nav>

      {/* Game path */}
      <div className="p-3 border-t border-gray-100">
        <div className="bg-gray-50 rounded-xl p-3">
          <div className="mb-3 rounded-lg bg-white px-3 py-3 shadow-sm">
            <div className="flex items-start justify-between gap-2">
              <div className="min-w-0">
                <p className="text-[10px] font-semibold text-gray-400 uppercase">当前游戏</p>
                <p className="mt-1 text-sm font-semibold text-gray-800 truncate">
                  {currentGame?.displayName || '未选择'}
                </p>
                {currentGame?.nexusDomain && (
                  <p className="mt-1 text-[10px] uppercase tracking-[0.16em] text-gray-400">
                    {currentGame.nexusDomain}
                  </p>
                )}
              </div>
              <button
                onClick={onSwitchGame}
                className="inline-flex items-center gap-1 rounded-lg border border-gray-200 px-2.5 py-1.5 text-[11px] font-medium text-gray-600 transition-colors hover:bg-gray-100 hover:text-gray-900"
                title="切换当前游戏"
              >
                <Shuffle size={12} />
                切换
              </button>
            </div>
          </div>

          <div className="flex items-center justify-between mb-1">
            <p className="text-[10px] font-semibold text-gray-400 uppercase">游戏路径</p>
            <button onClick={onSelectGamePath}
              className="text-[10px] text-blue-500 hover:text-blue-700 font-medium transition-colors"
              title="手动选择游戏目录">
              更改
            </button>
          </div>
          {gamePath ? (
            <p className="text-xs text-gray-600 truncate cursor-pointer hover:text-blue-600 transition-colors"
              title={gamePath} onClick={onSelectGamePath}>{gamePath}</p>
          ) : (
            <button onClick={onSelectGamePath}
              className="text-xs text-blue-600 hover:text-blue-800 font-medium">
              点击选择游戏目录
            </button>
          )}
          <div className="flex items-center gap-2 mt-2">
            <div className="w-2 h-2 rounded-full bg-emerald-400" />
            <span className="text-[11px] text-gray-500">{enabledCount}/{totalCount} MOD 已启用</span>
          </div>
          {gameVersion && (
            <div className="mt-2 pt-2 border-t border-gray-100">
              <p className="text-[10px] text-gray-400">游戏版本: <span className="text-gray-600 font-medium">{gameVersion}</span></p>
            </div>
          )}
          {!currentGame?.logsEnabled && !currentGame?.savesEnabled && (
            <div className="mt-2 pt-2 border-t border-gray-100">
              <p className="text-[10px] leading-4 text-gray-400">
                当前游戏仅启用基础 MOD 管理；存档和日志功能会自动隐藏。
              </p>
            </div>
          )}
          <button
            type="button"
            onClick={onSwitchGame}
            className="mt-3 inline-flex w-full items-center justify-between rounded-lg border border-gray-200 bg-white px-3 py-2 text-xs font-medium text-gray-600 transition-colors hover:bg-gray-100 hover:text-gray-900"
          >
            <span>切换到其他游戏</span>
            <ChevronRight size={14} />
          </button>
        </div>
      </div>
    </div>
  );
}
