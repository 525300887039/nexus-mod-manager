import React, { useDeferredValue, useEffect, useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  ChevronLeft,
  ChevronRight,
  Globe,
  Languages,
  Link2,
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
  parseNexusModUrl,
  saveNexusTranslationsMap,
} from './nexusShared';

const UPDATED_PAGED_TAB = 'pagedBrowse';
const POPULAR_PAGED_TAB = 'popularPagedBrowse';
const PAGED_DEFAULT_PERIOD = '1m';
const PAGED_DEFAULT_PAGE_SIZE = 20;

const PAGED_PERIOD_OPTIONS = [
  { value: '1d', label: '近 1 天' },
  { value: '1w', label: '近 1 周' },
  { value: '1m', label: '近 1 月' },
];

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
  [UPDATED_PAGED_TAB]: {
    label: '最近更新分页',
    paged: true,
    defaultPeriod: '1m',
    loader: ({ period, page, pageSize, force }) => window.api.nexusGetRecentlyUpdatedPage(
      period,
      page,
      pageSize,
      force,
    ),
  },
  [POPULAR_PAGED_TAB]: {
    label: '网页热门',
    paged: true,
    defaultPeriod: '1w',
    loader: ({ period, page, pageSize, force }) => window.api.nexusGetPopularPage(
      period,
      page,
      pageSize,
      force,
    ),
  },
};

const TRANSLATE_DELAY_MS = 1000;

function createTabState(tab) {
  const baseState = {
    data: [],
    loading: false,
    loaded: false,
    error: '',
    warning: '',
  };

  if (TAB_CONFIG[tab]?.paged) {
    return {
      ...baseState,
      page: 1,
      pageSize: PAGED_DEFAULT_PAGE_SIZE,
      totalItems: 0,
      totalPages: 0,
      hasPrev: false,
      hasNext: false,
      period: TAB_CONFIG[tab].defaultPeriod || PAGED_DEFAULT_PERIOD,
    };
  }

  return baseState;
}

