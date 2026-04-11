import React, { useDeferredValue, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  Globe,
  Languages,
  Loader2,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  Star,
} from 'lucide-react';
import NexusModDetail from './NexusModDetail';
import {
  formatCompactNumber,
  getNexusTranslationKey,
  hasNexusBrowserSupport,
  isChineseText,
  loadNexusTranslationsMap,
  saveNexusTranslationsMap,
} from './nexusShared';

const TAB_CONFIG = {
  trending: {
    label: '热门',
    loader: () => window.api.nexusGetTrending(),
  },
  latestAdded: {
    label: '最新',
    loader: () => window.api.nexusGetLatestAdded(),
  },
  latestUpdated: {
    label: '最近更新',
    loader: () => window.api.nexusGetLatestUpdated(),
  },
};

const TRANSLATE_DELAY_MS = 1000;

function createTabState() {
  return {
    data: [],
    loading: false,
    loaded: false,
    error: '',
  };
}

function createAllTabStates() {
  return {
    trending: createTabState(),
    latestAdded: createTabState(),
    latestUpdated: createTabState(),
  };
}

function delayWithAbort(ms, abortRef) {
  return new Promise((resolve) => {
    const startedAt = Date.now();
    const tick = () => {
      if (abortRef?.current || Date.now() - startedAt >= ms) {
        resolve();
        return;
      }
      window.setTimeout(tick, 50);
    };
    tick();
  });
}

function getBrowserSummary(translationEntry, mod) {
  return translationEntry?.summary || translationEntry?.desc || mod.summary || '暂无摘要';
}

function needsNameTranslation(mod, translationEntry, force = false) {
  if (!mod.name || isChineseText(mod.name)) {
    return false;
  }
  return force || !translationEntry?.name;
}

function needsSummaryTranslation(mod, translationEntry, force = false) {
  if (!mod.summary || isChineseText(mod.summary)) {
    return false;
  }
  return force || (!translationEntry?.summary && !translationEntry?.desc);
}

function NexusModCard({ mod, translationEntry, translating, onClick, onTranslate }) {
  const [imageError, setImageError] = useState(false);
  const isTranslated = !needsNameTranslation(mod, translationEntry) && !needsSummaryTranslation(mod, translationEntry);

  useEffect(() => {
    setImageError(false);
  }, [mod.modId, mod.pictureUrl]);

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          onClick();
        }
      }}
      className="w-full overflow-hidden rounded-2xl border border-gray-100 bg-white text-left shadow-sm transition-all hover:-translate-y-0.5 hover:shadow-lg"
    >
      <div className="overflow-hidden bg-gray-100">
        {mod.pictureUrl && !imageError ? (
          <img
            src={mod.pictureUrl}
            alt={mod.name}
            onError={() => setImageError(true)}
            className="h-44 w-full object-cover"
          />
        ) : (
          <div className="flex h-44 items-center justify-center bg-gray-200 text-gray-400">
            <Globe size={34} />
          </div>
        )}
      </div>

      <div className="space-y-4 px-5 py-4">
        <div>
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <h3 className="truncate text-base font-semibold text-gray-900">
                {translationEntry?.name || mod.name}
              </h3>
              {translationEntry?.name && (
                <p className="mt-1 truncate text-xs text-gray-400">{mod.name}</p>
              )}
            </div>
            <button
              type="button"
              onClick={(event) => {
                event.stopPropagation();
                onTranslate();
              }}
              disabled={translating}
              className="inline-flex flex-shrink-0 items-center gap-1 rounded-lg border border-gray-200 px-2.5 py-1.5 text-xs font-medium text-blue-600 transition-colors hover:bg-blue-50 hover:text-blue-700 disabled:cursor-not-allowed disabled:text-gray-300"
            >
              {translating ? <Loader2 size={14} className="animate-spin" /> : <Languages size={14} />}
              {translating ? '翻译中' : isTranslated ? '重译' : '翻译'}
            </button>
          </div>
          <p className="mt-2 text-sm text-gray-500">作者: {mod.author || mod.uploadedBy || '未知'}</p>
          <p
            className="mt-3 min-h-[40px] text-sm leading-5 text-gray-500"
            style={{
              display: '-webkit-box',
              WebkitLineClamp: 2,
              WebkitBoxOrient: 'vertical',
              overflow: 'hidden',
            }}
          >
            {getBrowserSummary(translationEntry, mod)}
          </p>
        </div>

        <div className="flex flex-wrap gap-2">
          <span className="inline-flex items-center gap-1 rounded-full bg-sky-50 px-2.5 py-1 text-xs font-medium text-sky-700">
            下载 {formatCompactNumber(mod.modDownloads)}
          </span>
          <span className="inline-flex items-center gap-1 rounded-full bg-amber-50 px-2.5 py-1 text-xs font-medium text-amber-700">
            <Star size={12} />
            {formatCompactNumber(mod.endorsementCount)}
          </span>
        </div>
      </div>
    </div>
  );
}

