import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { LanguageCode, ThemeMode } from '../types';
import { CenterWorkspace } from './CenterWorkspace';
import { LeftExplorer } from './LeftExplorer';
import { RightInspector } from './RightInspector';
import { StatusBar } from './StatusBar';
import { TerminalPrompt } from './TerminalPrompt';
import { TitleBar } from './TitleBar';

interface NodeStatus {
    connected: boolean;
    node_url: string;
    response_time_ms: number;
    block_height: number | null;
}

const emptyNode: NodeStatus = {
    connected: false,
    node_url: 'http://127.0.0.1:8332',
    response_time_ms: 0,
    block_height: null,
};

export const Workbench: React.FC = () => {
    const { i18n } = useTranslation();
    const [theme, setTheme] = useState<ThemeMode>(() => localStorage.getItem('scytale_theme') === 'light' ? 'light' : 'dark');
    const [language, setLanguage] = useState<LanguageCode>(() => i18n.language === 'id' ? 'id' : 'en');
    const [leftOpen, setLeftOpen] = useState(true);
    const [rightOpen, setRightOpen] = useState(true);
    const [bottomOpen, setBottomOpen] = useState(true);
    const [activeWalletPath, setActiveWalletPath] = useState<string | null>(null);
    const [node, setNode] = useState<NodeStatus>(emptyNode);

    useEffect(() => {
        document.documentElement.classList.toggle('dark', theme === 'dark');
        localStorage.setItem('scytale_theme', theme);
    }, [theme]);

    const refreshNode = useCallback(async () => {
        try {
            setNode(await invoke<NodeStatus>('get_node_telemetry', { nodeUrl: null }));
        } catch {
            setNode((current) => ({ ...current, connected: false }));
        }
    }, []);

    useEffect(() => {
        void refreshNode();
        const interval = window.setInterval(() => void refreshNode(), 15_000);
        return () => window.clearInterval(interval);
    }, [refreshNode]);

    const toggleLanguage = () => {
        const next: LanguageCode = language === 'en' ? 'id' : 'en';
        setLanguage(next);
        i18n.changeLanguage(next);
    };

    return (
        <div className="flex h-screen w-screen flex-col overflow-hidden bg-zinc-100 font-sans text-zinc-900 transition-colors dark:bg-zinc-950 dark:text-zinc-100">
            <TitleBar
                theme={theme}
                onToggleTheme={() => setTheme((current) => current === 'dark' ? 'light' : 'dark')}
                language={language}
                onToggleLanguage={toggleLanguage}
                leftOpen={leftOpen}
                onToggleLeft={() => setLeftOpen((current) => !current)}
                rightOpen={rightOpen}
                onToggleRight={() => setRightOpen((current) => !current)}
                bottomOpen={bottomOpen}
                onToggleBottom={() => setBottomOpen((current) => !current)}
                nodeConnected={node.connected}
                nodeUrl={node.node_url}
                blockHeight={node.block_height}
            />
            <main className="relative flex min-h-0 flex-1 overflow-hidden">
                {leftOpen && <LeftExplorer activeWalletPath={activeWalletPath} onActiveWalletChange={setActiveWalletPath} />}
                <div data-tauri-drag-region="false" className="flex h-full min-w-0 flex-1 flex-col overflow-hidden">
                    <CenterWorkspace activeWalletPath={activeWalletPath} />
                    {bottomOpen && <TerminalPrompt logs={[]} onClearLogs={() => undefined} onExecutePrompt={() => undefined} isExecuting={false} />}
                </div>
                {rightOpen && <RightInspector onExecuteTool={() => undefined} />}
            </main>
            <StatusBar
                theme={theme}
                language={language}
                blockHeight={node.block_height ?? 0}
                syncLagMs={node.response_time_ms}
                onToggleLanguage={toggleLanguage}
                onToggleTheme={() => setTheme((current) => current === 'dark' ? 'light' : 'dark')}
            />
        </div>
    );
};

export default Workbench;
