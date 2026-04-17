import React, { useEffect, useMemo, useState } from 'react';
import {
  CheckCircle2,
  ChevronRight,
  FolderPlus,
  Gamepad2,
  Loader2,
  Plus,
  Search,
  Sparkles,
  X,
} from 'lucide-react';

function buildCustomProfile({ nexusDomain, displayName }) {
  return {
    nexusDomain,
    displayName,
    steamAppId: null,
    exeName: null,
    processName: null,
    steamDirName: null,
    modsSubdir: 'mods',
    appdataDirName: null,
    logsSubdir: null,
    savesEnabled: false,
    logsEnabled: false,
    crashAnalysisEnabled: false,
  };
}

function statusText(entry) {
  if (entry.isCurrent) {
    return entry.gamePath ? '当前游戏' : '当前游戏，未配置路径';
  }
  return entry.gamePath ? '已配置' : '未配置';
}

export default function GameSelector({
  mode = 'fullscreen',
  currentGame = null,
  onGameSelected,
  onClose,
}) {
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');
  const [search, setSearch] = useState('');
  const [games, setGames] = useState([]);
  const [showCustomForm, setShowCustomForm] = useState(false);
  const [customForm, setCustomForm] = useState({
    displayName: '',
    nexusDomain: '',
    gamePath: '',
  });

  const loadGames = async () => {
    setLoading(true);
    try {
      const result = await window.api.listGames();
      setGames(Array.isArray(result?.games) ? result.games : []);
      setError('');
    } catch (loadError) {
      setError(loadError?.message || String(loadError));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadGames();
  }, []);

  const visibleGames = useMemo(() => {
    const keyword = search.trim().toLowerCase();
    if (!keyword) {
      return games;
    }

    return games.filter((entry) => {
      const profile = entry.profile || {};
      return [profile.displayName, profile.nexusDomain]
        .filter(Boolean)
        .some((value) => value.toLowerCase().includes(keyword));
    });
  }, [games, search]);

  const handleConfiguredGame = async (entry) => {
    const info = await window.api.switchGame(entry.profile.nexusDomain);
    onGameSelected?.(info?.currentGame || entry.profile, info);
  };

  const handleUnconfiguredGame = async (entry) => {
    const info = await window.api.switchGame(entry.profile.nexusDomain);
    const pathInfo = await window.api.selectGamePath();

    if (pathInfo) {
      onGameSelected?.(pathInfo.currentGame || info?.currentGame || entry.profile, pathInfo);
      return;
    }

    onGameSelected?.(info?.currentGame || entry.profile, info);
  };

  const handleSelectGame = async (entry) => {
    if (submitting) {
      return;
    }

    setSubmitting(true);
    try {
      setError('');
      if (entry.gamePath) {
        await handleConfiguredGame(entry);
      } else {
        await handleUnconfiguredGame(entry);
      }
    } catch (submitError) {
      setError(submitError?.message || String(submitError));
    } finally {
      setSubmitting(false);
    }
  };

  const handleCustomSubmit = async (event) => {
    event.preventDefault();

    if (submitting) {
      return;
    }

    const displayName = customForm.displayName.trim();
    const nexusDomain = customForm.nexusDomain.trim().toLowerCase();
    const gamePath = customForm.gamePath.trim();

    if (!displayName) {
      setError('请输入游戏名称');
      return;
    }
    if (!nexusDomain) {
      setError('请输入 Nexus 域名');
      return;
    }
    if (!/^[a-z0-9-]+$/.test(nexusDomain)) {
      setError('Nexus 域名只能包含小写字母、数字和连字符');
      return;
    }
    if (!gamePath) {
      setError('请输入游戏路径');
      return;
    }

    setSubmitting(true);
    try {
      setError('');
      const profile = buildCustomProfile({ displayName, nexusDomain });
      const info = await window.api.addGame(profile, gamePath);
      onGameSelected?.(info?.currentGame || profile, info);
    } catch (submitError) {
      setError(submitError?.message || String(submitError));
    } finally {
      setSubmitting(false);
    }
  };

  const shellClasses = mode === 'fullscreen'
    ? 'flex flex-1 bg-gradient-to-br from-slate-950 via-slate-900 to-slate-800'
    : 'fixed inset-0 z-50 flex items-center justify-center bg-black/45 px-6 py-8';
  const panelClasses = mode === 'fullscreen'
    ? 'flex h-full w-full flex-col overflow-hidden bg-transparent'
    : 'w-full max-w-5xl overflow-hidden rounded-[28px] border border-white/10 bg-slate-950 text-white shadow-2xl';
  const contentClasses = mode === 'fullscreen'
    ? 'mx-auto flex h-full w-full max-w-6xl flex-col px-6 py-8 text-white'
    : 'flex max-h-[88vh] flex-col px-6 py-6 text-white';

  return (
    <div className={shellClasses}>
      <div className={panelClasses}>
        <div className={contentClasses}>
          <div className="mb-6 flex items-start justify-between gap-4">
            <div>
              <div className="mb-3 inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs font-semibold uppercase tracking-[0.22em] text-slate-300">
                <Sparkles size={12} />
                Game Profile
              </div>
              <h1 className="text-3xl font-bold tracking-tight">
                {mode === 'fullscreen' ? '选择你要管理的游戏' : '切换当前游戏'}
              </h1>
              <p className="mt-2 max-w-2xl text-sm leading-6 text-slate-300">
                选择已配置的游戏可直接切换；未配置的预设游戏会先引导你选择安装目录。自定义游戏可手动输入 Nexus 域名和本地路径。
              </p>
            </div>
            {mode === 'modal' && (
              <button
                type="button"
                onClick={onClose}
                className="rounded-full border border-white/10 bg-white/5 p-3 text-slate-300 transition-colors hover:bg-white/10 hover:text-white"
                title="关闭"
              >
                <X size={18} />
              </button>
            )}
          </div>

          <div className="mb-6 flex flex-wrap items-center gap-3">
            <div className="relative min-w-[240px] flex-1">
              <Search size={16} className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-500" />
              <input
                type="text"
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="按游戏名或 Nexus 域名搜索"
                className="w-full rounded-2xl border border-white/10 bg-white/5 py-3 pl-11 pr-4 text-sm text-white outline-none transition-colors placeholder:text-slate-500 focus:border-slate-300/40 focus:bg-white/10"
              />
            </div>
            <button
              type="button"
              onClick={() => setShowCustomForm((current) => !current)}
              className={`inline-flex items-center gap-2 rounded-2xl px-4 py-3 text-sm font-medium transition-colors ${
                showCustomForm
                  ? 'bg-white text-slate-900'
                  : 'border border-white/10 bg-white/5 text-slate-200 hover:bg-white/10'
              }`}
            >
              <Plus size={16} />
              自定义游戏
            </button>
          </div>

          {error && (
            <div className="mb-5 rounded-2xl border border-rose-400/20 bg-rose-500/10 px-4 py-3 text-sm text-rose-100">
              {error}
            </div>
          )}

          <div className="grid min-h-0 flex-1 gap-6 xl:grid-cols-[1.25fr_0.75fr]">
            <section className="min-h-0 rounded-[28px] border border-white/10 bg-white/[0.04] p-5 shadow-[0_24px_80px_rgba(15,23,42,0.35)] backdrop-blur">
              <div className="mb-4 flex items-center justify-between gap-3">
                <div>
                  <p className="text-sm font-semibold text-slate-100">预设与已配置游戏</p>
                  <p className="mt-1 text-xs text-slate-400">共 {games.length} 个候选项</p>
                </div>
                {loading && (
                  <span className="inline-flex items-center gap-2 text-xs text-slate-400">
                    <Loader2 size={14} className="animate-spin" />
                    载入中
                  </span>
                )}
              </div>

              <div className="grid max-h-[calc(100vh-260px)] gap-3 overflow-y-auto pr-1 md:grid-cols-2 xl:grid-cols-3">
                {!loading && visibleGames.length === 0 && (
                  <div className="col-span-full rounded-3xl border border-dashed border-white/10 bg-black/10 px-6 py-12 text-center text-sm text-slate-400">
                    没有找到匹配的游戏配置。
                  </div>
                )}

                {visibleGames.map((entry) => {
                  const profile = entry.profile || {};
                  const isActiveCard = currentGame?.nexusDomain === profile.nexusDomain || entry.isCurrent;
                  return (
                    <button
                      key={profile.nexusDomain}
                      type="button"
                      onClick={() => handleSelectGame(entry)}
                      disabled={submitting}
                      className={`group rounded-3xl border p-4 text-left transition-all ${
                        isActiveCard
                          ? 'border-emerald-400/40 bg-emerald-500/10 shadow-[0_16px_40px_rgba(16,185,129,0.14)]'
                          : 'border-white/10 bg-black/10 hover:border-white/20 hover:bg-white/[0.08]'
                      } disabled:cursor-not-allowed disabled:opacity-60`}
                    >
                      <div className="mb-4 flex items-start justify-between gap-3">
                        <div className={`flex h-11 w-11 items-center justify-center rounded-2xl ${
                          isActiveCard ? 'bg-emerald-500 text-white' : 'bg-white/10 text-slate-200'
                        }`}>
                          <Gamepad2 size={18} />
                        </div>
                        {entry.gamePath ? (
                          <span className="inline-flex items-center gap-1 rounded-full border border-emerald-400/20 bg-emerald-400/10 px-2.5 py-1 text-[11px] font-medium text-emerald-200">
                            <CheckCircle2 size={12} />
                            已配置
                          </span>
                        ) : (
                          <span className="rounded-full border border-amber-400/20 bg-amber-400/10 px-2.5 py-1 text-[11px] font-medium text-amber-100">
                            待配置
                          </span>
                        )}
                      </div>

                      <p className="truncate text-base font-semibold text-white">{profile.displayName}</p>
                      <p className="mt-1 text-xs uppercase tracking-[0.16em] text-slate-400">{profile.nexusDomain}</p>
                      <p className="mt-4 min-h-[36px] text-xs leading-5 text-slate-300">{statusText(entry)}</p>

                      <div className="mt-5 flex items-center justify-between text-xs text-slate-400">
                        <span>{entry.gamePath ? '直接切换并刷新界面' : '先选择目录，再进入主界面'}</span>
                        <ChevronRight size={14} className="transition-transform group-hover:translate-x-0.5" />
                      </div>
                    </button>
                  );
                })}
              </div>
            </section>

            <section className="rounded-[28px] border border-white/10 bg-white/[0.04] p-5 shadow-[0_24px_80px_rgba(15,23,42,0.35)] backdrop-blur">
              <div className="mb-4">
                <p className="text-sm font-semibold text-slate-100">自定义游戏</p>
                <p className="mt-1 text-xs leading-5 text-slate-400">
                  适用于暂未内置预设、但你已经知道 Nexus 域名和本地安装路径的游戏。
                </p>
              </div>

              {showCustomForm ? (
                <form onSubmit={handleCustomSubmit} className="space-y-4">
                  <label className="block">
                    <span className="mb-2 block text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">游戏名称</span>
                    <input
                      type="text"
                      value={customForm.displayName}
                      onChange={(event) => setCustomForm((current) => ({ ...current, displayName: event.target.value }))}
                      placeholder="例如 Baldur's Gate 3"
                      className="w-full rounded-2xl border border-white/10 bg-black/15 px-4 py-3 text-sm text-white outline-none transition-colors placeholder:text-slate-500 focus:border-slate-300/40 focus:bg-white/10"
                    />
                  </label>

                  <label className="block">
                    <span className="mb-2 block text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">Nexus 域名</span>
                    <input
                      type="text"
                      value={customForm.nexusDomain}
                      onChange={(event) => setCustomForm((current) => ({
                        ...current,
                        nexusDomain: event.target.value.toLowerCase().replace(/\s+/g, ''),
                      }))}
                      placeholder="例如 baldursgate3"
                      className="w-full rounded-2xl border border-white/10 bg-black/15 px-4 py-3 text-sm text-white outline-none transition-colors placeholder:text-slate-500 focus:border-slate-300/40 focus:bg-white/10"
                    />
                  </label>

                  <label className="block">
                    <span className="mb-2 block text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">游戏路径</span>
                    <div className="relative">
                      <FolderPlus size={16} className="absolute left-4 top-1/2 -translate-y-1/2 text-slate-500" />
                      <input
                        type="text"
                        value={customForm.gamePath}
                        onChange={(event) => setCustomForm((current) => ({ ...current, gamePath: event.target.value }))}
                        placeholder="输入本地安装目录，例如 D:\\Games\\Example"
                        className="w-full rounded-2xl border border-white/10 bg-black/15 py-3 pl-11 pr-4 text-sm text-white outline-none transition-colors placeholder:text-slate-500 focus:border-slate-300/40 focus:bg-white/10"
                      />
                    </div>
                  </label>

                  <div className="rounded-2xl border border-white/10 bg-black/15 px-4 py-3 text-xs leading-5 text-slate-300">
                    自定义游戏默认只启用基础 MOD 管理能力，不会自动配置启动、存档、日志和崩溃分析；后续如需扩展，需要补齐对应的游戏配置。
                  </div>

                  <button
                    type="submit"
                    disabled={submitting}
                    className="inline-flex w-full items-center justify-center gap-2 rounded-2xl bg-white px-4 py-3 text-sm font-semibold text-slate-900 transition-colors hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {submitting ? <Loader2 size={16} className="animate-spin" /> : <Plus size={16} />}
                    添加并切换到这个游戏
                  </button>
                </form>
              ) : (
                <div className="rounded-3xl border border-dashed border-white/10 bg-black/10 px-6 py-12 text-center">
                  <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-white/10 text-slate-200">
                    <Plus size={22} />
                  </div>
                  <p className="mt-4 text-sm font-medium text-white">手动输入自定义游戏信息</p>
                  <p className="mt-2 text-xs leading-5 text-slate-400">
                    如果预设列表里没有目标游戏，可以展开表单，自行填写显示名称、Nexus 域名和本地路径。
                  </p>
                </div>
              )}
            </section>
          </div>
        </div>
      </div>
    </div>
  );
}