function createAllTabStates() {
  return Object.keys(TAB_CONFIG).reduce((result, tab) => {
    result[tab] = createTabState(tab);
    return result;
  }, {});
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
  const isTranslated = !needsNameTranslation(mod, translationEntry)
    && !needsSummaryTranslation(mod, translationEntry);

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
  const [selectedFileId, setSelectedFileId] = useState(null);
  const [translatingIds, setTranslatingIds] = useState({});
  const [translating, setTranslating] = useState(false);
  const [translateProgress, setTranslateProgress] = useState({ done: 0, total: 0 });
  const [linkInput, setLinkInput] = useState('');
  const [linkError, setLinkError] = useState('');
  const [openingLink, setOpeningLink] = useState(false);

  const translationsRef = useRef({});
  const tabStatesRef = useRef(createAllTabStates());
  const saveQueueRef = useRef(Promise.resolve());
  const translateAbortRef = useRef(false);
  const deferredSearch = useDeferredValue(search);
  const activeState = tabStates[activeTab];
  const activeTabConfig = TAB_CONFIG[activeTab];
  const hasApiKey = Boolean(apiKey.trim());
  const isPagedTab = Boolean(activeTabConfig?.paged);

  useEffect(() => {
    translationsRef.current = translations;
  }, [translations]);

  useEffect(() => {
    tabStatesRef.current = tabStates;
  }, [tabStates]);

  useEffect(() => () => {
    translateAbortRef.current = true;
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

  const handleSelectMod = (mod, fileId = null) => {
    setSelectedMod(mod);
    setSelectedFileId(fileId);
  };

  const handleCloseSelectedMod = () => {
    setSelectedMod(null);
    setSelectedFileId(null);
  };

  const fetchTab = async (tab, options = {}) => {
    const {
      force = false,
      keyOverride = '',
      page,
      pageSize,
      period,
    } = options;
    const effectiveKey = keyOverride || apiKey;

    if (!effectiveKey) {
      return;
    }

    if (!force && tabStatesRef.current[tab]?.loading) {
      return;
    }

    const currentState = tabStatesRef.current[tab] || createTabState(tab);
    const previousStateSnapshot = { ...currentState };
    const nextPage = TAB_CONFIG[tab].paged ? (page ?? currentState.page ?? 1) : undefined;
    const nextPageSize = TAB_CONFIG[tab].paged
      ? (pageSize ?? currentState.pageSize ?? PAGED_DEFAULT_PAGE_SIZE)
      : undefined;
    const nextPeriod = TAB_CONFIG[tab].paged
      ? (period ?? currentState.period ?? TAB_CONFIG[tab].defaultPeriod ?? PAGED_DEFAULT_PERIOD)
      : undefined;

    setTabStates((previous) => ({
      ...previous,
      [tab]: {
        ...previous[tab],
        loading: true,
        error: '',
        warning: '',
        ...(TAB_CONFIG[tab].paged
          ? {
            page: nextPage,
            pageSize: nextPageSize,
            period: nextPeriod,
          }
          : {}),
      },
    }));

    try {
      if (TAB_CONFIG[tab].paged) {
        const result = await TAB_CONFIG[tab].loader({
          force,
          page: nextPage,
          pageSize: nextPageSize,
          period: nextPeriod,
        });
        setTabStates((previous) => ({
          ...previous,
          [tab]: {
            ...previous[tab],
            data: Array.isArray(result?.items) ? result.items : [],
            loading: false,
            loaded: true,
            error: '',
            warning: result?.warning || '',
            page: result?.page || nextPage,
            pageSize: result?.pageSize || nextPageSize,
            period: nextPeriod,
            totalItems: result?.totalItems || 0,
            totalPages: result?.totalPages || 0,
            hasPrev: Boolean(result?.hasPrev),
            hasNext: Boolean(result?.hasNext),
          },
        }));
        return;
      }

      const data = await TAB_CONFIG[tab].loader();
      setTabStates((previous) => ({
        ...previous,
        [tab]: {
          ...previous[tab],
          data: Array.isArray(data) ? data : [],
          loading: false,
          loaded: true,
          error: '',
          warning: '',
        },
      }));
    } catch (error) {
      const message = error?.message || String(error);
      setTabStates((previous) => ({
        ...previous,
        [tab]: {
          ...(TAB_CONFIG[tab].paged ? previousStateSnapshot : previous[tab]),
          loading: false,
          loaded: TAB_CONFIG[tab].paged
            ? previousStateSnapshot.loaded
            : previous[tab]?.loaded || false,
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
      return needsNameTranslation(mod, translationEntry)
        || needsSummaryTranslation(mod, translationEntry);
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

  const handleOpenModLink = async (event) => {
    event?.preventDefault();

    if (openingLink) {
      return;
    }

    const rawValue = linkInput.trim();
    if (!rawValue) {
      setLinkError('请输入 Nexus Mod 链接');
      return;
    }

    if (!hasApiKey) {
      setLinkError('请先配置 Nexus Mods API Key，再通过链接打开 Mod 详情');
      return;
    }

    setLinkError('');
    setOpeningLink(true);

    try {
      const parsed = parseNexusModUrl(rawValue);
      const mod = await window.api.nexusGetMod(parsed.modId);

      setLinkInput(parsed.canonicalUrl);
      handleSelectMod(mod, parsed.fileId);
    } catch (error) {
      setLinkError(error?.message || String(error));
    } finally {
      setOpeningLink(false);
    }
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

  const handleRefreshCurrentTab = () => {
    fetchTab(activeTab, { force: true });
  };

  const handlePagedPeriodChange = (period) => {
    if (activeState.loading || activeState.period === period) {
      return;
    }
    fetchTab(activeTab, {
      period,
      page: 1,
      force: false,
    });
  };

  const handlePagedPageChange = (nextPage) => {
    if (activeState.loading || nextPage < 1 || nextPage === activeState.page) {
      return;
    }
    fetchTab(activeTab, { page: nextPage, force: false });
  };

  const translatePercent = translateProgress.total > 0
    ? Math.round((translateProgress.done / translateProgress.total) * 100)
    : 0;
  const pageStart = isPagedTab && activeState.totalItems > 0
    ? ((activeState.page - 1) * activeState.pageSize) + 1
    : 0;
  const pageEnd = isPagedTab && activeState.totalItems > 0
    ? Math.min(activeState.totalItems, activeState.page * activeState.pageSize)
    : 0;
  const pagedTitle = activeTab === POPULAR_PAGED_TAB ? '网页热门' : '最近更新分页';
  const pagedDescription = activeTab === POPULAR_PAGED_TAB
    ? `基于 Nexus 网页端热门列表的 GraphQL 分页加载。当前时间范围内共有 ${formatCompactNumber(activeState.totalItems || 0)} 个 Mod。`
    : `基于 Nexus 最近更新列表分页加载详情。当前时间范围内共有 ${formatCompactNumber(activeState.totalItems || 0)} 个 Mod。`;

  return (
    <div className="flex flex-1 overflow-hidden">
      <div className="flex-1 overflow-y-auto">
        <div className="px-8 pt-6 pb-4">
          <div className="mb-6 flex items-start justify-between gap-4">
            <div>
              <h1 className="text-2xl font-bold text-gray-900">Nexus 浏览</h1>
              <p className="mt-1 text-sm text-gray-500">
                浏览 Slay the Spire 2 在 Nexus Mods 上的热门、新增、最近更新，以及网页热门分页内容。
              </p>
            </div>
            {nexusSupported && (
              <div className="flex items-center gap-2">
                {hasApiKey && (
                  <button
                    type="button"
                    onClick={handleRefreshCurrentTab}
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

          {nexusSupported && (
            <div className="mb-6 rounded-2xl border border-gray-100 bg-white px-4 py-4 shadow-sm">
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <p className="text-sm font-semibold text-gray-900">通过链接打开 Mod</p>
                  <p className="mt-1 text-xs leading-5 text-gray-500">
                    直接粘贴 Slay the Spire 2 的 Nexus Mod 地址，应用会先跳到详情面板，再继续现有下载安装流程。
                  </p>
                </div>
                {!hasApiKey && (
                  <button
                    type="button"
                    onClick={() => onNavigate?.('settings', { tab: 'nexus' })}
                    className="inline-flex items-center gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 text-xs font-medium text-gray-700 transition-colors hover:bg-gray-50"
                  >
                    <ShieldCheck size={14} />
                    前往设置
                  </button>
                )}
              </div>

              <form onSubmit={handleOpenModLink} className="mt-4 flex flex-col gap-3 lg:flex-row">
                <div className="relative flex-1">
                  <Link2 size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
                  <input
                    type="text"
                    value={linkInput}
                    onChange={(event) => {
                      setLinkInput(event.target.value);
                      if (linkError) {
                        setLinkError('');
                      }
                    }}
                    placeholder="粘贴 https://www.nexusmods.com/slaythespire2/mods/123 或文件页链接"
                    disabled={openingLink}
                    className="w-full rounded-xl border border-gray-200 bg-white py-2.5 pl-10 pr-4 text-sm text-gray-900 outline-none transition-colors focus:border-gray-900 focus:ring-2 focus:ring-gray-900/5 disabled:cursor-not-allowed disabled:bg-gray-50"
                  />
                </div>
                <button
                  type="submit"
                  disabled={openingLink}
                  className="inline-flex items-center justify-center gap-2 rounded-xl bg-gray-900 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-gray-800 disabled:cursor-not-allowed disabled:bg-gray-400"
                >
                  {openingLink ? <Loader2 size={16} className="animate-spin" /> : <Link2 size={16} />}
                  {openingLink ? '正在打开...' : '打开详情'}
                </button>
              </form>

              {linkError && (
                <div className="mt-3 rounded-xl border border-red-100 bg-red-50 px-3 py-2 text-sm text-red-700">
                  {linkError}
                </div>
              )}

              <p className="mt-3 text-xs leading-5 text-gray-400">
                支持 Mod 页面链接和带 `file_id` 的文件页链接；仅接受 Slay the Spire 2 的 Nexus Mods 地址。
              </p>
            </div>
          )}

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
                这个页面的列表与详情通过 Rust 后端请求 Nexus Mods 官方 API 与网页列表接口。先完成 API Key 验证并保存，然后即可浏览热门、最新、最近更新以及分页内容。
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

                <div className="relative min-w-[260px] max-w-md flex-1">
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

              {!isPagedTab && (
                <div className="mb-4 rounded-2xl border border-blue-100 bg-blue-50 px-4 py-4 text-sm text-blue-900 shadow-sm">
                  当前标签使用 Nexus 官方单页列表接口，每次只会返回固定 10 条。需要查看更多内容时，请切换到“最近更新分页”或“网页热门”。
                </div>
              )}

              {isPagedTab && (
                <div className="mb-4 rounded-2xl border border-gray-100 bg-white px-4 py-4 shadow-sm">
                  <div className="flex flex-wrap items-start justify-between gap-4">
                    <div>
                      <p className="text-sm font-semibold text-gray-900">{pagedTitle}</p>
                      <p className="mt-1 text-xs leading-5 text-gray-500">{pagedDescription}</p>
                    </div>

                    <div className="flex rounded-xl bg-gray-100 p-1">
                      {PAGED_PERIOD_OPTIONS.map((option) => (
                        <button
                          key={option.value}
                          type="button"
                          disabled={activeState.loading}
                          onClick={() => handlePagedPeriodChange(option.value)}
                          className={`rounded-lg px-3 py-2 text-sm font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-60 ${
                            activeState.period === option.value
                              ? 'bg-white text-gray-900 shadow-sm'
                              : 'text-gray-500 hover:text-gray-700'
                          }`}
                        >
                          {option.label}
                        </button>
                      ))}
                    </div>
                  </div>

                  <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-gray-100 pt-4">
                    <p className="text-xs text-gray-500">
                      {activeState.totalItems > 0
                        ? `第 ${activeState.page}/${activeState.totalPages} 页 · 显示 ${pageStart}-${pageEnd} / ${activeState.totalItems}`
                        : '当前时间范围暂无可展示条目。'}
                    </p>
                    <div className="flex items-center gap-2">
                      <button
                        type="button"
                        disabled={!activeState.hasPrev || activeState.loading}
                        onClick={() => handlePagedPageChange(activeState.page - 1)}
                        className="inline-flex items-center gap-1 rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        <ChevronLeft size={16} />
                        上一页
                      </button>
                      <button
                        type="button"
                        disabled={!activeState.hasNext || activeState.loading}
                        onClick={() => handlePagedPageChange(activeState.page + 1)}
                        className="inline-flex items-center gap-1 rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60"
                      >
                        下一页
                        <ChevronRight size={16} />
                      </button>
                    </div>
                  </div>
                </div>
              )}

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

              {activeState.warning && (
                <div className="mb-4 rounded-xl border border-amber-100 bg-amber-50 px-4 py-3 text-sm text-amber-800">
                  <div className="flex items-start gap-2">
                    <AlertTriangle size={16} className="mt-0.5 flex-shrink-0" />
                    <span>{activeState.warning}</span>
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
                      onClick={() => handleSelectMod(mod)}
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
          initialFileId={selectedFileId}
          translationEntry={translations[getNexusTranslationKey(selectedMod.modId)]}
          onClose={handleCloseSelectedMod}
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
