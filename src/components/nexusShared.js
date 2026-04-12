export const NEXUS_TRANSLATION_PREFIX = 'nexus:';
const REQUIRED_NEXUS_API_METHODS = [
  'getNexusKey',
  'saveNexusKey',
  'nexusValidateKey',
  'nexusGetTrending',
  'nexusGetLatestAdded',
  'nexusGetLatestUpdated',
  'nexusGetRecentlyUpdatedPage',
  'nexusGetPopularPage',
  'nexusGetMod',
  'nexusGetModFiles',
  'translateSmart',
  'loadNexusTranslations',
  'saveNexusTranslations',
];
const HTML_BREAK_RE = /<br\s*\/?>/gi;
const HTML_PARAGRAPH_OPEN_RE = /<p\b[^>]*>/gi;
const HTML_PARAGRAPH_CLOSE_RE = /<\/p>/gi;
const HTML_DIV_OPEN_RE = /<div\b[^>]*>/gi;
const HTML_DIV_CLOSE_RE = /<\/div>/gi;
const HTML_LIST_OPEN_RE = /<(ul|ol)\b[^>]*>/gi;
const HTML_LIST_CLOSE_RE = /<\/(ul|ol)>/gi;
const HTML_LIST_ITEM_OPEN_RE = /<li\b[^>]*>/gi;
const HTML_LIST_ITEM_CLOSE_RE = /<\/li>/gi;
const HTML_BOLD_OPEN_RE = /<(strong|b)\b[^>]*>/gi;
const HTML_BOLD_CLOSE_RE = /<\/(strong|b)>/gi;
const HTML_ITALIC_OPEN_RE = /<(em|i)\b[^>]*>/gi;
const HTML_ITALIC_CLOSE_RE = /<\/(em|i)>/gi;
const HTML_UNDERLINE_OPEN_RE = /<u\b[^>]*>/gi;
const HTML_UNDERLINE_CLOSE_RE = /<\/u>/gi;
const HTML_ANCHOR_OPEN_RE = /<a\b[^>]*href\s*=\s*(['"]?)([^'" >]+)\1[^>]*>/gi;
const HTML_ANCHOR_CLOSE_RE = /<\/a>/gi;
const HTML_TAG_RE = /<[^>]+>/g;
const HTML_ENTITY_RE = /&(#x?[0-9a-f]+|[a-z]+);/gi;
const DISCARDABLE_BBCODE_RE = /^\/?[a-z*][a-z0-9-]*(?:=[^\]]*)?$/i;
const SUPPORTED_BBCODE_TAGS = new Set(['b', 'i', 'u', 'url', 'list', 'size']);
const HTML_ENTITY_MAP = {
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  apos: '\'',
  nbsp: ' ',
};

let nexusTranslationsSnapshot = null;
let nexusTranslationsLoadPromise = null;
let nexusTranslationsWriteQueue = Promise.resolve(null);

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

function decodeHtmlEntities(text) {
  return String(text || '').replace(HTML_ENTITY_RE, (entity, value) => {
    const normalized = value.toLowerCase();
    if (normalized in HTML_ENTITY_MAP) {
      return HTML_ENTITY_MAP[normalized];
    }

    if (normalized.startsWith('#x')) {
      const codePoint = Number.parseInt(normalized.slice(2), 16);
      return Number.isFinite(codePoint) ? String.fromCodePoint(codePoint) : entity;
    }

    if (normalized.startsWith('#')) {
      const codePoint = Number.parseInt(normalized.slice(1), 10);
      return Number.isFinite(codePoint) ? String.fromCodePoint(codePoint) : entity;
    }

    return entity;
  });
}

export function sanitizeExternalUrl(rawUrl) {
  const value = decodeHtmlEntities(String(rawUrl || '').trim());
  if (!value) {
    return null;
  }

  try {
    const parsed = new URL(value);
    if (parsed.protocol === 'http:' || parsed.protocol === 'https:') {
      return parsed.toString();
    }
  } catch (_error) {
    return null;
  }

  return null;
}

export function normalizeNexusRichText(raw) {
  if (!raw) {
    return '';
  }

  return decodeHtmlEntities(
    String(raw)
      .replace(/\r\n?/g, '\n')
      .replace(HTML_BREAK_RE, '\n')
      .replace(HTML_PARAGRAPH_OPEN_RE, '')
      .replace(HTML_PARAGRAPH_CLOSE_RE, '\n\n')
      .replace(HTML_DIV_OPEN_RE, '')
      .replace(HTML_DIV_CLOSE_RE, '\n')
      .replace(HTML_LIST_OPEN_RE, '[list]')
      .replace(HTML_LIST_CLOSE_RE, '[/list]')
      .replace(HTML_LIST_ITEM_OPEN_RE, '[*]')
      .replace(HTML_LIST_ITEM_CLOSE_RE, '\n')
      .replace(HTML_BOLD_OPEN_RE, '[b]')
      .replace(HTML_BOLD_CLOSE_RE, '[/b]')
      .replace(HTML_ITALIC_OPEN_RE, '[i]')
      .replace(HTML_ITALIC_CLOSE_RE, '[/i]')
      .replace(HTML_UNDERLINE_OPEN_RE, '[u]')
      .replace(HTML_UNDERLINE_CLOSE_RE, '[/u]')
      .replace(HTML_ANCHOR_OPEN_RE, (_match, _quote, href) => {
        const sanitized = sanitizeExternalUrl(href);
        return sanitized ? `[url=${sanitized}]` : '';
      })
      .replace(HTML_ANCHOR_CLOSE_RE, '[/url]')
      .replace(HTML_TAG_RE, '')
      .replace(/\u00a0/g, ' ')
      .replace(/[ \t]+\n/g, '\n')
      .replace(/\n[ \t]+/g, '\n')
      .trim(),
  );
}

function parseBbCodeTag(rawTagContent) {
  const tagContent = String(rawTagContent || '').trim();
  if (!tagContent) {
    return null;
  }

  const lowerTag = tagContent.toLowerCase();

  if (lowerTag === '*' || lowerTag === 'br' || lowerTag === 'br/') {
    return {
      kind: 'self',
      name: lowerTag.startsWith('br') ? 'br' : 'item',
    };
  }

  if (lowerTag.startsWith('/')) {
    const tagName = lowerTag.slice(1);
    if (SUPPORTED_BBCODE_TAGS.has(tagName)) {
      return {
        kind: 'close',
        name: tagName,
      };
    }

    if (DISCARDABLE_BBCODE_RE.test(lowerTag)) {
      return {
        kind: 'discard',
      };
    }

    return null;
  }

  const [tagName, ...rest] = tagContent.split('=');
  const normalizedTagName = tagName.trim().toLowerCase();
  const rawValue = rest.length > 0 ? tagContent.slice(tagContent.indexOf('=') + 1).trim() : '';

  if (SUPPORTED_BBCODE_TAGS.has(normalizedTagName)) {
    return {
      kind: 'open',
      name: normalizedTagName,
      value: rawValue,
    };
  }

  if (DISCARDABLE_BBCODE_RE.test(lowerTag)) {
    return {
      kind: 'discard',
    };
  }

  return null;
}

function tokenizeNexusRichText(raw) {
  const text = normalizeNexusRichText(raw);
  const tokens = [];

  if (!text) {
    return tokens;
  }

  let buffer = '';
  let index = 0;

  const flushText = () => {
    if (buffer) {
      tokens.push({
        type: 'text',
        value: buffer,
      });
      buffer = '';
    }
  };

  while (index < text.length) {
    const char = text[index];

    if (char === '\n') {
      flushText();
      tokens.push({ type: 'br' });
      index += 1;
      continue;
    }

    if (char === '[') {
      const closingIndex = text.indexOf(']', index + 1);
      if (closingIndex !== -1) {
        const parsedTag = parseBbCodeTag(text.slice(index + 1, closingIndex));
        if (parsedTag) {
          flushText();

          if (parsedTag.kind === 'open') {
            tokens.push({
              type: 'open',
              name: parsedTag.name,
              value: parsedTag.value,
            });
          } else if (parsedTag.kind === 'close') {
            tokens.push({
              type: 'close',
              name: parsedTag.name,
            });
          } else if (parsedTag.kind === 'self') {
            tokens.push({
              type: parsedTag.name === 'item' ? 'item' : 'br',
            });
          }

          index = closingIndex + 1;
          continue;
        }
      }
    }

    buffer += char;
    index += 1;
  }

  flushText();
  return tokens;
}

function appendChild(container, child) {
  if (!child) {
    return;
  }

  if (container.type === 'list' && child.type !== 'item') {
    const lastChild = container.children[container.children.length - 1];
    if (lastChild?.type === 'item') {
      appendChild(lastChild, child);
      return;
    }

    const implicitItem = {
      type: 'item',
      children: [],
    };
    container.children.push(implicitItem);
    appendChild(implicitItem, child);
    return;
  }

  if (child.type === 'text') {
    const lastChild = container.children[container.children.length - 1];
    if (lastChild?.type === 'text') {
      lastChild.value += child.value;
      return;
    }
  }

  container.children.push(child);
}

function findContainerIndex(stack, type) {
  for (let index = stack.length - 1; index >= 0; index -= 1) {
    if (stack[index].type === type) {
      return index;
    }
  }
  return -1;
}

export function parseNexusRichText(raw) {
  const root = {
    type: 'root',
    children: [],
  };
  const stack = [root];

  tokenizeNexusRichText(raw).forEach((token) => {
    const current = stack[stack.length - 1];

    if (token.type === 'text') {
      appendChild(current, {
        type: 'text',
        value: token.value,
      });
      return;
    }

    if (token.type === 'br') {
      appendChild(current, { type: 'br' });
      return;
    }

    if (token.type === 'item') {
      const listIndex = findContainerIndex(stack, 'list');
      if (listIndex === -1) {
        appendChild(current, {
          type: 'text',
          value: '• ',
        });
        return;
      }

      stack.length = listIndex + 1;
      const listNode = stack[listIndex];
      const itemNode = {
        type: 'item',
        children: [],
      };
      listNode.children.push(itemNode);
      stack.push(itemNode);
      return;
    }

    if (token.type === 'open') {
      const nextNode = {
        type: token.name === 'b'
          ? 'bold'
          : token.name === 'i'
            ? 'italic'
            : token.name === 'u'
              ? 'underline'
              : token.name,
        children: [],
      };

      if (token.name === 'url') {
        nextNode.href = token.value || '';
      }

      appendChild(current, nextNode);
      stack.push(nextNode);
      return;
    }

    if (token.type === 'close') {
      const expectedType = token.name === 'b'
        ? 'bold'
        : token.name === 'i'
          ? 'italic'
          : token.name === 'u'
            ? 'underline'
            : token.name;
      const containerIndex = findContainerIndex(stack, expectedType);
      if (containerIndex > 0) {
        stack.length = containerIndex;
      }
    }
  });

  return root.children;
}

export function extractPlainTextFromNexusRichText(raw) {
  const tokens = tokenizeNexusRichText(raw);
  if (tokens.length === 0) {
    return '';
  }

  let text = '';
  let pendingBullet = false;

  tokens.forEach((token) => {
    if (token.type === 'text') {
      text += pendingBullet ? `• ${token.value}` : token.value;
      pendingBullet = false;
      return;
    }

    if (token.type === 'br') {
      text = text.replace(/[ \t]+$/g, '');
      text += '\n';
      pendingBullet = false;
      return;
    }

    if (token.type === 'item') {
      text = text.replace(/[ \t]+$/g, '');
      if (text && !text.endsWith('\n')) {
        text += '\n';
      }
      pendingBullet = true;
    }
  });

  return text
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
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

  if (nexusTranslationsSnapshot) {
    return { ...nexusTranslationsSnapshot };
  }

  if (nexusTranslationsLoadPromise) {
    const loaded = await nexusTranslationsLoadPromise;
    return { ...loaded };
  }

  try {
    nexusTranslationsLoadPromise = window.api.loadNexusTranslations().then((data) => (
      data && typeof data === 'object' ? data : {}
    ));
    const loaded = await nexusTranslationsLoadPromise;
    nexusTranslationsSnapshot = loaded;
    return { ...loaded };
  } catch (_error) {
    return {};
  } finally {
    nexusTranslationsLoadPromise = null;
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
  nexusTranslationsSnapshot = { ...data };
  return result;
}

async function mergeAndPersistNexusTranslations({ modId, existing = {}, updates }) {
  const translationKey = getNexusTranslationKey(modId);

  nexusTranslationsWriteQueue = nexusTranslationsWriteQueue.then(async () => {
    const latestMap = await loadNexusTranslationsMap();
    const nextMap = {
      ...latestMap,
      [translationKey]: {
        ...(latestMap[translationKey] || existing || {}),
        ...updates,
      },
    };
    await saveNexusTranslationsMap(nextMap);
    return nextMap;
  });

  const savedMap = await nexusTranslationsWriteQueue;
  return {
    translationKey,
    translations: savedMap ? { ...savedMap } : {},
  };
}

export async function translateNexusModFields({
  modId,
  name,
  description,
  existing = {},
  includeDescription = false,
}) {
  const jobs = [];
  const cleanDescription = extractPlainTextFromNexusRichText(description);

  if (name && !isChineseText(name)) {
    jobs.push(
      window.api.translateSmart(name).then((result) => ({
        type: 'name',
        result,
      })),
    );
  }

  if (includeDescription && cleanDescription && !isChineseText(cleanDescription)) {
    jobs.push(
      window.api.translateSmart(cleanDescription).then((result) => ({
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
  const { translations } = await mergeAndPersistNexusTranslations({
    modId,
    existing,
    updates,
  });

  return {
    success: true,
    updates,
    translationKey,
    translations,
    error: errors[0] || null,
  };
}
