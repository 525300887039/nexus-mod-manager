import React, { useEffect } from 'react';
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  Loader2,
  Wrench,
  X,
} from 'lucide-react';

const PHASE_META = {
  preparing: {
    icon: Download,
    iconClassName: 'text-sky-600',
    containerClassName: 'border-sky-200 bg-sky-50 text-sky-800',
  },
  downloading: {
    icon: Loader2,
    iconClassName: 'text-sky-600 animate-spin',
    containerClassName: 'border-sky-200 bg-sky-50 text-sky-800',
  },
  installing: {
    icon: Wrench,
    iconClassName: 'text-amber-600',
    containerClassName: 'border-amber-200 bg-amber-50 text-amber-800',
  },
  success: {
    icon: CheckCircle2,
    iconClassName: 'text-emerald-600',
    containerClassName: 'border-emerald-200 bg-emerald-50 text-emerald-800',
  },
  error: {
    icon: AlertTriangle,
    iconClassName: 'text-red-600',
    containerClassName: 'border-red-200 bg-red-50 text-red-800',
  },
};

export default function DownloadProgress({ status, onClose }) {
  useEffect(() => {
    if (!status || status.phase !== 'success') {
      return undefined;
    }

    const timer = window.setTimeout(() => {
      if (onClose) {
        onClose();
      }
    }, 3000);

    return () => window.clearTimeout(timer);
  }, [onClose, status]);

  if (!status) {
    return null;
  }

  const meta = PHASE_META[status.phase] || PHASE_META.preparing;
  const Icon = meta.icon;
  const showClose = status.phase === 'error';

  return (
    <div
      className={`fixed bottom-4 right-4 z-50 w-[360px] max-w-[calc(100vw-2rem)] rounded-xl border px-4 py-3 shadow-lg ${meta.containerClassName}`}
    >
      <div className="flex items-start gap-3">
        <div className="mt-0.5 flex h-9 w-9 items-center justify-center rounded-full bg-white/70">
          <Icon size={18} className={meta.iconClassName} />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold">{status.message}</p>
          {status.fileName && (
            <p className="mt-1 truncate text-xs opacity-75">{status.fileName}</p>
          )}
        </div>
        {showClose && (
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-1.5 text-current/60 transition-colors hover:bg-white/70 hover:text-current"
          >
            <X size={15} />
          </button>
        )}
      </div>
    </div>
  );
}
