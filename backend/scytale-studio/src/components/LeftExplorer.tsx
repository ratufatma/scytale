import { invoke } from '@tauri-apps/api/core';
import { CheckCircle2, Clock3, Plus, RefreshCw, ShieldCheck, Wallet } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

interface WalletSummary {
    account_number: string | null;
    path: string;
    active: boolean;
}

interface LeftExplorerProps {
    activeWalletPath: string | null;
    onActiveWalletChange: (path: string) => void;
}

export const LeftExplorer: React.FC<LeftExplorerProps> = ({ activeWalletPath, onActiveWalletChange }) => {
    const { t } = useTranslation();
    const [wallets, setWallets] = useState<WalletSummary[]>([]);
    const [pin, setPin] = useState('');
    const [showPinDialog, setShowPinDialog] = useState(false);
    const [walletError, setWalletError] = useState('');
    const [loadingWallets, setLoadingWallets] = useState(false);
    const [activity, setActivity] = useState<string[]>([]);

    const refreshWallets = async () => {
        if (!('__TAURI_INTERNALS__' in window)) return;
        setLoadingWallets(true);
        setWalletError('');
        try {
            setWallets(await invoke<WalletSummary[]>('list_local_wallets'));
        } catch (error) {
            setWalletError(String(error));
        } finally {
            setLoadingWallets(false);
        }
    };

    useEffect(() => {
        void refreshWallets();
        try {
            const saved = JSON.parse(localStorage.getItem('scytale_local_activity') ?? '[]');
            if (Array.isArray(saved)) setActivity(saved.filter((item): item is string => typeof item === 'string'));
        } catch {
            setActivity([]);
        }
    }, []);

    const createWallet = async () => {
        if (!/^\d{6}$/.test(pin)) {
            setWalletError(t('navigation.pinInvalid'));
            return;
        }
        try {
            await invoke('create_wallet_with_pin', { pin });
            setActivity((current) => {
                const next = [t('navigation.walletCreated'), ...current].slice(0, 5);
                localStorage.setItem('scytale_local_activity', JSON.stringify(next));
                return next;
            });
            setPin('');
            setShowPinDialog(false);
            await refreshWallets();
        } catch (error) {
            setWalletError(String(error));
        }
    };

    const selectWallet = async (wallet: WalletSummary) => {
        try {
            await invoke('set_active_wallet', { walletPath: wallet.path });
            onActiveWalletChange(wallet.path);
            setWallets((current) => current.map((item) => ({ ...item, active: item.path === wallet.path })));
        } catch (error) {
            setWalletError(String(error));
        }
    };

    return (
        <aside id="workbench-left-explorer" data-tauri-drag-region="false" className="flex h-full w-64 flex-shrink-0 flex-col border-r border-zinc-200 bg-zinc-50/80 text-xs text-zinc-700 dark:border-zinc-800 dark:bg-zinc-900/60 dark:text-zinc-300">
            <header className="flex h-10 items-center justify-between border-b border-zinc-200/80 bg-zinc-100/60 px-3 dark:border-zinc-800/80 dark:bg-zinc-950/40">
                <div className="flex items-center gap-2 text-[11px] font-semibold tracking-wider text-zinc-600 dark:text-zinc-400"><ShieldCheck className="h-3.5 w-3.5 text-emerald-500" /><span>{t('navigation.title')}</span></div>
                <div className="flex items-center gap-1"><button type="button" onClick={() => void refreshWallets()} title={t('navigation.refreshAccounts')} className="rounded p-1 text-zinc-500 hover:bg-zinc-200 dark:hover:bg-zinc-800"><RefreshCw className={`h-3.5 w-3.5 ${loadingWallets ? 'animate-spin' : ''}`} /></button><button type="button" onClick={() => { setWalletError(''); setShowPinDialog(true); }} title={t('navigation.newAccount')} className="rounded p-1 text-zinc-500 hover:bg-zinc-200 dark:hover:bg-zinc-800"><Plus className="h-3.5 w-3.5" /></button></div>
            </header>

            <div className="flex-1 space-y-3 overflow-y-auto p-2">
                <section className="space-y-1.5">
                    <div className="flex items-center justify-between px-1"><h2 className="text-[10px] font-bold uppercase tracking-wider text-zinc-500">{t('navigation.localAccounts')}</h2><span className="font-mono text-[9px] text-zinc-400">{wallets.length}</span></div>
                    {wallets.length === 0 && <p className="rounded border border-dashed border-zinc-300 px-2 py-3 text-[10px] text-zinc-400 dark:border-zinc-700">{t('navigation.noWallets')}</p>}
                    {wallets.map((wallet) => { const active = wallet.active || wallet.path === activeWalletPath; return <button type="button" key={wallet.path} onClick={() => void selectWallet(wallet)} className={`flex w-full items-center gap-2 rounded border p-2 text-left transition-colors ${active ? 'border-emerald-500/40 bg-emerald-500/10' : 'border-zinc-200 bg-white hover:border-zinc-300 dark:border-zinc-800 dark:bg-zinc-950/50 dark:hover:border-zinc-700'}`}><Wallet className="h-3.5 w-3.5 shrink-0 text-emerald-500" /><span className="min-w-0 flex-1"><span className="block truncate font-mono text-[10px]">{wallet.account_number ?? t('navigation.unregistered')}</span><span className="block truncate text-[9px] text-zinc-400">{wallet.path}</span></span>{active && <span className="rounded bg-emerald-500/10 px-1 py-0.5 text-[8px] font-mono text-emerald-500">{t('navigation.active')}</span>}</button>; })}
                </section>

                <section className="space-y-1.5 border-t border-zinc-200/50 pt-3 dark:border-zinc-800/50"><h2 className="flex items-center gap-1.5 px-1 text-[10px] font-bold uppercase tracking-wider text-zinc-500"><Clock3 className="h-3 w-3" />{t('navigation.activity')}</h2>{activity.length === 0 ? <p className="px-1 py-2 text-[10px] text-zinc-400">{t('navigation.noActivity')}</p> : <div className="space-y-1">{activity.map((item, index) => <p key={`${item}-${index}`} className="rounded bg-zinc-100/70 px-2 py-1 text-[10px] dark:bg-zinc-950/40">{item}</p>)}</div>}</section>
            </div>

            {showPinDialog && <div className="border-t border-blue-500/30 bg-blue-500/5 p-2.5"><div className="mb-1 text-[10px] font-semibold">{t('navigation.newAccount')}</div><input autoFocus type="password" inputMode="numeric" maxLength={6} value={pin} onChange={(event) => setPin(event.target.value.replace(/\D/g, ''))} placeholder={t('navigation.pinPlaceholder')} className="mb-1.5 w-full rounded border border-zinc-200 bg-white px-2 py-1.5 text-[11px] dark:border-zinc-800 dark:bg-zinc-950" /><div className="flex gap-1"><button type="button" onClick={() => void createWallet()} className="flex-1 rounded bg-blue-600 px-2 py-1.5 text-[10px] text-white">{t('navigation.create')}</button><button type="button" onClick={() => setShowPinDialog(false)} className="rounded px-2 py-1.5 text-[10px] text-zinc-500">×</button></div></div>}
            {walletError && <p className="border-t border-rose-500/20 px-3 py-2 text-[10px] text-rose-500">{walletError}</p>}
            <footer className="flex items-center justify-between border-t border-zinc-200/80 bg-zinc-100/50 p-2 text-[10px] text-zinc-400 dark:border-zinc-800/80 dark:bg-zinc-950/30"><span>{t('navigation.workspace')}</span><span className="flex items-center gap-0.5 font-mono"><CheckCircle2 className="h-2.5 w-2.5 text-emerald-500" />{t('navigation.ready')}</span></footer>
        </aside>
    );
};
