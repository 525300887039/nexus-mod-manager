import React, { useEffect, useState } from 'react';
import { Database, Eye, EyeOff, Languages, RefreshCw, Save, Trash2 } from 'lucide-react';

const LEGACY_DEFAULT_SYSTEM_PROMPT = '你是一个游戏MOD翻译助手，请将以下英文翻译成简体中文，保留专有名词不翻译。只返回翻译结果，不要添加任何解释。';
const DEFAULT_SYSTEM_PROMPT = '你是一个游戏MOD翻译助手，请将以下英文翻译成简体中文，保留专有名词不翻译。保持原文的结构、段落、列表、换行和标点风格；如果原始文本存在明显的格式问题，例如换行错乱、段落断裂、列表混乱或空白异常，请在不改变原意的前提下做必要修复；如果原始文本格式正常，则不要额外调整格式。只返回最终翻译结果，不要添加任何解释。';

const DEFAULT_CONFIG = {
  enabled: false,
  apiUrl: '',
  apiKey: '',
  model: '',
  systemPrompt: DEFAULT_SYSTEM_PROMPT,
  engineMode: 'dual',
};

const ENGINE_OPTIONS = [
  {
    value: 'mymemory',
    title: '免费 API',
    subtitle: '只使用 MyMemory，配置最少，适合轻量场景。',
  },
  {
    value: 'llm',
    title: '大模型 API',
    subtitle: '只使用自定义 OpenAI 兼容接口，适合定制翻译质量。',
  },
  {
    value: 'dual',
    title: '双引擎',
    subtitle: '优先 MyMemory，失败后自动 fallback 到大模型。',
  },
];

function normalizeSystemPrompt(systemPrompt) {
  const trimmed = systemPrompt?.trim();
  if (!trimmed || trimmed === LEGACY_DEFAULT_SYSTEM_PROMPT) {
    return DEFAULT_SYSTEM_PROMPT;
  }
  return trimmed;
}

function normalizeConfig(raw = {}) {
  const engineMode = ['mymemory', 'llm', 'dual'].includes(raw.engineMode)
    ? raw.engineMode
    : raw.enabled
      ? 'dual'
      : 'mymemory';

  return {
    ...DEFAULT_CONFIG,
    ...raw,
    engineMode,
    systemPrompt: normalizeSystemPrompt(raw.systemPrompt),
  };
}

function getProviderMeta(provider) {
  switch (provider) {
    case 'cache':
      return { label: 'SQLite 缓存', className: 'bg-gray-100 text-gray-700 border border-gray-200' };
    case 'mymemory':
      return { label: 'MyMemory', className: 'bg-sky-50 text-sky-700 border border-sky-100' };
    case 'llm':
      return { label: '大模型 API', className: 'bg-emerald-50 text-emerald-700 border border-emerald-100' };
    default:
      return { label: provider || '未知来源', className: 'bg-gray-100 text-gray-600 border border-gray-200' };
  }
}

function buildPayload(config) {
  const normalized = normalizeConfig(config);
  return {
    ...normalized,
    enabled: normalized.engineMode !== 'mymemory',
  };
}

function validateConfig(config) {
  const normalized = normalizeConfig(config);

  if (normalized.engineMode === 'mymemory') {
    return '';
  }
  if (!normalized.apiUrl.trim()) {
    return '请输入大模型 API 地址';
  }
  if (!normalized.apiKey.trim()) {
    return '请输入大模型 API Key';
  }
  if (!normalized.model.trim()) {
    return '请输入模型名';
  }

  return '';
}

