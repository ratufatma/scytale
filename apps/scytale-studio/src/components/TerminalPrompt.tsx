import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { FitAddon } from '@xterm/addon-fit';
import { Terminal as XTerm } from '@xterm/xterm';
import '@xterm/xterm/css/xterm.css';
import { Copy, Eraser, TerminalSquare } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ConsoleLog } from '../types';

interface TerminalPromptProps {
    logs: ConsoleLog[];
    onClearLogs: () => void;
    onExecutePrompt: (command: string) => void;
    isExecuting?: boolean;
}

const terminalTheme = (dark: boolean) => ({
    background: dark ? '#09090b' : '#ffffff',
    foreground: dark ? '#f4f4f5' : '#18181b',
    cursor: dark ? '#f4f4f5' : '#18181b',
    selectionBackground: dark ? '#3f3f46' : '#d4d4d8',
});

export function TerminalPrompt({ logs, onClearLogs, isExecuting = false }: TerminalPromptProps) {
    const { t } = useTranslation();
    const containerRef = useRef<HTMLDivElement>(null);
    const terminalRef = useRef<XTerm | null>(null);
    const [copied, setCopied] = useState(false);

    useEffect(() => {
        if (!containerRef.current) return;
        const isTauriRuntime = '__TAURI_INTERNALS__' in window;
        if (!isTauriRuntime) {
            containerRef.current.textContent = 'Browser preview: launch Tauri Desktop to use the native PTY.';
            return;
        }
        const terminal = new XTerm({
            cursorBlink: true,
            convertEol: true,
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
            fontSize: 12,
            theme: terminalTheme(document.documentElement.classList.contains('dark')),
        });
        const fitAddon = new FitAddon();
        terminal.loadAddon(fitAddon);
        terminal.open(containerRef.current);
        terminalRef.current = terminal;

        const resize = () => {
            fitAddon.fit();
            if (isTauriRuntime) {
                void invoke('pty_resize', { cols: terminal.cols, rows: terminal.rows });
            }
        };
        resize();
        window.requestAnimationFrame(() => {
            resize();
            terminal.focus();
        });
        const observer = new ResizeObserver(resize);
        observer.observe(containerRef.current);
        const themeObserver = new MutationObserver(() => {
            terminal.options.theme = terminalTheme(document.documentElement.classList.contains('dark'));
        });
        themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });

        let disposed = false;
        let unlisten: (() => void) | undefined;
        void listen<string>('pty_output', (event) => terminal.write(event.payload)).then((stop) => {
            if (disposed) {
                stop();
                return;
            }
            unlisten = stop;
            void invoke('pty_write', { data: '\n' }).catch((error) => {
                terminal.write(`\r\n[PTY unavailable] ${String(error)}\r\n`);
            });
        }).catch((error) => {
            terminal.write(`\r\n[PTY listener error] ${String(error)}\r\n`);
        });
        const input = terminal.onData((data) => {
            void invoke('pty_write', { data }).catch((error) => {
                terminal.write(`\r\n[PTY write error] ${String(error)}\r\n`);
            });
        });

        return () => {
            disposed = true;
            unlisten?.();
            input.dispose();
            observer.disconnect();
            themeObserver.disconnect();
            terminal.dispose();
            terminalRef.current = null;
        };
    }, []);

    const copyLogs = () => {
        void navigator.clipboard.writeText(
            logs.map((log) => `[${log.timestamp}] [${log.level.toUpperCase()}] ${log.message}`).join('\n'),
        );
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1600);
    };

    return (
        <section id="workbench-terminal" className="h-56 flex-shrink-0 border-t border-zinc-200 bg-zinc-50 dark:border-zinc-800 dark:bg-zinc-950">
            <div className="flex h-8 items-center justify-between border-b border-zinc-200 bg-zinc-100 px-3 text-xs dark:border-zinc-800 dark:bg-zinc-900">
                <div className="flex items-center gap-2 font-semibold text-zinc-700 dark:text-zinc-300">
                    <TerminalSquare className="h-3.5 w-3.5" />
                    <span>{t('terminal.title')}</span>
                    <span className="flex items-center gap-1 text-[10px] font-normal text-zinc-500">
                        <span className={`h-1.5 w-1.5 rounded-full ${isExecuting ? 'bg-amber-400' : 'bg-emerald-500'}`} />
                        {isExecuting ? t('terminal.statusExecuting') : t('terminal.statusIdle')}
                    </span>
                </div>
                <div className="flex items-center gap-2 text-zinc-500">
                    <span className="font-mono text-[10px]">{t('terminal.shortcutHint')}</span>
                    <button type="button" onClick={copyLogs} title={t('terminal.copyLogs')} className="rounded p-1 hover:bg-zinc-200 dark:hover:bg-zinc-800">
                        <Copy className={`h-3 w-3 ${copied ? 'text-emerald-500' : ''}`} />
                    </button>
                    <button type="button" onClick={() => { terminalRef.current?.clear(); onClearLogs(); }} title={t('terminal.clearLogs')} className="rounded p-1 hover:bg-zinc-200 dark:hover:bg-zinc-800">
                        <Eraser className="h-3 w-3" />
                    </button>
                </div>
            </div>
            <div
                ref={containerRef}
                data-tauri-drag-region="false"
                onClick={() => terminalRef.current?.focus()}
                className="h-[calc(100%-2rem)] w-full bg-white px-2 py-1 dark:bg-zinc-950"
            />
        </section>
    );
}