export default function NexusBrowser({
  onNavigate,
  onRefreshMods,
  onShowToast,
  onNexusDownloadStatusChange,
}) {
  const nexusSupported = hasNexusBrowserSupport();
  const [apiKey, setApiKey] = useState('');
  const [initializing, setInitializing] = useState(true);
  const [activeTab, setActiveTab] = useState('trending');
  const [tabStates, setTabStates] = useState(createAllTabStates);
  const [search, setSearch] = useState('');
  const [translations, setTranslations] = useState({});
  const [selectedMod, setSelectedMod] = useState(null);
  const [translatingIds, setTranslatingIds] = useState({});
  const [translating, setTranslating] = useState(false);
  const [translateProgress, setTranslateProgress] = useState({ done: 0, total: 0 });

  const translationsRef = useRef({});
  const tabStatesRef = useRef(createAllTabStates());
  const saveQueueRef = useRef(Promise.resolve());
  const translateAbortRef = useRef(false);
  const deferredSearch = useDeferredValue(search);
  const activeState = tabStates[activeTab];
  const hasApiKey = Boolean(apiKey.trim());

  useEffect(() => {
    translationsRef.current = translations;
  }, [translations]);

  useEffect(() => {
    tabStatesRef.current = tabStates;
  }, [tabStates]);

  useEffect(() => {
    return () => {
      translateAbortRef.current = true;
    };
  }, []);

  useEffect(() => {
    if (translating || translateProgress.total === 0) {
      return undefined;
    }

    const timer = window.setTimeout(() => {
      setTranslateProgress({ done: 0, total: 0 });
    }, 1800);

    return () => window.clearTimeout(timer);
  }, [translating, translateProgress]);

  const persistTranslations = async (nextTranslations) => {
    saveQueueRef.current = saveQueueRef.current
      .catch(() => null)
      .then(() => saveNexusTranslationsMap(nextTranslations));
    await saveQueueRef.current;
  };

  const applyTranslationUpdates = async (translationKey, updates) => {
    const nextTranslations = {
      ...translationsRef.current,
      [translationKey]: {
        ...(translationsRef.current[translationKey] || {}),
        ...updates,
      },
    };
    translationsRef.current = nextTranslations;
    setTranslations(nextTranslations);
    await persistTranslations(nextTranslations);
    return nextTranslations[translationKey];
  };

  const fetchTab = async (tab, options = {}) => {
    const { force = false, keyOverride = '' } = options;
    const effectiveKey = keyOverride || apiKey;

    if (!effectiveKey) {
      return;
    }

    if (!force && tabStatesRef.current[tab]?.loading) {
      return;
    }

    setTabStates((previous) => ({
      ...previous,
      [tab]: {
        ...previous[tab],
        loading: true,
        error: '',
      },
    }));

    try {
      const data = await TAB_CONFIG[tab].loader();
      setTabStates((previous) => ({
        ...previous,
        [tab]: {
          data: Array.isArray(data) ? data : [],
          loading: false,
          loaded: true,
          error: '',
        },
      }));
    } catch (error) {
      const message = error?.message || String(error);
      setTabStates((previous) => ({
        ...previous,
        [tab]: {
          ...previous[tab],
          loading: false,
          loaded: false,
          error: message,
        },
      }));
    }
  };

  useEffect(() => {
    let cancelled = false;

    (async () => {
      if (!nexusSupported) {
        if (!cancelled) {
          setInitializing(false);
        }
        return;
      }

      try {
        const [savedKey, savedTranslations] = await Promise.all([
          window.api.getNexusKey(),
          loadNexusTranslationsMap(),
        ]);

        if (cancelled) {
          return;
        }

        const normalizedKey = savedKey || '';
        setApiKey(normalizedKey);
        setTranslations(savedTranslations);
        translationsRef.current = savedTranslations;

        if (normalizedKey) {
          await fetchTab('trending', { force: true, keyOverride: normalizedKey });
        }
      } finally {
        if (!cancelled) {
          setInitializing(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [nexusSupported]);

  const filteredMods = useMemo(() => {
    const keyword = deferredSearch.trim().toLowerCase();
    if (!keyword) {
      return activeState.data;
    }

    return activeState.data.filter((mod) => {
      const translationEntry = translations[getNexusTranslationKey(mod.modId)] || {};
      return [
        mod.name,
        mod.summary,
        translationEntry.name,
        translationEntry.summary,
        translationEntry.desc,
      ]
        .filter(Boolean)
        .some((value) => value.toLowerCase().includes(keyword));
    });
  }, [activeState.data, deferredSearch, translations]);

  const translateListMod = async (mod, { force = false, throttle = false } = {}) => {
    const translationKey = getNexusTranslationKey(mod.modId);
    const errors = [];

    const translateField = async ({ field, text }) => {
      if (!text) {
        return;
      }

      try {
        const result = await window.api.translateSmart(text);
        if (result?.success && result.translated) {
          await applyTranslationUpdates(translationKey, { [field]: result.translated });
        } else if (result?.error) {
          errors.push(result.error);
        }

        if (throttle && result?.provider && result.provider !== 'cache') {
          await delayWithAbort(TRANSLATE_DELAY_MS, translateAbortRef);
        }
      } catch (error) {
        errors.push(error?.message || String(error));
      }
    };

    let translationEntry = translationsRef.current[translationKey] || {};

    if (translateAbortRef.current) {
      return { aborted: true, errors };
    }

    if (needsNameTranslation(mod, translationEntry, force)) {
      await translateField({ field: 'name', text: mod.name });
      translationEntry = translationsRef.current[translationKey] || translationEntry;
    }

    if (translateAbortRef.current) {
      return { aborted: true, errors };
    }

    if (needsSummaryTranslation(mod, translationEntry, force)) {
      await translateField({ field: 'summary', text: mod.summary });
    }

    return { aborted: translateAbortRef.current, errors };
  };

  const handleTranslateCard = async (mod) => {
    if (translating) {
      return;
    }

    translateAbortRef.current = false;
    setTranslatingIds((previous) => ({ ...previous, [mod.modId]: true }));
    try {
      const result = await translateListMod(mod, { force: true, throttle: false });
      if (result.errors.length > 0) {
        onShowToast?.(result.errors[0], 'error');
      } else {
        onShowToast?.(`已更新 ${mod.name} 的翻译。`);
      }
    } catch (error) {
      onShowToast?.(error?.message || String(error), 'error');
    } finally {
      setTranslatingIds((previous) => ({ ...previous, [mod.modId]: false }));
    }
  };

  const handleTranslateAll = async () => {
    if (translating) {
      return;
    }

    const targets = filteredMods.filter((mod) => {
      const translationEntry = translationsRef.current[getNexusTranslationKey(mod.modId)] || {};
      return needsNameTranslation(mod, translationEntry) || needsSummaryTranslation(mod, translationEntry);
    });

    if (targets.length === 0) {
      onShowToast?.('当前列表没有需要翻译的 Mod。');
      return;
    }

    translateAbortRef.current = false;
    setTranslating(true);
    setTranslateProgress({ done: 0, total: targets.length });

    let done = 0;
    let firstError = '';
    let errorCount = 0;

    for (const mod of targets) {
      if (translateAbortRef.current) {
        break;
      }

      setTranslatingIds((previous) => ({ ...previous, [mod.modId]: true }));

      try {
        const result = await translateListMod(mod, { force: false, throttle: true });
        if (result.errors.length > 0) {
          errorCount += result.errors.length;
          if (!firstError) {
            firstError = result.errors[0];
          }
        }
        if (result.aborted && translateAbortRef.current) {
          break;
        }
      } catch (error) {
        errorCount += 1;
        if (!firstError) {
          firstError = error?.message || String(error);
        }
      } finally {
        setTranslatingIds((previous) => ({ ...previous, [mod.modId]: false }));
      }

      if (translateAbortRef.current) {
        break;
      }

      done += 1;
      setTranslateProgress({ done, total: targets.length });
    }

    setTranslating(false);

    if (translateAbortRef.current) {
      onShowToast?.(`已取消批量翻译，已完成 ${done}/${targets.length} 个 Mod。`, 'error');
      return;
    }

    if (errorCount > 0) {
      onShowToast?.(
        firstError || `批量翻译完成，但有 ${errorCount} 个字段翻译失败。`,
        'error',
      );
      return;
    }

    onShowToast?.(`批量翻译完成，已处理 ${done} 个 Mod。`);
  };

  const handleCancelTranslate = () => {
    translateAbortRef.current = true;
  };

  const handleTabChange = async (tab) => {
    setActiveTab(tab);
    if (!hasApiKey) {
      return;
    }
    const nextTabState = tabStatesRef.current[tab];
    if (!nextTabState?.loaded && !nextTabState?.loading) {
      await fetchTab(tab);
    }
  };

  const translatePercent = translateProgress.total > 0
    ? Math.round((translateProgress.done / translateProgress.total) * 100)
    : 0;

  return (
    <div className="flex flex-1 overflow-hidden">
      <div className="flex-1 overflow-y-auto">
        <div className="px-8 pt-6 pb-4">
          <div className="mb-6 flex items-start justify-between gap-4">
            <div>
              <h1 className="text-2xl font-bold text-gray-900">Nexus 浏览</h1>
              <p className="mt-1 text-sm text-gray-500">
                浏览 Slay the Spire 2 在 Nexus Mods 上的热门、新增和最近更新内容。
              </p>
            </div>
            {nexusSupported && (
              <div className="flex items-center gap-2">
                {hasApiKey && (
                  <button
                    type="button"
                    onClick={() => fetchTab(activeTab, { force: true })}
                    className="inline-flex items-center gap-2 rounded-xl border border-gray-200 bg-white px-4 py-2.5 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50"
                  >
                    <RefreshCw size={16} className={activeState.loading ? 'animate-spin' : ''} />
                    刷新
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => onNavigate?.('settings', { tab: 'nexus' })}
                  className="inline-flex items-center gap-2 rounded-xl bg-gray-900 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-gray-800"
                >
                  <Settings size={16} />
                  Nexus 设置
                </button>
              </div>
            )}
          </div>

          {!nexusSupported ? (
            <div className="rounded-3xl border border-amber-100 bg-amber-50 px-8 py-12 text-center shadow-sm">
              <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-full bg-amber-100 text-amber-700">
                <AlertTriangle size={24} />
              </div>
              <h2 className="mt-5 text-xl font-semibold text-gray-900">当前运行环境不支持 Nexus 浏览</h2>
              <p className="mx-auto mt-3 max-w-2xl text-sm leading-6 text-gray-600">
                这个页面依赖 Tauri 侧的 Nexus Mods bridge。你当前打开的是 Electron 运行时，它没有接入这些 API，因此这里只显示只读提示，避免进入页面后直接报错。
              </p>
            </div>
          ) : initializing ? (
            <div className="flex min-h-[360px] items-center justify-center rounded-2xl border border-gray-100 bg-white">
              <div className="flex items-center gap-3 text-sm text-gray-500">
                <Loader2 size={18} className="animate-spin" />
                正在加载 Nexus 浏览器...
              </div>
            </div>
          ) : !hasApiKey ? (
            <div className="rounded-3xl border-2 border-dashed border-gray-200 bg-white px-8 py-12 text-center shadow-sm">
              <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-full bg-gray-900 text-white">
                <Globe size={24} />
              </div>
              <h2 className="mt-5 text-xl font-semibold text-gray-900">配置 Nexus Mods API Key 以浏览在线 Mod</h2>
              <p className="mx-auto mt-3 max-w-2xl text-sm leading-6 text-gray-500">
                这个页面的列表与详情都通过 Rust 后端代理请求 Nexus Mods V1 API。先完成 API Key 验证并保存，然后即可浏览热门、最新和最近更新的模组。
              </p>
              <div className="mt-6 flex justify-center">
                <button
                  type="button"
                  onClick={() => onNavigate?.('settings', { tab: 'nexus' })}
                  className="inline-flex items-center gap-2 rounded-xl bg-gray-900 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-gray-800"
                >
                  <ShieldCheck size={16} />
                  前往设置
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="mb-4 flex flex-wrap items-center gap-3">
                <div className="flex rounded-xl bg-gray-100 p-1">
                  {Object.entries(TAB_CONFIG).map(([tab, config]) => (
                    <button
                      key={tab}
                      type="button"
                      onClick={() => handleTabChange(tab)}
                      className={`rounded-lg px-4 py-2 text-sm font-medium transition-colors ${
                        activeTab === tab
                          ? 'bg-white text-gray-900 shadow-sm'
                          : 'text-gray-500 hover:text-gray-700'
                      }`}
                    >
                      {config.label}
                    </button>
                  ))}
                </div>

                <div className="relative min-w-[260px] flex-1 max-w-md">
                  <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                  <input
                    type="text"
                    value={search}
                    onChange={(event) => setSearch(event.target.value)}
                    placeholder="按名称或摘要过滤当前列表"
                    className="w-full rounded-xl border border-gray-200 bg-white py-2.5 pl-10 pr-4 text-sm text-gray-900 outline-none transition-colors focus:border-gray-900 focus:ring-2 focus:ring-gray-900/5"
                  />
                </div>

                <button
                  type="button"
                  onClick={handleTranslateAll}
                  disabled={translating || filteredMods.length === 0}
                  className="inline-flex items-center gap-2 rounded-xl border border-gray-200 bg-white px-4 py-2.5 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  {translating ? <Loader2 size={16} className="animate-spin" /> : <Languages size={16} />}
                  {translating ? '翻译中...' : '全部翻译'}
                </button>
              </div>

              {(translating || translateProgress.total > 0) && (
                <div className="mb-4 rounded-2xl border border-gray-100 bg-white px-4 py-4 shadow-sm">
                  <div className="mb-3 flex items-center justify-between gap-4">
                    <div>
                      <p className="text-sm font-semibold text-gray-900">批量翻译进度</p>
                      <p className="mt-1 text-xs text-gray-500">
                        已完成 {translateProgress.done}/{translateProgress.total} · {translatePercent}%
                      </p>
                    </div>
                    {translating && (
                      <button
                        type="button"
                        onClick={handleCancelTranslate}
                        className="inline-flex items-center gap-2 rounded-lg bg-red-50 px-3 py-2 text-xs font-medium text-red-600 transition-colors hover:bg-red-100"
                      >
                        取消
                      </button>
                    )}
                  </div>
                  <div className="h-2 overflow-hidden rounded-full bg-gray-100">
                    <div
                      className="h-full rounded-full bg-emerald-500 transition-[width] duration-300"
                      style={{ width: `${translatePercent}%` }}
                    />
                  </div>
                </div>
              )}

              {activeState.error && (
                <div className="mb-4 rounded-xl border border-red-100 bg-red-50 px-4 py-3 text-sm text-red-700">
                  <div className="flex items-start gap-2">
                    <AlertTriangle size={16} className="mt-0.5 flex-shrink-0" />
                    <span>{activeState.error}</span>
                  </div>
                </div>
              )}

              {activeState.loading && activeState.data.length === 0 ? (
                <div className="flex min-h-[320px] items-center justify-center rounded-2xl border border-gray-100 bg-white">
                  <div className="flex items-center gap-3 text-sm text-gray-500">
                    <Loader2 size={18} className="animate-spin" />
                    正在加载 {TAB_CONFIG[activeTab].label} 模组...
                  </div>
                </div>
              ) : filteredMods.length === 0 ? (
                <div className="rounded-2xl border border-dashed border-gray-200 bg-white px-6 py-16 text-center text-sm text-gray-400">
                  {search ? '没有找到匹配的 Nexus Mod。' : '当前列表暂无可展示的 Mod。'}
                </div>
              ) : (
                <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 xl:grid-cols-3">
                  {filteredMods.map((mod) => (
                    <NexusModCard
                      key={mod.modId}
                      mod={mod}
                      translationEntry={translations[getNexusTranslationKey(mod.modId)]}
                      translating={Boolean(translatingIds[mod.modId]) || translating}
                      onClick={() => setSelectedMod(mod)}
                      onTranslate={() => handleTranslateCard(mod)}
                    />
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {selectedMod && (
        <NexusModDetail
          mod={selectedMod}
          translationEntry={translations[getNexusTranslationKey(selectedMod.modId)]}
          onClose={() => setSelectedMod(null)}
          onTranslationsChange={(nextTranslations) => {
            setTranslations(nextTranslations);
            translationsRef.current = nextTranslations;
          }}
          onRefreshMods={onRefreshMods}
          onShowToast={onShowToast}
          onNexusDownloadStatusChange={onNexusDownloadStatusChange}
        />
      )}
    </div>
  );
}
