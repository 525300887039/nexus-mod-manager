import React, { useMemo } from 'react';
import {
  extractPlainTextFromNexusRichText,
  parseNexusRichText,
  sanitizeExternalUrl,
} from './nexusShared';

function joinClassNames(...values) {
  return values.filter(Boolean).join(' ');
}

function collectNodeText(nodes) {
  return nodes.reduce((text, node) => {
    if (!node) {
      return text;
    }

    if (node.type === 'text') {
      return text + node.value;
    }

    if (node.type === 'br') {
      return text + '\n';
    }

    if (Array.isArray(node.children) && node.children.length > 0) {
      return text + collectNodeText(node.children);
    }

    return text;
  }, '');
}

function openExternalLink(event, href) {
  event.preventDefault();

  if (!href) {
    return;
  }

  if (typeof window.api?.openUrl === 'function') {
    window.api.openUrl(href);
    return;
  }

  window.open(href, '_blank', 'noopener,noreferrer');
}

function renderNodes(nodes, keyPrefix) {
  return nodes.map((node, index) => renderNode(node, `${keyPrefix}-${index}`)).filter(Boolean);
}

function renderNode(node, key) {
  if (!node) {
    return null;
  }

  switch (node.type) {
    case 'text':
      return node.value;
    case 'br':
      return <br key={key} />;
    case 'bold':
      return (
        <strong key={key} className="font-semibold text-gray-900">
          {renderNodes(node.children || [], key)}
        </strong>
      );
    case 'italic':
      return (
        <em key={key}>
          {renderNodes(node.children || [], key)}
        </em>
      );
    case 'underline':
      return (
        <span key={key} className="underline decoration-gray-400 underline-offset-2">
          {renderNodes(node.children || [], key)}
        </span>
      );
    case 'url': {
      const labelText = collectNodeText(node.children || []).trim();
      const href = sanitizeExternalUrl(node.href) || sanitizeExternalUrl(labelText);
      const content = labelText ? renderNodes(node.children || [], key) : href;

      if (!content) {
        return null;
      }

      if (!href) {
        return (
          <span key={key} className="break-all text-gray-700">
            {content}
          </span>
        );
      }

      return (
        <a
          key={key}
          href={href}
          onClick={(event) => openExternalLink(event, href)}
          className="break-all text-blue-600 underline decoration-blue-300 underline-offset-2 transition-colors hover:text-blue-800"
        >
          {content}
        </a>
      );
    }
    case 'list':
      if (!node.children?.length) {
        return null;
      }
      return (
        <ul key={key} className="my-3 list-disc space-y-2 pl-5 marker:text-gray-400">
          {node.children
            .filter((child) => child?.type === 'item')
            .map((child, index) => renderNode(child, `${key}-item-${index}`))
            .filter(Boolean)}
        </ul>
      );
    case 'item':
      return (
        <li key={key} className="break-words">
          {renderNodes(node.children || [], key)}
        </li>
      );
    case 'size':
      return (
        <React.Fragment key={key}>
          {renderNodes(node.children || [], key)}
        </React.Fragment>
      );
    default:
      return Array.isArray(node.children) ? renderNodes(node.children, key) : null;
  }
}

export default function NexusRichText({
  content,
  className = 'text-sm leading-7 text-gray-700',
  fallbackClassName = 'whitespace-pre-wrap break-words text-sm leading-7 text-gray-700',
  emptyText = '暂无描述',
  emptyClassName = 'text-sm leading-7 text-gray-400',
}) {
  const parsedNodes = useMemo(() => parseNexusRichText(content), [content]);
  const plainText = useMemo(() => extractPlainTextFromNexusRichText(content), [content]);
  const hasStructuredContent = useMemo(
    () => parsedNodes.some((node) => {
      if (!node) {
        return false;
      }
      if (node.type === 'text') {
        return node.value.trim().length > 0;
      }
      return true;
    }),
    [parsedNodes],
  );

  if (hasStructuredContent) {
    return (
      <div className={joinClassNames('break-words', className)}>
        {renderNodes(parsedNodes, 'nexus-rich-text')}
      </div>
    );
  }

  if (plainText) {
    return (
      <p className={joinClassNames(fallbackClassName)}>
        {plainText}
      </p>
    );
  }

  return (
    <p className={joinClassNames(emptyClassName)}>
      {emptyText}
    </p>
  );
}
