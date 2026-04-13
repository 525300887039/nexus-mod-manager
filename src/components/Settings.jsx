import React from 'react';
import {
  ExternalLink,
  Github,
  Globe,
  Info,
  Languages,
  Shield,
} from 'lucide-react';
import NexusSettings from './NexusSettings';
import TranslateSettings from './TranslateSettings';

const VERSION = '2.1.0';
const TABS = [
  { id: 'nexus', label: 'Nexus Mods', icon: Globe },
  { id: 'translate', label: '翻译设置', icon: Languages },
  { id: 'about', label: '关于', icon: Info },
];

function AboutCard({ title, value, icon: Icon, actionLabel, onAction }) {
  return (
    <div className="rounded-2xl border border-gray-100 bg-white p-5 shadow-sm">
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-gray-900 text-white">
            <Icon size={20} />
          </div>
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-gray-400">{title}</p>
            <p className="mt-2 text-sm font-medium text-gray-900 break-all">{value}</p>
          </div>
        </div>
        {actionLabel && onAction && (
          <button
            type="button"
            onClick={onAction}
            className="inline-flex items-center gap-1 rounded-lg border border-gray-200 px-3 py-2 text-xs font-medium text-gray-600 transition-colors hover:bg-gray-50 hover:text-gray-900"
          >
            <ExternalLink size={14} />
            {actionLabel}
          </button>
        )}
      </div>
    </div>
  );
}

export default function Settings({
  activeTab = 'nexus',
  onTabChange,
  onShowToast,
  onConfirm,
}) {
  const resolvedActiveTab = TABS.some((tab) => tab.id === activeTab) ? activeTab : 'nexus';

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="px-8 pt-6 pb-4">
        <div className="mb-6 flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold text-gray-900">设置</h1>
            <p className="mt-1 text-sm text-gray-500">
              统一管理 Nexus API、翻译引擎，以及当前应用的版本与项目信息。
            </p>
          </div>
        </div>

        <div className="mb-6 flex flex-wrap gap-3">
          <div className="flex rounded-xl bg-gray-100 p-1">
            {TABS.map((tab) => {
              const TabIcon = tab.icon;
              const selected = resolvedActiveTab === tab.id;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => onTabChange?.(tab.id)}
                  className={`inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-colors ${
                    selected
                      ? 'bg-white text-gray-900 shadow-sm'
                      : 'text-gray-500 hover:text-gray-700'
                  }`}
                >
                  <TabIcon size={16} />
                  {tab.label}
                </button>
              );
            })}
          </div>
        </div>

        {resolvedActiveTab === 'nexus' && (
          <NexusSettings onShowToast={onShowToast} />
        )}

        {resolvedActiveTab === 'translate' && (
          <TranslateSettings
            embedded
            onShowToast={onShowToast}
            onConfirm={onConfirm}
          />
        )}

        {resolvedActiveTab === 'about' && (
          <div className="grid gap-4 xl:grid-cols-2">
            <div className="rounded-3xl border border-gray-100 bg-white p-6 shadow-sm xl:col-span-2">
              <div className="flex items-start gap-4">
                <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-gray-900 text-white">
                  <Shield size={24} />
                </div>
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.2em] text-gray-400">STS2 Mod Manager</p>
                  <h2 className="mt-2 text-2xl font-bold text-gray-900">统一管理 Slay the Spire 2 MOD 与 Nexus 工作流</h2>
                  <p className="mt-3 max-w-3xl text-sm leading-6 text-gray-500">
                    当前版本 {VERSION}，许可证为 MIT。这个分支延续原项目的本地 MOD 管理能力，并补齐 Nexus 浏览、下载和翻译整合体验。
                  </p>
                </div>
              </div>
            </div>

            <AboutCard title="应用版本" value={VERSION} icon={Info} />
            <AboutCard title="许可证" value="MIT" icon={Shield} />
            <AboutCard
              title="原项目"
              value="https://github.com/ImogeneOctaviap794/sts2-mod-manager"
              icon={Github}
              actionLabel="打开"
              onAction={() => window.api.openUrl('https://github.com/ImogeneOctaviap794/sts2-mod-manager')}
            />
            <AboutCard
              title="Fork 项目"
              value="https://github.com/525300887039/sts2-mod-manager"
              icon={Github}
              actionLabel="打开"
              onAction={() => window.api.openUrl('https://github.com/525300887039/sts2-mod-manager')}
            />
          </div>
        )}
      </div>
    </div>
  );
}
