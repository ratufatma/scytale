import { invoke } from '@tauri-apps/api/core';
import { Activity, Check, Copy, RefreshCw, SlidersHorizontal, Wallet, Zap } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

interface RightInspectorProps {
    onExecuteTool: (toolName: string) => void;
}

interface NodeStatus {
    connected: boolean;
    node_url: string;
    response_time_ms: number;
    block_height: number | null;
    error: string | null;
}

interface AccountDetails {
    account_number: string | null;
    passbook_id: string | null;
    balance_quanta: number | null;
    registered: boolean;
}

const emptyNode: NodeStatus = {
    connected: false,
    node_url: 'http://127.0.0.1:8332',
    response_time_ms: 0,
    block_height: null,
    error: null,
};

const emptyAccount: AccountDetails = {
    account_number: null,
    passbook_id: null,
    balance_quanta: null,
    registered: false,
};

function formatQuanta(value: number | null): string {
    return value === null ? '—' : `${(value / 100_000_000).toFixed(8)} SCY`;
}

export function RightInspector({ onExecuteTool }: RightInspectorProps) {
    const { t } = useTranslation();
    const [node, setNode] = useState<NodeStatus>(emptyNode);
    const [account, setAccount] = useState<AccountDetails>(emptyAccount);
    const [refreshing, setRefreshing] = useState(false);
    const [copied, setCopied] = useState(false);

    const refresh = useCallback(async () => {
        setRefreshing(true);
        try {
            const [nodeStatus, accountDetails] = await Promise.all([
                invoke<NodeStatus>('get_node_telemetry', { nodeUrl: null }),
                invoke<AccountDetails>('get_active_account_details', { walletPath: null }),
            ]);
            setNode(nodeStatus);
            setAccount(accountDetails);
        } catch (error) {
            setNode((current) => ({ ...current, connected: false, error: String(error) }));
        } finally {
            setRefreshing(false);
        }
    }, []);

    useEffect(() => {
        void refresh();
        const interval = window.setInterval(() => void refresh(), 15_000);
        return () => window.clearInterval(interval);
    }, [refresh]);

    const copyAccount = () => {
        if (!account.account_number) return;
        void navigator.clipboard.writeText(account.account_number);
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
    };

    return (
        <aside id="workbench-right-inspector" data-tauri-drag-region="false" className="flex h-full w-72 flex-shrink-0 flex-col border-l border-zinc-200 bg-zinc-50/80 text-xs text-zinc-700 dark:border-zinc-800 dark:bg-zinc-900/60 dark:text-zinc-300">
            <div className="flex h-9 items-center justify-between border-b border-zinc-200/80 bg-zinc-100/60 px-3 dark:border-zinc-800/80 dark:bg-zinc-950/40">
                <div className="flex items-center gap-2 text-[11px] font-semibold tracking-wider text-zinc-600 dark:text-zinc-400">
                    <SlidersHorizontal className="h-3.5 w-3.5" />
                    <span>{t('inspector.title')}</span>
                </div>
                <button type="button" onClick={() => void refresh()} title={t('inspector.refresh')} className="rounded p-1 text-zinc-500 hover:bg-zinc-200 dark:hover:bg-zinc-800">
                    <RefreshCw className={`h-3.5 w-3.5 ${refreshing ? 'animate-spin' : ''}`} />
                </button>
            </div>

            <div className="flex-1 space-y-3 overflow-y-auto p-3">
                <section className="space-y-2.5 rounded-lg border border-zinc-200 bg-white p-3 shadow-sm dark:border-zinc-800/90 dark:bg-zinc-950/70">
                    <div className="flex items-center justify-between">
                        <h2 className="flex items-center gap-1.5 text-[11px] font-semibold text-zinc-800 dark:text-zinc-200"><Activity className="h-3.5 w-3.5 text-emerald-500" />{t('inspector.networkStatus')}</h2>
                        <span className={`flex items-center gap-1 font-mono text-[10px] ${node.connected ? 'text-emerald-500' : 'text-rose-500'}`}><span className={`h-1.5 w-1.5 rounded-full ${node.connected ? 'bg-emerald-500' : 'bg-rose-500'}`} />{node.connected ? t('inspector.online') : t('inspector.offline')}</span>
                    </div>
                    <div className="space-y-1.5 border-t border-zinc-100 pt-2 text-[11px] dark:border-zinc-900">
                        <div className="flex justify-between gap-2"><span>{t('inspector.rpcUrl')}</span><span className="max-w-[150px] truncate font-mono text-[10px]">{node.node_url}</span></div>
                        <div className="flex justify-between"><span>{t('inspector.blockHeight')}</span><span className="font-mono">{node.block_height ?? '—'}</span></div>
                        <div className="flex justify-between"><span>{t('inspector.latency')}</span><span className="font-mono">{node.response_time_ms} ms</span></div>
                    </div>
                </section>

                <section className="space-y-2.5 rounded-lg border border-zinc-200 bg-white p-3 shadow-sm dark:border-zinc-800/90 dark:bg-zinc-950/70">
                    <div className="flex items-center justify-between">
                        <h2 className="flex items-center gap-1.5 text-[11px] font-semibold text-zinc-800 dark:text-zinc-200"><Wallet className="h-3.5 w-3.5 text-blue-500" />{t('inspector.accountMetadata')}</h2>
                        <span className={`rounded px-1.5 py-0.5 font-mono text-[9px] ${account.registered ? 'bg-emerald-500/10 text-emerald-500' : 'bg-zinc-500/10 text-zinc-500'}`}>{account.registered ? t('inspector.registered') : t('inspector.local')}</span>
                    </div>
                    <div className="flex items-center justify-between gap-2 rounded border border-zinc-200 bg-zinc-50 p-2 font-mono text-[11px] dark:border-zinc-800 dark:bg-zinc-900"><span className="truncate font-semibold">{account.account_number ?? t('inspector.noAccount')}</span><button type="button" onClick={copyAccount} title={t('inspector.copyAccount')} className="shrink-0 text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-100">{copied ? <Check className="h-3 w-3 text-emerald-500" /> : <Copy className="h-3 w-3" />}</button></div>
                    <div className="space-y-1 text-[11px]"><div className="flex justify-between"><span>{t('inspector.balance')}</span><span className="font-mono font-semibold">{formatQuanta(account.balance_quanta)}</span></div><div className="flex justify-between gap-2"><span>{t('inspector.passbookId')}</span><span className="max-w-[150px] truncate font-mono text-[10px]">{account.passbook_id ?? '—'}</span></div></div>
                </section>

                <section className="space-y-2 rounded-lg border border-zinc-200 bg-white p-3 shadow-sm dark:border-zinc-800/90 dark:bg-zinc-950/70">
                    <h2 className="flex items-center gap-1.5 text-[11px] font-semibold text-zinc-800 dark:text-zinc-200"><Zap className="h-3.5 w-3.5 text-amber-500" />{t('inspector.fees')}</h2>
                    <div className="flex items-center justify-between border-t border-zinc-100 pt-2 text-[11px] dark:border-zinc-900"><span>{t('inspector.minimumFee')}</span><span className="font-mono font-semibold">60 quanta</span></div>
                    <button type="button" onClick={() => onExecuteTool('rpc.estimateGas')} className="w-full rounded border border-zinc-200 px-2 py-1.5 text-left text-[10px] text-zinc-500 hover:bg-zinc-100 dark:border-zinc-800 dark:hover:bg-zinc-900">{t('inspector.estimateGas')}</button>
                </section>
            </div>
        </aside>
    );
}
