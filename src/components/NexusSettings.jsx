import React, { useEffect, useState } from 'react';
import { AlertTriangle, CheckCircle2, Eye, EyeOff, ExternalLink, KeyRound, Loader2, Save, ShieldCheck } from 'lucide-react';
import { getMembershipLabel } from './nexusShared';

export default function NexusSettings({ initialKey = '', onSaved, compact = false }) {
  const [key, setKey] = useState(initialKey || '');
  const [showKey, setShowKey] = useState(false);
  const [validating, setValidating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [validatedKey, setValidatedKey] = useState('');
  const [validationResult, setValidationResult] = useState(null);
  const [validationError, setValidationError] = useState('');
  const [status, setStatus] = useState(null);

  useEffect(() => {
    setKey(initialKey || '');
    setValidatedKey('');
    setValidationResult(null);
    setValidationError('');
    setStatus(null);
  }, [initialKey]);

  const normalizedKey = key.trim();
  const canSave = Boolean(validationResult) && validatedKey === normalizedKey;

  const handleKeyChange = (event) => {
    const nextValue = event.target.value;
    setKey(nextValue);
    if (nextValue.trim() !== validatedKey) {
      setValidationResult(null);
      setValidationError('');
    }
    setStatus(null);
  };

  const handleValidate = async () => {
    if (!normalizedKey) {
      setValidationError('请输入 API Key');
      setValidationResult(null);
      return;
    }

    setValidating(true);
    setValidationError('');
    setStatus(null);

    try {
      const result = await window.api.nexusValidateKey(normalizedKey);
      setValidationResult(result);
      setValidatedKey(normalizedKey);
    } catch (error) {
      setValidationResult(null);
      setValidatedKey('');
      setValidationError(error?.message || String(error));
    } finally {
      setValidating(false);
    }
  };

  const handleSave = async () => {
    if (!canSave) {
      return;
    }

    setSaving(true);
    setStatus(null);

    try {
      await window.api.saveNexusKey(normalizedKey);
      setStatus({ type: 'success', message: 'Nexus Mods API Key 已保存。' });
      if (onSaved) {
        onSaved(normalizedKey, validationResult);
      }
    } catch (error) {
      setStatus({
        type: 'error',
        message: error?.message || String(error),
      });
    } finally {
      setSaving(false);
    }
  };

  return (
    <section className={`rounded-2xl border border-gray-100 bg-white shadow-sm ${compact ? '' : 'overflow-hidden'}`}>
      <div className={`border-b border-gray-100 ${compact ? 'px-5 py-4' : 'px-6 py-5'}`}>
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-start gap-3">
            <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-gray-900 text-white">
              <KeyRound size={20} />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-gray-900">Nexus Mods API Key</h2>
              <p className="mt-1 text-sm text-gray-500">
                前往 `nexusmods.com/users/myaccount?tab=api` 获取你的 API Key。
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => window.api.openUrl('https://www.nexusmods.com/users/myaccount?tab=api')}
            className="inline-flex items-center gap-1 rounded-lg border border-gray-200 px-3 py-2 text-xs font-medium text-gray-600 transition-colors hover:bg-gray-50 hover:text-gray-900"
          >
            <ExternalLink size={14} />
            打开页面
          </button>
        </div>
      </div>

      <div className={compact ? 'space-y-4 px-5 py-4' : 'space-y-5 px-6 py-6'}>
        {status && (
          <div
            className={`rounded-xl border px-4 py-3 text-sm ${
              status.type === 'success'
                ? 'border-emerald-100 bg-emerald-50 text-emerald-700'
                : 'border-red-100 bg-red-50 text-red-700'
            }`}
          >
            {status.message}
          </div>
        )}

        <div>
          <label className="mb-2 block text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">
            API Key
          </label>
          <div className="relative">
            <input
              type={showKey ? 'text' : 'password'}
              value={key}
              onChange={handleKeyChange}
              placeholder="输入你的 Nexus Mods API Key"
              className="w-full rounded-xl border border-gray-200 bg-white px-4 py-3 pr-12 text-sm text-gray-900 outline-none transition-colors placeholder:text-gray-300 focus:border-gray-900 focus:ring-2 focus:ring-gray-900/5"
            />
            <button
              type="button"
              onClick={() => setShowKey((current) => !current)}
              className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 transition-colors hover:text-gray-700"
              aria-label={showKey ? '隐藏 API Key' : '显示 API Key'}
            >
              {showKey ? <EyeOff size={18} /> : <Eye size={18} />}
            </button>
          </div>
        </div>

        <div className="flex flex-wrap gap-3">
          <button
            type="button"
            onClick={handleValidate}
            disabled={validating || !normalizedKey}
            className="inline-flex items-center gap-2 rounded-xl border border-gray-200 bg-white px-4 py-3 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {validating ? <Loader2 size={16} className="animate-spin" /> : <ShieldCheck size={16} />}
            {validating ? '验证中...' : '验证'}
          </button>
          <button
            type="button"
            onClick={handleSave}
            disabled={saving || !canSave}
            className="inline-flex items-center gap-2 rounded-xl bg-gray-900 px-4 py-3 text-sm font-medium text-white transition-colors hover:bg-gray-800 disabled:cursor-not-allowed disabled:opacity-60"
          >
            {saving ? <Loader2 size={16} className="animate-spin" /> : <Save size={16} />}
            {saving ? '保存中...' : '保存'}
          </button>
          {!canSave && normalizedKey && (
            <p className="self-center text-xs text-gray-400">需要先验证成功，才可保存当前 API Key。</p>
          )}
        </div>

        {validationError && (
          <div className="rounded-xl border border-red-100 bg-red-50 px-4 py-3 text-sm text-red-700">
            <div className="flex items-center gap-2">
              <AlertTriangle size={16} />
              <span>{validationError}</span>
            </div>
          </div>
        )}

        {validationResult && (
          <div className="rounded-2xl border border-emerald-100 bg-emerald-50/70 px-5 py-4">
            <div className="flex items-start justify-between gap-4">
              <div>
                <div className="flex items-center gap-2 text-emerald-700">
                  <CheckCircle2 size={18} />
                  <p className="text-sm font-semibold">验证成功</p>
                </div>
                <p className="mt-3 text-lg font-semibold text-gray-900">{validationResult.name}</p>
                <p className="mt-1 text-sm text-gray-500">{validationResult.email || '未返回邮箱信息'}</p>
              </div>
              <span className="rounded-full bg-white px-3 py-1 text-xs font-semibold text-emerald-700 shadow-sm">
                {getMembershipLabel(validationResult)}
              </span>
            </div>
            <div className="mt-4 flex flex-wrap gap-3 text-xs text-gray-500">
              <span>用户 ID: {validationResult.userId}</span>
              {validationResult.profileUrl && (
                <button
                  type="button"
                  onClick={() => window.api.openUrl(validationResult.profileUrl)}
                  className="inline-flex items-center gap-1 text-blue-600 transition-colors hover:text-blue-800"
                >
                  <ExternalLink size={13} />
                  打开个人主页
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
