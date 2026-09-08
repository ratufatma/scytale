import { invoke } from '@tauri-apps/api/core';
import { Send, ShieldCheck } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

interface CenterWorkspaceProps {
    activeWalletPath: string | null;
}

interface AccountDetails {
    account_number: string | null;
    passbook_id: string | null;
}

const QUANTA_PER_SCY = 100_000_000;

export function CenterWorkspace({ activeWalletPath }: CenterWorkspaceProps) {
    const { t } = useTranslation();
    const [sender, setSender] = useState('');
    const [recipient, setRecipient] = useState('');
    const [amount, setAmount] = useState('');
    const [fee, setFee] = useState('60');
    const [draft, setDraft] = useState<Record<string, unknown> | null>(null);
    const [pin, setPin] = useState('');
    const [showPin, setShowPin] = useState(false);
    const [status, setStatus] = useState('');
    const [working, setWorking] = useState(false);

    useEffect(() => {
        if (!activeWalletPath) { setSender(''); return; }
        void invoke<AccountDetails>('get_active_account_details', { walletPath: activeWalletPath })
            .then((details) => setSender(details.account_number ?? details.passbook_id ?? ''))
            .catch(() => setSender(''));
    }, [activeWalletPath]);

    const amountQuanta = useMemo(() => {
        const parsed = Number(amount.replace(',', '.'));
        return Number.isFinite(parsed) && parsed > 0 ? Math.round(parsed * QUANTA_PER_SCY) : 0;
    }, [amount]);
    const totalQuanta = amountQuanta + Number(fee);
    const validRecipient = (recipient.startsWith('SCY-') && recipient.length === 10) || recipient.startsWith('scy1');

    const createDraft = async () => {
        setStatus('');
        try {
            const result = await invoke<Record<string, unknown>>('draft_transaction', { sender, recipient, amountQuanta, feeQuanta: Number(fee) });
            setDraft(result);
            setStatus(t('workspace.draftReady'));
        } catch (error) { setStatus(String(error)); }
    };

    const signAndBroadcast = async () => {
        if (!activeWalletPath) { setStatus(t('workspace.walletRequired')); return; }
        let currentDraft = draft;
        if (!currentDraft) {
            try {
                currentDraft = await invoke<Record<string, unknown>>('draft_transaction', { sender, recipient, amountQuanta, feeQuanta: Number(fee) });
                setDraft(currentDraft);
            } catch (error) { setStatus(String(error)); return; }
        }
        if (!/^\d{6}$/.test(pin)) { setStatus(t('workspace.pinInvalid')); return; }
        setWorking(true);
        try {
            const result = await invoke<Record<string, unknown>>('sign_and_broadcast_transaction', { walletPath: activeWalletPath, pin, draft: currentDraft, nodeUrl: null });
            setStatus(`${t('workspace.broadcastSuccess')}: ${JSON.stringify(result)}`);
            setShowPin(false); setPin('');
        } catch (error) { setStatus(String(error)); }
        finally { setWorking(false); }
    };

    return (
        <section className="flex min-h-0 flex-1 flex-col overflow-hidden bg-white text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
            <div className="flex-1 overflow-y-auto p-4 md:p-6"><div className="mx-auto max-w-4xl space-y-4">
                <div className="flex items-start justify-between"><div><p className="font-mono text-[10px] uppercase tracking-[0.2em] text-blue-500">{t('workspace.transactionCanvas')}</p><h2 className="mt-1 text-xl font-semibold">{t('workspace.assembleTitle')}</h2><p className="mt-1 text-xs text-zinc-500">{t('workspace.assembleDescription')}</p></div><ShieldCheck className="h-6 w-6 text-emerald-500" /></div>
                <div className="grid gap-4 lg:grid-cols-[1fr_0.8fr]">
                    <div className="space-y-3 rounded-xl border border-zinc-200 bg-zinc-50/60 p-4 dark:border-zinc-800 dark:bg-zinc-900/40">
                        <label className="block text-xs font-medium">{t('workspace.sender')}<input readOnly value={sender} placeholder={t('workspace.noWallet')} className="mt-1 w-full rounded border border-zinc-200 bg-zinc-100 px-3 py-2 font-mono text-xs dark:border-zinc-800 dark:bg-zinc-950" /></label>
                        <label className="block text-xs font-medium">{t('workspace.recipient')}<input value={recipient} onChange={(event) => setRecipient(event.target.value)} placeholder="SCY-xxxxxx or scy1..." className={`mt-1 w-full rounded border px-3 py-2 font-mono text-xs dark:bg-zinc-950 ${recipient && !validRecipient ? 'border-rose-500' : 'border-zinc-200 dark:border-zinc-800'}`} /></label>
                        <div className="grid gap-3 sm:grid-cols-2"><label className="block text-xs font-medium">{t('workspace.amount')}<input type="text" inputMode="decimal" value={amount} onChange={(event) => setAmount(event.target.value)} placeholder="0.50000000" className="mt-1 w-full rounded border border-zinc-200 bg-white px-3 py-2 font-mono text-xs dark:border-zinc-800 dark:bg-zinc-950" /><span className="mt-1 block font-mono text-[10px] text-zinc-500">= {amountQuanta.toLocaleString()} quanta</span></label><label className="block text-xs font-medium">{t('workspace.fee')}<select value={fee} onChange={(event) => setFee(event.target.value)} className="mt-1 w-full rounded border border-zinc-200 bg-white px-3 py-2 text-xs dark:border-zinc-800 dark:bg-zinc-950"><option value="60">{t('workspace.normalFee')}</option><option value="120">{t('workspace.fastFee')}</option></select></label></div>
                        <div className="flex items-center justify-between border-t border-zinc-200 pt-3 text-xs dark:border-zinc-800"><span>{t('workspace.total')}</span><span className="font-mono font-semibold">{(totalQuanta / QUANTA_PER_SCY).toFixed(8)} SCY · {totalQuanta.toLocaleString()} quanta</span></div>
                        <div className="flex flex-wrap gap-2"><button type="button" disabled={!sender || !validRecipient || amountQuanta === 0} onClick={() => void createDraft()} className="rounded bg-zinc-900 px-3 py-2 text-xs font-medium text-white disabled:opacity-40 dark:bg-zinc-100 dark:text-zinc-950">{t('workspace.draftPayload')}</button><button type="button" disabled={working || !sender || !validRecipient || amountQuanta === 0} onClick={() => setShowPin(true)} className="flex items-center gap-1.5 rounded bg-blue-600 px-3 py-2 text-xs font-medium text-white disabled:opacity-40"><Send className="h-3.5 w-3.5" />{t('workspace.signBroadcast')}</button></div>
                        {status && <p className="break-all rounded border border-blue-500/30 bg-blue-500/5 p-2 text-[11px] text-blue-600 dark:text-blue-300">{status}</p>}
                    </div>
                    <div className="rounded-xl border border-zinc-200 bg-zinc-950 p-4 text-zinc-100 dark:border-zinc-800"><div className="mb-2 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">{t('workspace.payloadInspector')}</div><pre className="max-h-80 overflow-auto whitespace-pre-wrap font-mono text-[10px] leading-relaxed text-emerald-300">{draft ? JSON.stringify(draft, null, 2) : t('workspace.noDraft')}</pre></div>
                </div>
            </div></div>
            {showPin && <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"><div className="w-full max-w-sm space-y-3 rounded-xl border border-zinc-200 bg-white p-5 shadow-xl dark:border-zinc-800 dark:bg-zinc-900"><h3 className="font-semibold">{t('workspace.confirmTitle')}</h3><p className="text-xs text-zinc-500">{t('workspace.confirmDescription')}</p><input autoFocus type="password" inputMode="numeric" maxLength={6} value={pin} onChange={(event) => setPin(event.target.value.replace(/\D/g, ''))} placeholder="••••••" className="w-full rounded border border-zinc-200 px-3 py-2 text-center font-mono tracking-[0.5em] dark:border-zinc-700 dark:bg-zinc-950" /><div className="flex justify-end gap-2"><button type="button" onClick={() => setShowPin(false)} className="rounded px-3 py-2 text-xs text-zinc-500">{t('workspace.cancel')}</button><button type="button" onClick={() => void signAndBroadcast()} className="rounded bg-blue-600 px-3 py-2 text-xs text-white">{t('workspace.confirm')}</button></div></div></div>}
        </section>
    );
}