export default function TranslateSettings({ embedded = false, onShowToast, onConfirm }) {
  const [config, setConfig] = useState(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);
  const [status, setStatus] = useState(null);
  const [cacheCount, setCacheCount] = useState(0);
  const [cacheLoading, setCacheLoading] = useState(true);
  const [cacheError, setCacheError] = useState('');
  const [testText, setTestText] = useState('Hello World');
  const [testResult, setTestResult] = useState(null);

  const includesLlm = config.engineMode !== 'mymemory';

  const refreshCacheCount = async () => {
    setCacheLoading(true);
    try {
      const count = await window.api.getCacheCount();
      setCacheCount(count);
      setCacheError('');
    } catch (error) {
      setCacheError(error?.message || String(error));
    } finally {
      setCacheLoading(false);
    }
  };

  const loadData = async () => {
    setLoading(true);
    try {
      const [loadedConfig, count] = await Promise.all([
        window.api.loadLlmConfig(),
        window.api.getCacheCount(),
      ]);
      setConfig(normalizeConfig(loadedConfig));
      setCacheCount(count);
      setCacheError('');
      setStatus(null);
    } catch (error) {
      setStatus({
        type: 'error',
        message: `加载翻译设置失败: ${error?.message || String(error)}`,
      });
    } finally {
      setLoading(false);
      setCacheLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const persistConfig = async (silent = false) => {
    const validationError = validateConfig(config);
    if (validationError) {
      setStatus({
        type: 'error',
        message: validationError,
      });
      return false;
    }

    setSaving(true);
    try {
      await window.api.saveLlmConfig(buildPayload(config));
      const latest = normalizeConfig(await window.api.loadLlmConfig());
      setConfig(latest);
      setStatus(
        silent
          ? null
          : {
            type: 'success',
            message: '翻译设置已保存到本地配置。',
          },
      );
      if (!silent) {
        onShowToast?.('翻译设置已保存到本地配置。');
      }
      return true;
    } catch (error) {
      setStatus({
        type: 'error',
        message: `保存翻译设置失败: ${error?.message || String(error)}`,
      });
      return false;
    } finally {
      setSaving(false);
    }
  };

  const handleTestTranslate = async () => {
    if (!testText.trim()) {
      setTestResult({
        success: false,
        translated: null,
        error: '请输入测试文本',
        provider: null,
      });
      return;
    }

    const validationError = validateConfig(config);
    if (validationError) {
      setStatus({
        type: 'error',
        message: validationError,
      });
      setTestResult({
        success: false,
        translated: null,
        error: validationError,
        provider: null,
      });
      return;
    }

    setTesting(true);
    setTestResult(null);

    const saved = await persistConfig(true);
    if (!saved) {
      setTesting(false);
      return;
    }

    try {
      const result = await window.api.translateSmart(testText.trim());
      setTestResult(result);
      if (result?.success) {
        await refreshCacheCount();
      }
    } catch (error) {
      setTestResult({
        success: false,
        translated: null,
        error: error?.message || String(error),
        provider: null,
      });
    } finally {
      setTesting(false);
    }
  };

  const handleClearCache = async () => {
    const clearCache = async () => {
      try {
        await window.api.clearTranslationCache();
        await refreshCacheCount();
        setStatus(null);
        onShowToast?.('翻译缓存已清空。');
      } catch (error) {
        setStatus({
          type: 'error',
          message: `清理翻译缓存失败: ${error?.message || String(error)}`,
        });
      }
    };

    if (onConfirm) {
      onConfirm({
        title: '清除翻译缓存',
        message: '确定清空全部翻译缓存吗？此操作不可撤销。',
        danger: true,
        onConfirm: clearCache,
      });
      return;
    }

    const confirmed = window.confirm('确定清空全部翻译缓存吗？此操作不可撤销。');
    if (confirmed) {
      await clearCache();
    }
  };

  const providerMeta = getProviderMeta(testResult?.provider);
  const header = embedded ? null : (
    <div className="mb-6 flex items-end justify-between gap-4">
      <div>
        <p className="text-xs font-semibold uppercase tracking-[0.22em] text-gray-400">Translation Control</p>
        <h1 className="mt-2 text-3xl font-bold text-gray-900">翻译引擎设置</h1>
        <p className="mt-2 max-w-2xl text-sm leading-6 text-gray-500">
          统一管理 SQLite 缓存、MyMemory 免费翻译和自定义大模型翻译链路。所有网络请求都在 Rust 端发起，不受 WebView CSP 限制。
        </p>
      </div>
      <button
        onClick={loadData}
        disabled={loading}
        className="inline-flex items-center gap-2 rounded-xl border border-gray-200 bg-white px-4 py-2.5 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-100 disabled:cursor-not-allowed disabled:opacity-60"
      >
        <RefreshCw size={16} className={loading ? 'animate-spin' : ''} />
        重新加载
      </button>
    </div>
  );
  const embeddedToolbar = embedded ? (
    <div className="mb-4 flex justify-end">
      <button
        onClick={loadData}
        disabled={loading}
        className="inline-flex items-center gap-2 rounded-xl border border-gray-200 bg-white px-4 py-2.5 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-100 disabled:cursor-not-allowed disabled:opacity-60"
      >
        <RefreshCw size={16} className={loading ? 'animate-spin' : ''} />
        重新加载
      </button>
    </div>
  ) : null;
  const content = (
    <>
      {embeddedToolbar}
      {status && (
        <div
          className={`mb-6 rounded-xl border px-4 py-3 text-sm ${
            status.type === 'success'
              ? 'border-emerald-100 bg-emerald-50 text-emerald-700'
              : 'border-red-100 bg-red-50 text-red-700'
          }`}
        >
          {status.message}
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-[1.15fr_0.85fr]">
        <section className="rounded-2xl border border-gray-100 bg-white shadow-sm">
          <div className="border-b border-gray-100 px-6 py-5">
            <div className="flex items-center gap-3">
              <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-gray-900 text-white">
                <Languages size={20} />
              </div>
              <div>
                <h2 className="text-lg font-semibold text-gray-900">引擎策略</h2>
                <p className="mt-1 text-sm text-gray-500">选择翻译优先级，并配置自定义 OpenAI 兼容接口。</p>
              </div>
            </div>
          </div>

          <div className="space-y-6 px-6 py-6">
            <div className="grid gap-3">
              {ENGINE_OPTIONS.map((option) => (
                <label
                  key={option.value}
                  className={`cursor-pointer rounded-xl border p-4 transition-all ${
                    config.engineMode === option.value
                      ? 'border-gray-900 bg-gray-900 text-white shadow-lg shadow-gray-900/10'
                      : 'border-gray-200 bg-white text-gray-900 hover:border-gray-300 hover:bg-gray-50'
                  }`}
                >
                  <div className="flex items-start gap-3">
                    <input
                      type="radio"
                      name="engineMode"
                      value={option.value}
                      checked={config.engineMode === option.value}
                      onChange={(event) => {
                        setConfig((current) => ({
                          ...current,
                          engineMode: event.target.value,
                        }));
                      }}
                      className="mt-1 h-4 w-4 border-gray-300 text-gray-900 focus:ring-gray-900"
                    />
                    <div>
                      <p className="text-sm font-semibold">{option.title}</p>
                      <p className={`mt-1 text-sm leading-6 ${config.engineMode === option.value ? 'text-gray-200' : 'text-gray-500'}`}>
                        {option.subtitle}
                      </p>
                    </div>
                  </div>
                </label>
              ))}
            </div>

            {includesLlm && (
              <div className="rounded-2xl border border-gray-100 bg-gray-50/80 p-5">
                <div className="mb-5">
                  <h3 className="text-sm font-semibold text-gray-900">大模型 API 配置</h3>
                  <p className="mt-1 text-sm text-gray-500">
                    配置会保存到本地 `%APPDATA%/STS2ModManager/llm_config.json`。支持 OpenAI、兼容网关和本地 Ollama。
                  </p>
                </div>

                <div className="space-y-4">
                  <label className="block">
                    <span className="mb-2 block text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">API 地址</span>
                    <input
                      type="text"
                      value={config.apiUrl}
                      onChange={(event) => setConfig((current) => ({ ...current, apiUrl: event.target.value }))}
                      placeholder="https://api.openai.com/v1/chat/completions"
                      className="w-full rounded-xl border border-gray-200 bg-white px-4 py-3 text-sm text-gray-900 outline-none transition-colors placeholder:text-gray-300 focus:border-gray-900 focus:ring-2 focus:ring-gray-900/5"
                    />
                  </label>

                  <label className="block">
                    <span className="mb-2 block text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">API Key</span>
                    <div className="relative">
                      <input
                        type={showApiKey ? 'text' : 'password'}
                        value={config.apiKey}
                        onChange={(event) => setConfig((current) => ({ ...current, apiKey: event.target.value }))}
                        placeholder="sk-..."
                        className="w-full rounded-xl border border-gray-200 bg-white px-4 py-3 pr-12 text-sm text-gray-900 outline-none transition-colors placeholder:text-gray-300 focus:border-gray-900 focus:ring-2 focus:ring-gray-900/5"
                      />
                      <button
                        type="button"
                        onClick={() => setShowApiKey((current) => !current)}
                        className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 transition-colors hover:text-gray-700"
                        aria-label={showApiKey ? '隐藏 API Key' : '显示 API Key'}
                      >
                        {showApiKey ? <EyeOff size={18} /> : <Eye size={18} />}
                      </button>
                    </div>
                  </label>

                  <label className="block">
                    <span className="mb-2 block text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">模型名</span>
                    <input
                      type="text"
                      value={config.model}
                      onChange={(event) => setConfig((current) => ({ ...current, model: event.target.value }))}
                      placeholder="gpt-4o-mini"
                      className="w-full rounded-xl border border-gray-200 bg-white px-4 py-3 text-sm text-gray-900 outline-none transition-colors placeholder:text-gray-300 focus:border-gray-900 focus:ring-2 focus:ring-gray-900/5"
                    />
                  </label>

                  <label className="block">
                    <span className="mb-2 block text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">System Prompt</span>
                    <textarea
                      value={config.systemPrompt}
                      onChange={(event) => setConfig((current) => ({ ...current, systemPrompt: event.target.value }))}
                      rows={5}
                      className="w-full resize-y rounded-xl border border-gray-200 bg-white px-4 py-3 text-sm leading-6 text-gray-900 outline-none transition-colors placeholder:text-gray-300 focus:border-gray-900 focus:ring-2 focus:ring-gray-900/5"
                    />
                  </label>

                </div>
              </div>
            )}

            <div className="flex items-center justify-between gap-3 rounded-xl border border-dashed border-gray-200 bg-gray-50 px-4 py-3">
              <p className="text-sm text-gray-500">
                {config.engineMode === 'dual' && '当前模式会优先使用 MyMemory，失败时再自动切到大模型。'}
                {config.engineMode === 'llm' && '当前模式只会调用你配置的大模型接口。'}
                {config.engineMode === 'mymemory' && '当前模式只会调用 MyMemory 免费翻译接口。'}
              </p>
              <button
                onClick={() => persistConfig(false)}
                disabled={saving}
                className="inline-flex flex-shrink-0 items-center gap-2 rounded-xl bg-gray-900 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:bg-gray-800 disabled:cursor-not-allowed disabled:opacity-60"
              >
                <Save size={15} />
                {saving ? '保存中...' : '保存设置'}
              </button>
            </div>
          </div>
        </section>

        <div className="space-y-6">
          <section className="rounded-2xl border border-gray-100 bg-white shadow-sm">
            <div className="border-b border-gray-100 px-6 py-5">
              <div className="flex items-center gap-3">
                <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-emerald-500 text-white shadow-lg shadow-emerald-500/20">
                  <Languages size={20} />
                </div>
                <div>
                  <h2 className="text-lg font-semibold text-gray-900">测试翻译</h2>
                  <p className="mt-1 text-sm text-gray-500">会先保存当前表单，再调用 `translate_smart`，方便验证真实链路和来源。</p>
                </div>
              </div>
            </div>

            <div className="space-y-4 px-6 py-6">
              <textarea
                value={testText}
                onChange={(event) => setTestText(event.target.value)}
                rows={4}
                placeholder="输入一段英文进行测试"
                className="w-full resize-y rounded-xl border border-gray-200 bg-gray-50 px-4 py-3 text-sm leading-6 text-gray-900 outline-none transition-colors placeholder:text-gray-300 focus:border-gray-900 focus:bg-white focus:ring-2 focus:ring-gray-900/5"
              />

              <button
                onClick={handleTestTranslate}
                disabled={testing || saving}
                className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-emerald-500 px-4 py-3 text-sm font-semibold text-white transition-colors hover:bg-emerald-400 disabled:cursor-not-allowed disabled:opacity-60"
              >
                <Languages size={16} />
                {testing ? '测试中...' : '测试'}
              </button>

              <div className="rounded-2xl border border-gray-100 bg-gray-50/80 p-4">
                <div className="mb-3 flex items-center justify-between gap-3">
                  <p className="text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">结果</p>
                  {testResult?.provider && (
                    <span className={`inline-flex items-center rounded-full px-2.5 py-1 text-xs font-medium ${providerMeta.className}`}>
                      {providerMeta.label}
                    </span>
                  )}
                </div>

                {!testResult && (
                  <p className="text-sm leading-6 text-gray-400">还没有测试结果。建议先用 “Hello World” 验证缓存与引擎切换。</p>
                )}

                {testResult?.success && (
                  <div className="space-y-3">
                    <div className="rounded-xl border border-emerald-100 bg-white px-4 py-3">
                      <p className="text-xs font-semibold uppercase tracking-[0.18em] text-emerald-500">Translated</p>
                      <p className="mt-2 text-sm leading-7 text-gray-800">{testResult.translated}</p>
                    </div>
                  </div>
                )}

                {testResult && !testResult.success && (
                  <div className="rounded-xl border border-red-100 bg-white px-4 py-3 text-sm leading-6 text-red-600">
                    {testResult.error || '翻译失败'}
                  </div>
                )}
              </div>
            </div>
          </section>

          <section className="rounded-2xl border border-gray-100 bg-white shadow-sm">
            <div className="border-b border-gray-100 px-6 py-5">
              <div className="flex items-center gap-3">
                <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-gray-100 text-gray-700">
                  <Database size={20} />
                </div>
                <div>
                  <h2 className="text-lg font-semibold text-gray-900">缓存统计</h2>
                  <p className="mt-1 text-sm text-gray-500">已缓存文本会优先从 SQLite 直接返回，适合重复翻译 MOD 名称和描述。</p>
                </div>
              </div>
            </div>

            <div className="space-y-4 px-6 py-6">
              <div className="rounded-2xl bg-gray-900 px-5 py-5 text-white">
                <p className="text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">Cached Entries</p>
                <p className="mt-3 text-4xl font-bold">
                  {cacheLoading ? '...' : cacheCount}
                </p>
                {cacheError && <p className="mt-3 text-sm text-red-300">{cacheError}</p>}
              </div>

              <div className="flex gap-3">
                <button
                  onClick={refreshCacheCount}
                  disabled={cacheLoading}
                  className="inline-flex flex-1 items-center justify-center gap-2 rounded-xl border border-gray-200 bg-white px-4 py-3 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-100 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  <RefreshCw size={16} className={cacheLoading ? 'animate-spin' : ''} />
                  刷新统计
                </button>
                <button
                  onClick={handleClearCache}
                  className="inline-flex flex-1 items-center justify-center gap-2 rounded-xl bg-red-50 px-4 py-3 text-sm font-medium text-red-600 transition-colors hover:bg-red-100"
                >
                  <Trash2 size={16} />
                  清除缓存
                </button>
              </div>
            </div>
          </section>
        </div>
      </div>
    </>
  );

  return (
    embedded ? (
      <div>{content}</div>
    ) : (
      <div className="flex-1 overflow-y-auto bg-gray-50">
        <div className="mx-auto max-w-6xl px-6 py-8">
          {header}
          {content}
        </div>
      </div>
    )
  );
}
