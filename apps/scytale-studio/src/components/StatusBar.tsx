import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  Radio,
  GitBranch,
  CheckCircle2,
  Cpu,
  Layers,
  Globe,
  Sun,
  Moon,
  Zap,
} from 'lucide-react';
import { ThemeMode, LanguageCode } from '../types';

interface StatusBarProps {
  theme: ThemeMode;
  language: LanguageCode;
  blockHeight: number;
  syncLagMs: number;
  onToggleLanguage: () => void;
  onToggleTheme: () => void;
}

export const StatusBar: React.FC<StatusBarProps> = ({
  theme,
  language,
  blockHeight,
  syncLagMs,
  onToggleLanguage,
  onToggleTheme,
}) => {
  const { t } = useTranslation();

  return (
    <footer
      id="workbench-status-bar"
      className="h-6 w-full flex-shrink-0 px-3 flex items-center justify-between text-[11px] font-mono select-none bg-zinc-200/90 dark:bg-zinc-950 border-t border-zinc-300/80 dark:border-zinc-800 text-zinc-600 dark:text-zinc-400 transition-colors duration-150 z-20"
    >
      {/* Left: Node connection status, current block height / tip, Git branch */}
      <div className="flex items-center gap-3">
        {/* Node status pill */}
        <div
          id="status-node-indicator"
          className="flex items-center gap-1.5 hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors cursor-pointer"
        >
          <span className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
          <span className="font-semibold text-zinc-800 dark:text-zinc-200">
            {t('statusbar.nodeConnected')}
          </span>
        </div>

        {/* Current block height */}
        <div
          id="status-block-height"
          className="hidden sm:flex items-center gap-1 text-zinc-500 dark:text-zinc-400 hover:text-zinc-800 dark:hover:text-zinc-200 transition-colors cursor-pointer"
        >
          <Layers className="w-3 h-3 text-indigo-400" />
          <span>Block: #{blockHeight.toLocaleString()}</span>
        </div>

        {/* Git branch */}
        <div className="hidden md:flex items-center gap-1 text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200 transition-colors cursor-pointer">
          <GitBranch className="w-3 h-3 text-zinc-400" />
          <span>{t('statusbar.gitBranch')}*</span>
        </div>
      </div>

      {/* Right: Encoding, CRLF, Sync indicator, active language, theme */}
      <div className="flex items-center gap-3">
        {/* Sync Lag */}
        <div
          id="status-sync-indicator"
          className="hidden sm:flex items-center gap-1 text-emerald-600 dark:text-emerald-400 font-medium"
        >
          <Zap className="w-3 h-3" />
          <span>{syncLagMs}ms lag</span>
        </div>

        {/* Encoding */}
        <span
          id="status-encoding"
          className="hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors cursor-pointer hidden md:inline"
        >
          {t('statusbar.encoding')}
        </span>

        {/* Line separator */}
        <span className="hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors cursor-pointer hidden lg:inline">
          {t('statusbar.crlf')}
        </span>

        <span className="hover:text-zinc-900 dark:hover:text-zinc-200 transition-colors cursor-pointer hidden lg:inline">
          {t('statusbar.spaces')}
        </span>

        {/* Language Badge */}
        <button
          id="status-lang-badge"
          onClick={onToggleLanguage}
          title="Toggle Language"
          className="flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-zinc-300/80 dark:hover:bg-zinc-800 text-zinc-800 dark:text-zinc-200 font-bold uppercase transition-colors cursor-pointer"
        >
          <Globe className="w-2.5 h-2.5 text-zinc-400" />
          <span>{language}</span>
        </button>

        {/* Theme mode indicator */}
        <button
          id="status-theme-indicator"
          onClick={onToggleTheme}
          title="Toggle Theme"
          className="p-1 rounded hover:bg-zinc-300/80 dark:hover:bg-zinc-800 text-zinc-500 hover:text-zinc-800 dark:hover:text-zinc-200 transition-colors cursor-pointer"
        >
          {theme === 'dark' ? (
            <Sun className="w-3 h-3 text-amber-400" />
          ) : (
            <Moon className="w-3 h-3 text-zinc-600" />
          )}
        </button>
      </div>
    </footer>
  );
};
