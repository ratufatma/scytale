import React from 'react';
import { useTranslation } from 'react-i18next';
import { CheckCircle2 } from 'lucide-react';
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
}) => {
  const { t } = useTranslation();

  return (
    <footer
      id="workbench-status-bar"
      className="z-20 flex h-6 w-full flex-shrink-0 select-none items-center border-t border-zinc-300/80 bg-zinc-200/90 px-3 text-[11px] font-mono text-zinc-600 transition-colors duration-150 dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-400"
    >
      <div id="status-node-indicator" className="flex items-center gap-1.5 font-semibold text-emerald-600 dark:text-emerald-400">
        <CheckCircle2 className="h-3 w-3" />
        <span>{t('statusbar.nodeConnected')}</span>
      </div>
    </footer>
  );
};
