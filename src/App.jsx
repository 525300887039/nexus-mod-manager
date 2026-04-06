import React, { useState, useEffect, useCallback } from 'react';
import Sidebar from './components/Sidebar';
import ModCard from './components/ModCard';
import ModDetail from './components/ModDetail';
import LogViewer from './components/LogViewer';
import SaveManager from './components/SaveManager';
import ProfileManager from './components/ProfileManager';
import TitleBar from './components/TitleBar';
import {
  Download, RefreshCw, Search, FolderOpen, Archive, UploadCloud, Play, Loader, X, AlertTriangle, Info,
} from 'lucide-react';

export default function App() {
  const [page, setPage] = useState('mods');
  const [mods, setMods] = useState([]);
  const [selectedMod, setSelectedMod] = useState(null);
  const [gamePath, setGamePath] = useState(null);
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState('all'); // all | enabled | disabled
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState(null);
  const [dragOver, setDragOver] = useState(false);
  const [gameState, setGameState] = useState('idle'); // idle | launching | running
  const [crashReport, setCrashReport] = useState(null); // null or { issues, logFile, errorCount, warnCount }
  const [gameVersion, setGameVersion] = useState(null);

  // Listen for game state changes from backend
  useEffect(() => {
    window.api.getGameState().then(setGameState);
    window.api.getGameVersion().then(v => { if (v.version) setGameVersion(v.version); });
    window.api.onGameStateChanged((state) => setGameState(state));
    window.api.onGameExited(async (info) => {
      // Refresh version (new log may have been created)
      const v = await window.api.getGameVersion();
      if (v.version) setGameVersion(v.version);
      // Auto-analyze crash when game exits
      const report = await window.api.analyzeCrash();
      if (report && (report.issues.length > 0 || report.errorCount > 0)) {
        setCrashReport(report);
      }
    });
  }, []);

  const handleLaunchGame = async () => {
    if (gameState !== 'idle') return;
    const result = await window.api.launchGame();
    if (!result.success && result.error) {
      showToast(result.error, 'error');
    }
  };

  const showToast = (msg, type = 'success') => {
    setToast({ msg, type });
    setTimeout(() => setToast(null), 3000);
  };

  const syncMods = useCallback((list) => {
    setMods(list);
    setSelectedMod((prev) => {
      if (!prev?.instanceKey) return null;
      return list.find((m) => m.instanceKey === prev.instanceKey) || null;
    });
  }, []);

  const refreshMods = useCallback(async () => {
    setLoading(true);
    const list = await window.api.scanMods();
    syncMods(list);
    setLoading(false);
  }, [syncMods]);

  useEffect(() => {
    (async () => {
      const info = await window.api.init();
      setGamePath(info.gamePath);
      if (info.gamePath) {
        const list = await window.api.scanMods();
        syncMods(list);
      }
    })();
  }, [syncMods]);

  const handleSelectGamePath = async () => {
    const info = await window.api.selectGamePath();
    if (info) {
      setGamePath(info.gamePath);
      refreshMods();
    }
  };

  const handleToggle = async (mod) => {
    const result = await window.api.toggleMod(mod);
    if (result.success) {
      showToast(`${mod.name} ${mod.enabled ? '已禁用' : '已启用'}`);
      if (result.mods) syncMods(result.mods);
      else refreshMods();
    } else {
      showToast(result.error, 'error');
    }
  };

  const handleUninstall = async (mod) => {
    const result = await window.api.uninstallMod(mod);
    if (result.success) {
      showToast(`${mod.name} 已卸载`);
      if (result.mods) syncMods(result.mods);
      else refreshMods();
    } else {
      showToast(result.error, 'error');
    }
  };

  const handleInstall = async () => {
    const result = await window.api.installMod();
    if (result.success) {
      showToast(`已安装: ${result.installed.join(', ')}`);
      if (result.mods) syncMods(result.mods);
      else refreshMods();
    } else if (result.error !== 'Cancelled') {
      showToast(result.error, 'error');
    }
  };

  const handleBackup = async () => {
    const result = await window.api.backupMods();
    if (result.success) showToast('备份完成');
    else if (result.error) showToast(result.error, 'error');
  };

  const handleRestore = async () => {
    const result = await window.api.restoreMods();
    if (result.success) {
      showToast('还原完成');
      if (result.mods) syncMods(result.mods);
      else refreshMods();
    } else if (result.error) showToast(result.error, 'error');
  };

  // Drag & drop
  const handleDrop = async (e) => {
    e.preventDefault();
    setDragOver(false);
    const files = Array.from(e.dataTransfer.files)
      .filter(f => f.name.endsWith('.zip'))
      .map(f => f.path);
    if (files.length > 0) {
      const result = await window.api.installDrop(files);
      if (result.success) {
        showToast(`已安装: ${result.installed.join(', ')}`);
        if (result.mods) syncMods(result.mods);
        else refreshMods();
      } else {
        showToast(result.error, 'error');
      }
    }
  };

  const filteredMods = mods.filter(m => {
    if (filter === 'enabled' && !m.enabled) return false;
    if (filter === 'disabled' && m.enabled) return false;
    if (search) {
      const s = search.toLowerCase();
      return (m.name || '').toLowerCase().includes(s)
        || (m.id || '').toLowerCase().includes(s)
        || (m.author || '').toLowerCase().includes(s);
    }
    return true;
  });

  const enabledCount = mods.filter(m => m.enabled).length;
  const disabledCount = mods.filter(m => !m.enabled).length;

  return (
    <div className="h-screen flex flex-col bg-white text-gray-900">
      <TitleBar />

      <div className="flex flex-1 overflow-hidden">
        <Sidebar
          page={page}
          setPage={setPage}
          gamePath={gamePath}
          onSelectGamePath={handleSelectGamePath}
          enabledCount={enabledCount}
          totalCount={mods.length}
          gameVersion={gameVersion}
        />

        <main
          className={`flex-1 flex flex-col overflow-hidden transition-colors ${dragOver ? 'bg-blue-50' : 'bg-gray-50'}`}
          onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
          onDragLeave={() => setDragOver(false)}
          onDrop={handleDrop}
        >
          {page === 'mods' && (
            <>
              {/* Header */}
              <div className="px-8 pt-6 pb-4">
                <div className="flex items-center justify-between mb-4">
                  <div>
                    <h1 className="text-2xl font-bold">MOD 管理</h1>
                    <p className="text-sm text-gray-500 mt-1">
                      共 {mods.length} 个 MOD · {enabledCount} 已启用 · {disabledCount} 已禁用
                    </p>
                  </div>
                  <div className="flex items-center gap-2">
                    <button onClick={handleInstall}
                      className="flex items-center gap-2 px-4 py-2 bg-gray-900 text-white rounded-lg text-sm font-medium hover:bg-gray-800 transition-colors">
                      <Download size={16} /> 安装 MOD
                    </button>
                    <button onClick={refreshMods}
                      className="flex items-center gap-2 px-4 py-2 border border-gray-200 rounded-lg text-sm font-medium hover:bg-gray-100 transition-colors">
                      <RefreshCw size={16} className={loading ? 'animate-spin' : ''} /> 刷新
                    </button>
                    <button onClick={handleLaunchGame}
                      disabled={gameState !== 'idle'}
                      className={`flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
                        gameState === 'idle'
                          ? 'bg-emerald-600 text-white hover:bg-emerald-500'
                          : gameState === 'launching'
                            ? 'bg-amber-500 text-white cursor-wait'
                            : 'bg-blue-500 text-white cursor-default'
                      }`}>
                      {gameState === 'idle' && <><Play size={14} /> 启动游戏</>}
                      {gameState === 'launching' && <><Loader size={14} className="animate-spin" /> 正在启动...</>}
                      {gameState === 'running' && <><span className="w-2 h-2 rounded-full bg-white animate-pulse" /> 游戏运行中</>}
                    </button>
                  </div>
                </div>

                {/* Search & filter */}
                <div className="flex items-center gap-3">
                  <div className="relative flex-1 max-w-md">
                    <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                    <input
                      type="text"
                      value={search}
                      onChange={(e) => setSearch(e.target.value)}
                      placeholder="搜索 MOD 名称、作者..."
                      className="w-full pl-10 pr-4 py-2 border border-gray-200 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-gray-200"
                    />
                  </div>
                  <div className="flex bg-gray-100 rounded-lg p-0.5">
                    {[['all', '全部'], ['enabled', '已启用'], ['disabled', '已禁用']].map(([key, label]) => (
                      <button key={key}
                        onClick={() => setFilter(key)}
                        className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                          filter === key ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500 hover:text-gray-700'
                        }`}>
                        {label}
                      </button>
                    ))}
                  </div>
                </div>
              </div>

              {/* Quick actions */}
              <div className="px-8 pb-3 flex gap-2">
                <button onClick={() => window.api.openModsDir()}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-md transition-colors">
                  <FolderOpen size={14} /> MOD 文件夹
                </button>
                <button onClick={() => window.api.openGameDir()}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-md transition-colors">
                  <FolderOpen size={14} /> 游戏目录
                </button>
                <button onClick={handleBackup}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-md transition-colors">
                  <Archive size={14} /> 备份
                </button>
                <button onClick={handleRestore}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-md transition-colors">
                  <UploadCloud size={14} /> 还原
                </button>
              </div>

              {/* Mod grid */}
              <div className="flex-1 overflow-y-auto px-8 pb-6">
                {!gamePath ? (
                  <div className="flex flex-col items-center justify-center h-full text-gray-400">
                    <FolderOpen size={48} className="mb-4" />
                    <p className="text-lg font-medium mb-2">未检测到游戏路径</p>
                    <button onClick={handleSelectGamePath}
                      className="px-4 py-2 bg-gray-900 text-white rounded-lg text-sm">
                      选择游戏目录
                    </button>
                  </div>
                ) : filteredMods.length === 0 ? (
                  <div className="flex flex-col items-center justify-center h-full text-gray-400">
                    <p className="text-lg font-medium">
                      {search ? '没有找到匹配的 MOD' : '暂无 MOD'}
                    </p>
                    <p className="text-sm mt-1">拖拽 ZIP 文件到此处安装</p>
                  </div>
                ) : (
                  <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
                    {filteredMods.map(mod => (
                      <ModCard
                        key={mod.instanceKey || `${mod.id}-${mod.enabled}-${mod.folderName}`}
                        mod={mod}
                        allMods={mods}
                        onToggle={() => handleToggle(mod)}
                        onClick={() => setSelectedMod(mod)}
                        selected={selectedMod?.instanceKey === mod.instanceKey}
                      />
                    ))}
                  </div>
                )}
              </div>

              {/* Drag overlay */}
              {dragOver && (
                <div className="absolute inset-0 bg-blue-50/80 flex items-center justify-center z-50 pointer-events-none">
                  <div className="bg-white rounded-2xl p-8 shadow-lg text-center">
                    <Download size={40} className="mx-auto mb-3 text-gray-900" />
                    <p className="text-lg font-semibold">拖放 ZIP 文件安装 MOD</p>
                  </div>
                </div>
              )}
            </>
          )}

          {page === 'saves' && <SaveManager />}
          {page === 'logs' && <LogViewer />}
          {page === 'profiles' && <ProfileManager mods={mods} onRefresh={refreshMods} />}
        </main>

        {/* Detail panel */}
        {page === 'mods' && selectedMod && (
          <ModDetail
            mod={selectedMod}
            allMods={mods}
            onClose={() => setSelectedMod(null)}
            onToggle={() => handleToggle(selectedMod)}
            onUninstall={() => handleUninstall(selectedMod)}
          />
        )}
      </div>

      {/* Crash Analysis Dialog */}
      {crashReport && (
        <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50" onClick={() => setCrashReport(null)}>
          <div className="bg-white rounded-2xl shadow-2xl w-[480px] max-h-[80vh] overflow-hidden" onClick={e => e.stopPropagation()}>
            {/* Dialog header */}
            <div className="flex items-center justify-between px-6 py-4 border-b border-gray-100">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-full bg-amber-50 flex items-center justify-center">
                  <AlertTriangle size={20} className="text-amber-500" />
                </div>
                <div>
                  <h3 className="font-bold text-gray-900">游戏退出分析</h3>
                  <p className="text-xs text-gray-400">{crashReport.logFile}</p>
                </div>
              </div>
              <button onClick={() => setCrashReport(null)} className="text-gray-400 hover:text-gray-600 transition-colors">
                <X size={18} />
              </button>
            </div>

            {/* Dialog body */}
            <div className="px-6 py-4 overflow-y-auto max-h-[60vh] space-y-4">
              {/* Stats bar */}
              <div className="flex gap-3">
                <div className="flex-1 bg-red-50 rounded-lg px-3 py-2 text-center">
                  <p className="text-lg font-bold text-red-600">{crashReport.errorCount}</p>
                  <p className="text-[10px] text-red-400 uppercase font-semibold">错误</p>
                </div>
                <div className="flex-1 bg-amber-50 rounded-lg px-3 py-2 text-center">
                  <p className="text-lg font-bold text-amber-600">{crashReport.warnCount}</p>
                  <p className="text-[10px] text-amber-400 uppercase font-semibold">警告</p>
                </div>
              </div>

              {/* Issues */}
              {crashReport.issues.length > 0 ? (
                <div className="space-y-2">
                  <p className="text-xs font-semibold text-gray-400 uppercase">检测到的问题</p>
                  {crashReport.issues.map((issue, i) => (
                    <div key={i} className="bg-gray-50 rounded-xl p-4">
                      <div className="flex items-center gap-2 mb-1">
                        <AlertTriangle size={14} className="text-amber-500" />
                        <span className="text-sm font-semibold text-gray-800">{issue.reason}</span>
                      </div>
                      <p className="text-xs text-gray-500 leading-relaxed pl-[22px]">{issue.detail}</p>
                      {issue.mods && issue.mods.length > 0 && (
                        <div className="flex flex-wrap gap-1 mt-2 pl-[22px]">
                          {issue.mods.map(m => (
                            <span key={m} className="text-[10px] px-2 py-0.5 rounded-full bg-amber-100 text-amber-700 font-medium">{m}</span>
                          ))}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="bg-gray-50 rounded-xl p-4 text-center">
                  <Info size={20} className="mx-auto mb-2 text-gray-300" />
                  <p className="text-sm text-gray-500">未检测到明确的崩溃原因</p>
                  <p className="text-xs text-gray-400 mt-1">可能是正常退出，或者查看完整日志获取更多信息</p>
                </div>
              )}

              {/* Involved mods */}
              {crashReport.involvedMods && crashReport.involvedMods.length > 0 && (
                <div className="space-y-2">
                  <p className="text-xs font-semibold text-gray-400 uppercase">涉及的 MOD</p>
                  {crashReport.involvedMods.map((mod, i) => (
                    <div key={i} className="flex items-start gap-3 bg-gray-50 rounded-lg px-4 py-3">
                      <div className="w-7 h-7 rounded-lg bg-red-50 flex items-center justify-center flex-shrink-0 mt-0.5">
                        <span className="text-xs font-bold text-red-500">{mod.errorCount}</span>
                      </div>
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-gray-800">{mod.name}</p>
                        <p className="text-[11px] text-gray-400 truncate">{mod.sample}</p>
                      </div>
                    </div>
                  ))}
                </div>
              )}

              {/* Notices - harmless config file warnings */}
              {crashReport.notices && crashReport.notices.length > 0 && (
                <div>
                  <p className="text-xs font-semibold text-gray-400 uppercase mb-2">无害提示（可忽略）</p>
                  <div className="bg-blue-50 rounded-lg px-4 py-3 space-y-1">
                    {crashReport.notices.map((n, i) => (
                      <p key={i} className="text-[11px] text-blue-600">{n}</p>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Dialog footer */}
            <div className="px-6 py-4 border-t border-gray-100 flex gap-2">
              <button onClick={() => { setCrashReport(null); setPage('logs'); }}
                className="flex-1 py-2.5 rounded-lg text-sm font-medium bg-gray-900 text-white hover:bg-gray-800 transition-colors">
                查看完整日志
              </button>
              <button onClick={() => setCrashReport(null)}
                className="flex-1 py-2.5 rounded-lg text-sm font-medium border border-gray-200 text-gray-700 hover:bg-gray-50 transition-colors">
                关闭
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Toast */}
      {toast && (
        <div className={`fixed bottom-6 right-6 px-4 py-3 rounded-xl shadow-lg text-sm font-medium z-50 transition-all ${
          toast.type === 'error' ? 'bg-red-50 text-red-700 border border-red-200' : 'bg-emerald-50 text-emerald-700 border border-emerald-200'
        }`}>
          {toast.msg}
        </div>
      )}
    </div>
  );
}
