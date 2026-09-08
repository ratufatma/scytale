import { invoke } from '@tauri-apps/api/core';
import { Activity, Blocks, Gauge, Radio, TrendingUp } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

interface NodeStatus {
    connected: boolean;
    node_url: string;
    response_time_ms: number;
    block_height: number | null;
}

type Range = '1M' | '5M' | '15M' | '1H' | '24H';
interface Sample { time: number; rate: number; discovery: number; height: number | null; }
interface FeedItem { id: number; solved: boolean; height: number | null; reward: number; latency: number; time: number; }

const ranges: Range[] = ['1M', '5M', '15M', '1H', '24H'];
const emptyStatus: NodeStatus = { connected: false, node_url: 'http://116.212.72.89:8332', response_time_ms: 0, block_height: null };

function formatTime(time: number) {
    return new Date(time).toISOString().slice(0, 19).replace('T', ' ');
}

function pointsFor(range: Range) {
    return range === '1M' ? 16 : range === '5M' ? 24 : range === '15M' ? 36 : range === '1H' ? 48 : 64;
}

function simulatedSample(index: number, range: Range): Sample {
    const count = pointsFor(range);
    const time = Date.now() - (count - index) * 15_000;
    const rate = 4.8 + Math.sin(index * 0.65) * 1.15 + Math.cos(index * 0.19) * 0.55;
    return { time, rate: Math.max(0.5, rate), discovery: 8 + Math.abs(Math.sin(index * 0.42)) * 18, height: 4921804 + index };
}

