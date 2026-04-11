import React, { useDeferredValue, useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  Globe,
  Languages,
  Loader2,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  Star,
} from 'lucide-react';
import NexusModDetail from './NexusModDetail';
import NexusSettings from './NexusSettings';
import {
  formatCompactNumber,
  getNexusTranslationKey,
  loadNexusTranslationsMap,
  translateNexusModFields,
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

function NexusModCard({ mod, translationEntry, translating, onClick, onTranslate }) {
  const [imageError, setImageError] = useState(false);

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
              {translating ? '翻译中' : translationEntry?.name ? '重译' : '翻译'}
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
            {mod.summary || '暂无摘要'}
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

export default function NexusBrowser() {
  const [apiKey, setApiKey] = useState('');
  const [initializing, setInitializing] = useState(true);
  const [showSettings, setShowSettings] = useState(false);
  const [activeTab, setActiveTab] = useState('trending');
  const [tabStates, setTabStates] = useState(createAllTabStates);
  const [search, setSearch] = useState('');
  const [translations, setTranslations] = useState({});
  const [selectedMod, setSelectedMod] = useState(null);
  const [translatingIds, setTranslatingIds] = useState({});
  const [flash, setFlash] = useState(null);

  const deferredSearch = useDeferredValue(search);
  const activeState = tabStates[activeTab];
  const hasApiKey = Boolean(apiKey);

  useEffect(() => {
    if (!flash) {
      return undefined;
    }

    const timer = window.setTimeout(() => setFlash(null), 3000);
    return () => window.clearTimeout(timer);
  }, [flash]);

  const fetchTab = async (tab, options = {}) => {
    const { force = false, keyOverride = '' } = options;
    const effectiveKey = keyOverride || apiKey;

    if (!effectiveKey) {
      return;
    }

    if (!force && tabStates[tab].loading) {
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
      if (message.includes('API Key')) {
        setShowSettings(true);
      }
    }
  };

  useEffect(() => {
    let cancelled = false;

    (async () => {
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
        setShowSettings(!normalizedKey);

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
  }, []);

  const filteredMods = useMemo(() => {
    const keyword = deferredSearch.trim().toLowerCase();
    if (!keyword) {
      return activeState.data;
    }

    return activeState.data.filter((mod) => {
      const translatedName = translations[getNexusTranslationKey(mod.modId)]?.name || '';
      return [mod.name, mod.summary, translatedName]
        .filter(Boolean)
        .some((value) => value.toLowerCase().includes(keyword));
    });
  }, [activeState.data, deferredSearch, translations]);

  const handleTranslateCard = async (mod) => {
    setTranslatingIds((previous) => ({ ...previous, [mod.modId]: true }));

    try {
      const result = await translateNexusModFields({
        modId: mod.modId,
        name: mod.name,
        existing: translations[getNexusTranslationKey(mod.modId)],
        includeDescription: false,
      });

      if (result.translations) {
        setTranslations(result.translations);
      }
      if (result.error) {
        setFlash({ type: 'error', message: result.error });
      }
    } catch (error) {
      setFlash({
        type: 'error',
        message: error?.message || String(error),
      });
    } finally {
      setTranslatingIds((previous) => ({ ...previous, [mod.modId]: false }));
    }
  };

  const handleSettingsSaved = async (savedKey) => {
    setApiKey(savedKey);
    setTabStates(createAllTabStates());
    setShowSettings(false);
    await fetchTab(activeTab, { force: true, keyOverride: savedKey });
  };

  const handleTabChange = async (tab) => {
    setActiveTab(tab);
    if (!hasApiKey) {
      return;
    }
    if (!tabStates[tab].loaded && !tabStates[tab].loading) {
      await fetchTab(tab);
    }
  };

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
                onClick={() => setShowSettings((current) => !current)}
                className="inline-flex items-center gap-2 rounded-xl bg-gray-900 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-gray-800"
              >
                <Settings2 size={16} />
                {showSettings ? '收起 API Key' : 'API Key 设置'}
              </button>
            </div>
          </div>

          {showSettings && (
            <div className="mb-6">
              <NexusSettings initialKey={apiKey} onSaved={handleSettingsSaved} />
            </div>
          )}

          {initializing ? (
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
                  onClick={() => setShowSettings(true)}
                  className="inline-flex items-center gap-2 rounded-xl bg-gray-900 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-gray-800"
                >
                  <ShieldCheck size={16} />
                  配置 API Key
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
              </div>

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
                      translating={Boolean(translatingIds[mod.modId])}
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
          onTranslationsChange={setTranslations}
        />
      )}

      {flash && (
        <div
          className={`fixed bottom-6 right-6 rounded-xl border px-4 py-3 text-sm font-medium shadow-lg ${
            flash.type === 'error'
              ? 'border-red-200 bg-red-50 text-red-700'
              : 'border-emerald-200 bg-emerald-50 text-emerald-700'
          }`}
        >
          {flash.message}
        </div>
      )}
    </div>
  );
}
