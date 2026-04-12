import React, { useEffect, useMemo, useState } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  FileArchive,
  Globe,
  Languages,
  Loader2,
  Package,
  Star,
  X,
} from 'lucide-react';
import {
  extractPlainTextFromNexusRichText,
  formatBytes,
  formatCompactNumber,
  formatUnixDateTime,
  translateNexusModFields,
} from './nexusShared';
import NexusRichText from './NexusRichText';

const CATEGORY_ORDER = ['MAIN', 'UPDATE', 'OPTIONAL'];

function groupFilesByCategory(files) {
  const groups = files.reduce((accumulator, file) => {
    const category = (file.categoryName || '其他').toUpperCase();
    const normalized = CATEGORY_ORDER.includes(category) ? category : '其他';
    if (!accumulator[normalized]) {
      accumulator[normalized] = [];
    }
    accumulator[normalized].push(file);
    return accumulator;
  }, {});

  return [...CATEGORY_ORDER, '其他']
    .filter((category) => groups[category]?.length)
    .map((category) => [category, groups[category]]);
}

function canAutoInstallFile(file) {
  const name = file?.fileName || file?.name || '';
  return name.toLowerCase().endsWith('.zip');
}

export default function NexusModDetail({
  mod,
  translationEntry,
  onClose,
  onTranslationsChange,
  onRefreshMods,
  onShowToast,
  onNexusDownloadStatusChange,
}) {
  const [detail, setDetail] = useState(mod);
  const [files, setFiles] = useState([]);
  const [loading, setLoading] = useState(false);
  const [detailError, setDetailError] = useState('');
  const [filesError, setFilesError] = useState('');
  const [translating, setTranslating] = useState(false);
  const [translateError, setTranslateError] = useState('');
  const [imageError, setImageError] = useState(false);
  const [openingDownload, setOpeningDownload] = useState(false);
  const [manualInstalling, setManualInstalling] = useState(false);

  useEffect(() => {
    setImageError(false);
  }, [mod.modId, mod.pictureUrl]);

  useEffect(() => {
    let cancelled = false;

    setDetail(mod);
    setFiles([]);
    setDetailError('');
    setFilesError('');
    setTranslateError('');
    setLoading(true);

    (async () => {
      const [detailResult, filesResult] = await Promise.allSettled([
        window.api.nexusGetMod(mod.modId),
        window.api.nexusGetModFiles(mod.modId),
      ]);

      if (cancelled) {
        return;
      }

      if (detailResult.status === 'fulfilled') {
        setDetail(detailResult.value);
      } else {
        setDetailError(detailResult.reason?.message || String(detailResult.reason));
      }

      if (filesResult.status === 'fulfilled') {
        setFiles(filesResult.value || []);
      } else {
        setFilesError(filesResult.reason?.message || String(filesResult.reason));
      }

      setLoading(false);
    })();

    return () => {
      cancelled = true;
    };
  }, [mod]);

  const currentMod = { ...mod, ...detail };
  const detailContent = currentMod.description || currentMod.summary || '';
  const translatedName = translationEntry?.name || '';
  const translatedDescription = useMemo(
    () => extractPlainTextFromNexusRichText(
      translationEntry?.desc || (!currentMod.description ? translationEntry?.summary || '' : ''),
    ),
    [currentMod.description, translationEntry],
  );
  const fileGroups = useMemo(() => groupFilesByCategory(files), [files]);
  const preferredFile = useMemo(
    () => files.find((file) => (file.categoryName || '').toUpperCase() === 'MAIN') || files[0] || null,
    [files],
  );
  const autoDownloadSupported = typeof window.api?.openNexusDownload === 'function';
  const preferredFileCanAutoInstall = canAutoInstallFile(preferredFile);

  const handleTranslate = async () => {
    setTranslating(true);
    setTranslateError('');

    try {
      const result = await translateNexusModFields({
        modId: currentMod.modId,
        name: currentMod.name,
        description: detailContent,
        existing: translationEntry,
        includeDescription: Boolean(detailContent),
      });

      if (result.translations && onTranslationsChange) {
        onTranslationsChange(result.translations);
      }
      if (result.error) {
        setTranslateError(result.error);
      }
    } catch (error) {
      setTranslateError(error?.message || String(error));
    } finally {
      setTranslating(false);
    }
  };

  const handleOpenDownload = async (file = preferredFile) => {
    const fileName = file?.fileName || file?.name || currentMod.name || 'Nexus Mod';
    const canAutoInstall = canAutoInstallFile(file);

    if (!autoDownloadSupported) {
      const message = '当前运行环境不支持内嵌 Nexus 下载窗口';
      onNexusDownloadStatusChange?.({
        phase: 'error',
        message,
        fileName,
      });
      onShowToast?.(message, 'error');
      return;
    }

    setOpeningDownload(true);
    onNexusDownloadStatusChange?.({
      phase: 'preparing',
      message: canAutoInstall
        ? `正在打开 ${fileName} 的下载页...`
        : `正在打开 ${fileName} 的下载页（该文件仅下载，不会自动安装）...`,
      fileName,
    });

    try {
      await window.api.openNexusDownload(currentMod.modId, file?.fileId ?? null);
    } catch (error) {
      const message = `打开下载窗口失败: ${error?.message || String(error)}`;
      onNexusDownloadStatusChange?.({
        phase: 'error',
        message,
        fileName,
      });
      onShowToast?.(message, 'error');
    } finally {
      setOpeningDownload(false);
    }
  };

  const handleManualInstall = async () => {
    setManualInstalling(true);

    try {
      const result = await window.api.installMod();
      if (result.success) {
        const installedNames = Array.isArray(result.installed) && result.installed.length > 0
          ? result.installed.join(', ')
          : '手动安装已完成';
        onShowToast?.(`已安装: ${installedNames}`);
        if (onRefreshMods) {
          await onRefreshMods();
        }
      } else if (result.error && result.error !== 'Cancelled') {
        onShowToast?.(result.error, 'error');
      }
    } catch (error) {
      onShowToast?.(error?.message || String(error), 'error');
    } finally {
      setManualInstalling(false);
    }
  };

  return (
    <aside className="w-[420px] bg-white border-l border-gray-100 flex flex-col overflow-hidden">
      <div className="flex items-center justify-between px-5 py-4 border-b border-gray-100">
        <div className="min-w-0 pr-3">
          <h2 className="text-lg font-semibold text-gray-900 truncate">
            {translatedName || currentMod.name}
          </h2>
          {translatedName && (
            <p className="mt-1 text-xs text-gray-400 truncate">{currentMod.name}</p>
          )}
        </div>
        <button
          type="button"
          onClick={onClose}
          className="rounded-lg p-2 text-gray-400 transition-colors hover:bg-gray-50 hover:text-gray-700"
        >
          <X size={18} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto px-5 py-5 space-y-5">
        <div className="overflow-hidden rounded-2xl border border-gray-100 bg-gray-100">
          {currentMod.pictureUrl && !imageError ? (
            <img
              src={currentMod.pictureUrl}
              alt={currentMod.name}
              onError={() => setImageError(true)}
              className="h-52 w-full object-cover"
            />
          ) : (
            <div className="flex h-52 items-center justify-center bg-gray-200 text-gray-400">
              <Package size={36} />
            </div>
          )}
        </div>

        <div className="flex flex-wrap gap-2">
          <span className="inline-flex items-center gap-1 rounded-full bg-sky-50 px-3 py-1 text-xs font-medium text-sky-700">
            <Download size={13} />
            下载 {formatCompactNumber(currentMod.modDownloads)}
          </span>
          <span className="inline-flex items-center gap-1 rounded-full bg-amber-50 px-3 py-1 text-xs font-medium text-amber-700">
            <Star size={13} />
            Endorsements {formatCompactNumber(currentMod.endorsementCount)}
          </span>
          <span className={`inline-flex items-center gap-1 rounded-full px-3 py-1 text-xs font-medium ${
            currentMod.available
              ? 'bg-emerald-50 text-emerald-700'
              : 'bg-gray-100 text-gray-600'
          }`}>
            <CheckCircle2 size={13} />
            {currentMod.available ? '可用' : '不可用'}
          </span>
        </div>

        <div className="grid grid-cols-2 gap-3">
          <div className="rounded-xl border border-gray-100 bg-gray-50 px-4 py-3">
            <p className="text-[11px] uppercase tracking-[0.18em] text-gray-400">作者</p>
            <p className="mt-2 text-sm font-medium text-gray-800">{currentMod.author || currentMod.uploadedBy || '未知'}</p>
          </div>
          <div className="rounded-xl border border-gray-100 bg-gray-50 px-4 py-3">
            <p className="text-[11px] uppercase tracking-[0.18em] text-gray-400">版本</p>
            <p className="mt-2 text-sm font-medium text-gray-800">{currentMod.version || '未知'}</p>
          </div>
        </div>

        <div className="space-y-3 rounded-2xl border border-gray-100 bg-white px-4 py-4">
          {[
            ['上传者', currentMod.uploadedBy || currentMod.author || '未知'],
            ['唯一下载', formatCompactNumber(currentMod.modUniqueDownloads)],
            ['创建时间', formatUnixDateTime(currentMod.createdTimestamp)],
            ['更新时间', formatUnixDateTime(currentMod.updatedTimestamp)],
            ['状态', currentMod.status || '未知'],
          ].map(([label, value]) => (
            <div key={label} className="flex items-center justify-between gap-4 text-sm">
              <span className="text-gray-400">{label}</span>
              <span className="text-right font-medium text-gray-700">{value}</span>
            </div>
          ))}
        </div>

        <div>
          <div className="mb-2 flex items-center justify-between gap-3">
            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">描述</p>
            <button
              type="button"
              onClick={handleTranslate}
              disabled={translating}
              className="inline-flex items-center gap-1 text-xs font-medium text-blue-600 transition-colors hover:text-blue-800 disabled:cursor-not-allowed disabled:text-gray-300"
            >
              {translating ? <Loader2 size={14} className="animate-spin" /> : <Languages size={14} />}
              {translating ? '翻译中...' : translatedDescription ? '重新翻译' : '翻译名称和描述'}
            </button>
          </div>
          <div className="rounded-2xl border border-gray-100 bg-gray-50 px-4 py-4">
            {translatedDescription ? (
              <p className="whitespace-pre-wrap break-words text-sm leading-7 text-gray-700">
                {translatedDescription}
              </p>
            ) : (
              <NexusRichText
                content={detailContent}
                className="text-sm leading-7 text-gray-700"
                fallbackClassName="whitespace-pre-wrap break-words text-sm leading-7 text-gray-700"
                emptyClassName="text-sm leading-7 text-gray-400"
              />
            )}
            {translatedDescription && detailContent && (
              <div className="mt-4 border-t border-gray-200 pt-4">
                <NexusRichText
                  content={detailContent}
                  className="text-xs leading-6 text-gray-500"
                  fallbackClassName="whitespace-pre-wrap break-words text-xs leading-6 text-gray-500"
                  emptyText=""
                />
              </div>
            )}
            {translateError && (
              <p className="mt-3 text-xs text-red-500">翻译失败: {translateError}</p>
            )}
          </div>
        </div>

        {(detailError || filesError) && (
          <div className="rounded-xl border border-amber-100 bg-amber-50 px-4 py-3 text-sm text-amber-700">
            <div className="flex items-start gap-2">
              <AlertTriangle size={16} className="mt-0.5 flex-shrink-0" />
              <div className="space-y-1">
                {detailError && <p>详情加载失败: {detailError}</p>}
                {filesError && <p>文件列表加载失败: {filesError}</p>}
              </div>
            </div>
          </div>
        )}

        <div>
          <div className="mb-2 flex items-center justify-between">
            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">文件列表</p>
            {loading && (
              <span className="inline-flex items-center gap-1 text-xs text-gray-400">
                <Loader2 size={13} className="animate-spin" />
                加载中...
              </span>
            )}
          </div>

          {fileGroups.length === 0 && !loading ? (
            <div className="rounded-2xl border border-dashed border-gray-200 px-4 py-6 text-center text-sm text-gray-400">
              暂无文件信息
            </div>
          ) : (
            <div className="space-y-4">
              {fileGroups.map(([category, categoryFiles]) => (
                <section key={category} className="rounded-2xl border border-gray-100 bg-white px-4 py-4">
                  <p className="text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">{category}</p>
                  <div className="mt-3 space-y-3">
                    {categoryFiles.map((file) => (
                      <div key={file.fileId} className="rounded-xl bg-gray-50 px-4 py-3">
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0">
                            <p className="truncate text-sm font-medium text-gray-900">{file.name || file.fileName}</p>
                            <p className="mt-1 text-xs text-gray-500">{file.version || '未知版本'}</p>
                          </div>
                          <FileArchive size={16} className="mt-1 flex-shrink-0 text-gray-300" />
                        </div>
                        <div className="mt-3 flex flex-wrap gap-3 text-xs text-gray-500">
                          <span>文件名: {file.fileName}</span>
                          <span>大小: {formatBytes(file.sizeInBytes)}</span>
                          <span>上传: {formatUnixDateTime(file.uploadedTimestamp)}</span>
                        </div>
                        {file.description && (
                          <NexusRichText
                            content={file.description}
                            className="mt-3 text-xs leading-6 text-gray-500"
                            fallbackClassName="mt-3 whitespace-pre-wrap break-words text-xs leading-6 text-gray-500"
                            emptyText=""
                          />
                        )}
                        <div className="mt-4 flex justify-end">
                          <button
                            type="button"
                            onClick={() => handleOpenDownload(file)}
                            disabled={openingDownload || !autoDownloadSupported}
                            className="inline-flex items-center gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 text-xs font-medium text-gray-700 transition-colors hover:bg-gray-100 disabled:cursor-not-allowed disabled:bg-gray-100 disabled:text-gray-400"
                          >
                            {openingDownload ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}
                            {canAutoInstallFile(file) ? '下载安装' : '仅下载'}
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                </section>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="space-y-2 border-t border-gray-100 p-4">
        <button
          type="button"
          onClick={() => window.api.openUrl(`https://www.nexusmods.com/slaythespire2/mods/${currentMod.modId}`)}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-gray-900 px-4 py-3 text-sm font-semibold text-white transition-colors hover:bg-gray-800"
        >
          <Globe size={16} />
          在 Nexus 打开
        </button>
        <button
          type="button"
          onClick={() => handleOpenDownload()}
          disabled={openingDownload || !autoDownloadSupported}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl border border-gray-200 bg-white px-4 py-3 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 disabled:cursor-not-allowed disabled:bg-gray-50 disabled:text-gray-400"
        >
          {openingDownload ? <Loader2 size={16} className="animate-spin" /> : <Download size={16} />}
          {openingDownload
            ? '正在打开下载页...'
            : preferredFileCanAutoInstall ? '下载安装' : '下载文件'}
        </button>
        <button
          type="button"
          onClick={handleManualInstall}
          disabled={manualInstalling}
          className="inline-flex w-full items-center justify-center gap-2 rounded-xl border border-gray-200 bg-gray-50 px-4 py-3 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-100 disabled:cursor-not-allowed disabled:text-gray-400"
        >
          {manualInstalling ? <Loader2 size={16} className="animate-spin" /> : <FileArchive size={16} />}
          {manualInstalling ? '正在选择文件...' : '手动安装'}
        </button>
        <p className="text-xs leading-5 text-gray-400">
          ZIP 文件会在下载完成后自动安装；如果文件是 .rar / .7z 或自动下载不生效，请先在浏览器下载，再点击“手动安装”选择已下载的压缩包。
        </p>
      </div>
    </aside>
  );
}
