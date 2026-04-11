export const NEXUS_TRANSLATION_PREFIX = 'nexus:';
const REQUIRED_NEXUS_API_METHODS = [
  'getNexusKey',
  'saveNexusKey',
  'nexusValidateKey',
  'nexusGetTrending',
  'nexusGetLatestAdded',
  'nexusGetLatestUpdated',
  'nexusGetMod',
  'nexusGetModFiles',
  'translateSmart',
  'loadNexusTranslations',
  'saveNexusTranslations',
];

export function getNexusTranslationKey(modId) {
  return `${NEXUS_TRANSLATION_PREFIX}${modId}`;
}

export function hasNexusBrowserSupport() {
  const api = window.api;
  if (!api || typeof api !== 'object') {
    return false;
  }

  return REQUIRED_NEXUS_API_METHODS.every((method) => typeof api[method] === 'function');
}

export function isChineseText(text) {
  if (!text) {
    return false;
  }
  return /[\u4e00-\u9fff]/.test(text);
}

export function formatCompactNumber(value) {
  return new Intl.NumberFormat('zh-CN').format(value || 0);
}

export function formatBytes(value) {
  if (!value && value !== 0) {
    return '未知';
  }
  if (value < 1024) {
    return `${value} B`;
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }
  if (value < 1024 * 1024 * 1024) {
    return `${(value / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export function formatUnixDateTime(timestamp) {
  if (!timestamp) {
    return '未知';
  }
  return new Date(timestamp * 1000).toLocaleString('zh-CN', {
    year: 'numeric',
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function getMembershipLabel(result) {
  if (result?.isPremium) {
    return 'Premium';
  }
  if (result?.isSupporter) {
    return 'Supporter';
  }
  return 'Regular';
}

export async function loadNexusTranslationsMap() {
  if (!hasNexusBrowserSupport()) {
    return {};
  }

  try {
    const data = await window.api.loadNexusTranslations();
    return data && typeof data === 'object' ? data : {};
  } catch (_error) {
    return {};
  }
}

export async function saveNexusTranslationsMap(data) {
  if (!hasNexusBrowserSupport()) {
    throw new Error('Nexus 翻译存储未启用');
  }

  const result = await window.api.saveNexusTranslations(data);
  if (result?.success === false) {
    throw new Error(result.error || '保存 Nexus 翻译失败');
  }
  return result;
}

export async function translateNexusModFields({
  modId,
  name,
  description,
  existing = {},
  includeDescription = false,
}) {
  const jobs = [];

  if (name && !isChineseText(name)) {
    jobs.push(
      window.api.translateSmart(name).then((result) => ({
        type: 'name',
        result,
      })),
    );
  }

  if (includeDescription && description && !isChineseText(description)) {
    jobs.push(
      window.api.translateSmart(description).then((result) => ({
        type: 'desc',
        result,
      })),
    );
  }

  if (jobs.length === 0) {
    return {
      success: false,
      updates: {},
      translations: null,
      error: null,
    };
  }

  const responses = await Promise.all(jobs);
  const updates = {};
  const errors = [];

  responses.forEach(({ type, result }) => {
    if (result?.success && result.translated) {
      updates[type] = result.translated;
    } else if (result?.error) {
      errors.push(result.error);
    }
  });

  if (Object.keys(updates).length === 0) {
    return {
      success: false,
      updates: {},
      translations: null,
      error: errors[0] || '翻译失败',
    };
  }

  const translationKey = getNexusTranslationKey(modId);
  const translationMap = await loadNexusTranslationsMap();
  translationMap[translationKey] = {
    ...(translationMap[translationKey] || existing || {}),
    ...updates,
  };
  await saveNexusTranslationsMap(translationMap);

  return {
    success: true,
    updates,
    translationKey,
    translations: translationMap,
    error: errors[0] || null,
  };
}