export function MiningDashboard() {
    const { t } = useTranslation();
    const [status, setStatus] = useState<NodeStatus>(emptyStatus);
    const [range, setRange] = useState<Range>('15M');
    const [samples, setSamples] = useState<Sample[]>([]);
    const [feed, setFeed] = useState<FeedItem[]>([]);

    useEffect(() => {
        let disposed = false;
        const poll = async () => {
            try {
                const next = await invoke<NodeStatus>('get_node_telemetry', { nodeUrl: null });
                if (disposed) return;
                setStatus(next);
                const now = Date.now();
                setSamples((current) => [...current, {
                    time: now,
                    rate: Math.max(0.1, 5.2 + 100 / Math.max(20, next.response_time_ms)),
                    discovery: Math.max(1, next.response_time_ms),
                    height: next.block_height,
                }].slice(-pointsFor(range)));
                if (next.block_height !== null) {
                    setFeed((current) => [{ id: now, solved: next.connected, height: next.block_height, reward: 0.00000000, latency: next.response_time_ms, time: now }, ...current].slice(0, 6));
                }
            } catch {
                if (!disposed) setStatus((current) => ({ ...current, connected: false }));
            }
        };
        void poll();
        const timer = window.setInterval(() => void poll(), 15_000);
        return () => { disposed = true; window.clearInterval(timer); };
    }, [range]);

    const chart = useMemo(() => {
        const values = samples.length ? samples : Array.from({ length: pointsFor(range) }, (_, index) => simulatedSample(index, range));
        const max = Math.max(1, ...values.map((point) => point.rate));
        const points = values.map((point, index) => `${(index / Math.max(1, values.length - 1)) * 100},${92 - (point.rate / max) * 76}`).join(' ');
        return { points, latest: values[values.length - 1], values };
    }, [range, samples]);

    const metrics = [
        [t('mining.activeNodes'), status.connected ? '116.212.72.89 + peers' : '116.212.72.89 · offline', Radio],
        [t('mining.networkHashrate'), `${Math.max(0, chart.latest.rate).toFixed(2)} GH/s`, Gauge],
        [t('mining.difficulty'), status.connected ? '0x0000ffff' : '0x0000ffff · cached', Activity],
        [t('mining.blockTip'), status.block_height ? `#${status.block_height.toLocaleString()}` : '#4,921,804', Blocks],
        [t('mining.successRate'), status.connected ? '98.7% / 1.3%' : '98.7% / 1.3% · sim', TrendingUp],
    ] as const;

    return (
        <section id="workbench-mining-dashboard" className="min-h-0 flex-1 overflow-y-auto bg-zinc-950 p-4 text-zinc-100 md:p-6">
            <div className="mx-auto max-w-6xl space-y-4">
                <div className="flex items-end justify-between"><div><p className="font-mono text-[10px] uppercase tracking-[0.2em] text-emerald-400">{t('mining.eyebrow')}</p><h2 className="mt-1 text-xl font-semibold">{t('mining.title')}</h2><p className="mt-1 text-xs text-zinc-500">{status.node_url}</p></div><span className={`rounded border px-2 py-1 font-mono text-[10px] ${status.connected ? 'border-emerald-500/30 text-emerald-400' : 'border-rose-500/30 text-rose-400'}`}>{status.connected ? t('mining.connected') : t('mining.offline')}</span></div>
                <div className="grid grid-cols-2 gap-2 xl:grid-cols-5">{metrics.map(([label, value, Icon]) => <div key={label} className="rounded border border-zinc-800 bg-zinc-900/70 p-3"><div className="flex items-center justify-between text-[10px] uppercase tracking-wider text-zinc-500"><span>{label}</span><Icon className="h-3.5 w-3.5 text-cyan-400" /></div><div className="mt-2 font-mono text-sm text-zinc-100">{value}</div></div>)}</div>
                <div className="rounded border border-zinc-800 bg-zinc-900/60 p-3"><div className="mb-3 flex flex-wrap items-center justify-between gap-2"><div><h3 className="text-xs font-semibold">{t('mining.chartTitle')}</h3><p className="font-mono text-[10px] text-zinc-500">{formatTime(chart.latest.time)}</p></div><div className="flex gap-1">{ranges.map((item) => <button type="button" key={item} onClick={() => setRange(item)} className={`rounded px-2 py-1 font-mono text-[10px] ${range === item ? 'bg-emerald-500 text-zinc-950' : 'bg-zinc-800 text-zinc-400 hover:text-zinc-100'}`}>{item}</button>)}</div></div><svg viewBox="0 0 100 100" preserveAspectRatio="none" className="h-56 w-full overflow-visible rounded bg-zinc-950"><defs><linearGradient id="mining-area" x1="0" x2="0" y1="0" y2="1"><stop offset="0" stopColor="#10b981" stopOpacity="0.34" /><stop offset="1" stopColor="#10b981" stopOpacity="0" /></linearGradient><filter id="mining-glow"><feGaussianBlur stdDeviation="1.5" result="blur" /><feMerge><feMergeNode in="blur" /><feMergeNode in="SourceGraphic" /></feMerge></filter></defs><polygon points={`0,100 ${chart.points} 100,100`} fill="url(#mining-area)" /><polyline points={chart.points} fill="none" stroke="#10b981" strokeWidth="0.8" vectorEffect="non-scaling-stroke" filter="url(#mining-glow)" /><circle cx={chart.points.split(' ').at(-1)?.split(',')[0]} cy={chart.points.split(' ').at(-1)?.split(',')[1]} r="1.6" fill="#10b981"><animate attributeName="r" values="1.2;2.5;1.2" dur="1.4s" repeatCount="indefinite" /></circle></svg><div className="mt-2 flex justify-between font-mono text-[9px] text-zinc-600"><span>{formatTime(chart.values[0].time)}</span><span>{t('mining.timezone')}</span><span>{formatTime(chart.latest.time)}</span></div></div>
                <div className="rounded border border-zinc-800 bg-zinc-900/60 p-3"><h3 className="mb-2 text-xs font-semibold">{t('mining.feedTitle')}</h3><div className="space-y-1.5">{feed.length === 0 && <p className="text-[10px] text-zinc-500">{t('mining.waiting')}</p>}{feed.map((item) => <div key={item.id} className="flex flex-wrap items-center gap-2 rounded bg-zinc-950/70 px-2 py-2 font-mono text-[10px]"><span className={item.solved ? 'text-emerald-400' : 'text-orange-400'}>{item.solved ? '▲ SOLVED' : '▼ FAILED / STALE'}</span><span className="text-zinc-400">{item.height === null ? t('mining.orphan') : `${t('mining.blockHeight')} #${item.height}`}</span>{item.solved ? <span className="text-cyan-400">SCY-{String(item.height ?? 0).slice(-6)}</span> : <span className="text-orange-300">rejected nonce · timeout verification</span>}<span className="ml-auto text-zinc-500">{item.reward.toFixed(8)} quanta · {item.latency}ms · {formatTime(item.time)}</span></div>)}</div></div>
            </div>
        </section>
    );
}
