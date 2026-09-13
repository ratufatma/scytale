import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  Sun,
  Moon,
  Globe,
  Radio,
  Terminal,
  PanelLeftClose,
  PanelLeftOpen,
  PanelRightClose,
  PanelRightOpen,
} from 'lucide-react';
import { ThemeMode, LanguageCode } from '../types';

interface TitleBarProps {
  theme: ThemeMode;
  onToggleTheme: () => void;
  language: LanguageCode;
  onToggleLanguage: () => void;
  leftOpen: boolean;
  onToggleLeft: () => void;
  rightOpen: boolean;
  onToggleRight: () => void;
  bottomOpen: boolean;
  onToggleBottom: () => void;
  nodeConnected: boolean;
  nodeUrl: string;
  blockHeight: number | null;
}

export const TitleBar: React.FC<TitleBarProps> = ({
  theme,
  onToggleTheme,
  language,
  onToggleLanguage,
  leftOpen,
  onToggleLeft,
  rightOpen,
  onToggleRight,
  bottomOpen,
  onToggleBottom,
  nodeConnected,
  nodeUrl,
  blockHeight,
}) => {
  const { t } = useTranslation();

  return (
    <header
      id="workbench-titlebar"
      className="relative z-20 flex h-10 w-full flex-shrink-0 items-center justify-between border-b border-zinc-200 bg-zinc-100 px-3 text-zinc-700 transition-colors duration-150 select-none dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-200"
    >
      <div data-tauri-drag-region className="absolute inset-0 -z-10" />
      {/* Left: App Identity */}
      <div className="flex min-w-[200px] items-center gap-3">
        {/* Brand Tag */}
        <div className="flex items-center gap-2">
          <img
            src="/icon.png"
            alt="Scytale"
            className="h-5 w-5 rounded object-cover shadow-sm"
          />
          <span className="font-semibold text-xs tracking-tight text-zinc-900 dark:text-zinc-100">
            {t('app.title')}
          </span>
          <span className="text-[10px] font-mono uppercase px-1.5 py-0.5 rounded bg-zinc-200/70 dark:bg-zinc-800/80 text-zinc-600 dark:text-zinc-400 font-medium">
            v1.2.0-desktop
          </span>
        </div>
      </div>

      {/* Center: Live network context */}
      <div className="hidden flex-1 items-center justify-center md:flex">
        <div className="flex items-center gap-2 text-[10px] font-mono text-zinc-500 dark:text-zinc-400">
          <Radio className="h-3 w-3 text-emerald-500" />
          <span>{nodeConnected ? 'Connected' : 'Offline'}</span>
          <span className="text-zinc-300 dark:text-zinc-700">•</span>
          <span>{nodeUrl.replace(/^https?:\/\//, '')}</span>
          <span className="text-zinc-300 dark:text-zinc-700">•</span>
          <span>Block Tip: #{blockHeight ?? '—'}</span>
        </div>
      </div>

      {/* Right: Layout Toggles, Language Switcher, Theme Switcher, Connection Badge */}
      <div className="flex items-center gap-1.5 sm:gap-2">
        {/* Panel View Toggles */}
        <div className="flex items-center gap-0.5 pr-1 border-r border-zinc-300 dark:border-zinc-800">
          <button
            id="toggle-left-panel"
            onClick={onToggleLeft}
            title={leftOpen ? "Hide Explorer" : "Show Explorer"}
            className={`p-1.5 rounded hover:bg-zinc-200/70 dark:hover:bg-zinc-800 transition-colors ${leftOpen ? 'text-zinc-900 dark:text-zinc-100' : 'text-zinc-400'
              }`}
          >
            {leftOpen ? <PanelLeftClose className="w-3.5 h-3.5" /> : <PanelLeftOpen className="w-3.5 h-3.5" />}
          </button>
          <button
            id="toggle-bottom-panel"
            onClick={onToggleBottom}
            title={bottomOpen ? "Hide Terminal" : "Show Terminal"}
            className={`p-1.5 rounded hover:bg-zinc-200/70 dark:hover:bg-zinc-800 transition-colors ${bottomOpen ? 'text-zinc-900 dark:text-zinc-100' : 'text-zinc-400'
              }`}
          >
            <Terminal className="w-3.5 h-3.5" />
          </button>
          <button
            id="toggle-right-panel"
            onClick={onToggleRight}
            title={rightOpen ? "Hide Inspector" : "Show Inspector"}
            className={`p-1.5 rounded hover:bg-zinc-200/70 dark:hover:bg-zinc-800 transition-colors ${rightOpen ? 'text-zinc-900 dark:text-zinc-100' : 'text-zinc-400'
              }`}
          >
            {rightOpen ? <PanelRightClose className="w-3.5 h-3.5" /> : <PanelRightOpen className="w-3.5 h-3.5" />}
          </button>
        </div>

        {/* Network Connection Badge */}
        <div
          id="titlebar-connection-badge"
          className="hidden sm:flex items-center gap-1.5 px-2 py-1 rounded bg-emerald-500/10 border border-emerald-500/20 text-emerald-700 dark:text-emerald-400 text-xs font-medium"
        >
          <Radio className="w-3 h-3 animate-pulse" />
          <span className="text-[11px] font-mono font-medium">{nodeConnected ? 'NODE' : 'OFFLINE'}</span>
        </div>

        {/* Language Switcher */}
        <button
          id="btn-language-switch"
          onClick={onToggleLanguage}
          title={t('header.switchLanguage')}
          className="flex items-center gap-1 px-2 py-1 rounded text-xs font-medium bg-zinc-200/60 dark:bg-zinc-800 hover:bg-zinc-200 dark:hover:bg-zinc-700 transition-colors border border-zinc-300/60 dark:border-zinc-700 text-zinc-800 dark:text-zinc-200 cursor-pointer"
        >
          <Globe className="w-3 h-3 text-zinc-500 dark:text-zinc-400" />
          <span className="font-mono text-[11px] font-bold uppercase">{language}</span>
        </button>

        {/* Theme Toggle (Persistent) */}
        <button
          id="btn-theme-toggle"
          onClick={onToggleTheme}
          title={t('header.toggleTheme')}
          className="p-1.5 rounded text-xs bg-zinc-200/60 dark:bg-zinc-800 hover:bg-zinc-200 dark:hover:bg-zinc-700 transition-colors border border-zinc-300/60 dark:border-zinc-700 text-zinc-800 dark:text-zinc-200 cursor-pointer"
        >
          {theme === 'dark' ? (
            <Sun className="w-3.5 h-3.5 text-amber-400" />
          ) : (
            <Moon className="w-3.5 h-3.5 text-zinc-700" />
          )}
        </button>

      </div>
    </header>
  );
};
