import React, { useState, useEffect, useRef } from 'react';
import {
  ArrowLeft,
  Play,
  Square,
  Zap,
  Layers,
  Activity,
  Terminal,
  ExternalLink,
  Copy,
  Check,
  CheckCircle,
  Database,
  Cpu,
  Clock,
  ShieldCheck,
  RotateCw,
} from 'lucide-react';
import { HunterState, Puzzle, ScannedWindow, formatLargeNumber, formatCompactKeys, formatKeysSpeed } from '../types';

interface PuzzleDetailPageProps {
  puzzle: Puzzle;
  hunter?: HunterState;
  destinationAddress: string;
  onBack: () => void;
  onRefresh: () => void;
}

export const PuzzleDetailPage: React.FC<PuzzleDetailPageProps> = ({
  puzzle,
  hunter,
  destinationAddress,
  onBack,
  onRefresh,
}) => {
  const [windowBits, setWindowBits] = useState(30);
  const [loading, setLoading] = useState(false);
  const [copiedAddr, setCopiedAddr] = useState(false);
  const [copiedPub, setCopiedPub] = useState(false);
  const [windows, setWindows] = useState<ScannedWindow[]>([]);
  const [loadingWindows, setLoadingWindows] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);
  const logContainerRef = useRef<HTMLDivElement>(null);

  const isRunning = hunter?.status === 'Running';
  const isSolved = puzzle.is_solved || hunter?.status === 'SolvedByUs' || hunter?.status === 'SolvedByOther';

  // Perhitungan total keys dari perkalian block / window
  const isPubkeyExposed = Boolean(puzzle.pubkey_hex && puzzle.pubkey_hex.trim().length === 66);
  const effectiveBits = isPubkeyExposed ? windowBits : Math.min(windowBits, 18);
  const keysPerBlock = Math.pow(2, effectiveBits);
  const windowsCount = hunter?.windows_scanned || windows.length;
  const totalHistoricalKeys = windowsCount * keysPerBlock;

  // Fetch riwayat scanned windows dari PostgreSQL
  const fetchScannedWindows = async () => {
    setLoadingWindows(true);
    try {
      const res = await fetch(`/api/puzzles/${puzzle.puzzle_number}/windows`);
      if (res.ok) {
        const data = await res.json();
        setWindows(data.windows || []);
      }
    } catch (e) {
      console.error(e);
    } finally {
      setLoadingWindows(false);
    }
  };

  useEffect(() => {
    fetchScannedWindows();
  }, [puzzle.puzzle_number]);

  // Log real-time saat hunter mengabarkan window baru via WebSocket
  useEffect(() => {
    if (hunter && hunter.last_window_hex) {
      const now = new Date().toLocaleTimeString();
      const keysInWindow = hunter.total_keys_checked > 0
        ? (hunter.round > 0 ? Math.round(hunter.total_keys_checked / hunter.round) : 0)
        : 0;
      const speed = formatKeysSpeed(hunter.keys_per_second);
      const keysLabel = keysInWindow >= 1_000_000_000
        ? `${(keysInWindow / 1_000_000_000).toFixed(2)}B keys`
        : keysInWindow >= 1_000_000
        ? `${(keysInWindow / 1_000_000).toFixed(1)}M keys`
        : `${keysInWindow.toLocaleString()} keys`;
      const newLog = `[${now}] Ronde #${hunter.round} | ${keysLabel} | Speed: ${speed} | Window: ${hunter.last_window_hex} | MISS`;
      setLogs((prev) => {
        if (prev[prev.length - 1] === newLog) return prev;
        return [...prev.slice(-150), newLog];
      });
    }
  }, [hunter?.round, hunter?.last_window_hex]);

  // Auto scroll terminal log ke bawah
  useEffect(() => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [logs]);

  const handleStart = async () => {
    setLoading(true);
    try {
      await fetch(`/api/hunt/${puzzle.puzzle_number}/start`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          window_bits: windowBits,
          destination: destinationAddress,
        }),
      });
      onRefresh();
      fetchScannedWindows();
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const handleStop = async () => {
    setLoading(true);
    try {
      await fetch(`/api/hunt/${puzzle.puzzle_number}/stop`, { method: 'POST' });
      onRefresh();
      fetchScannedWindows();
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const copyAddress = () => {
    navigator.clipboard.writeText(puzzle.address);
    setCopiedAddr(true);
    setTimeout(() => setCopiedAddr(false), 2000);
  };

  const copyPubkey = () => {
    navigator.clipboard.writeText(puzzle.pubkey_hex);
    setCopiedPub(true);
    setTimeout(() => setCopiedPub(false), 2000);
  };

  return (
    <div className="space-y-6 pb-12 animate-in fade-in duration-200">
      {/* Top Navigation Bar */}
      <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 pb-4 border-b border-slate-800">
        <div className="flex items-center gap-3">
          <button
            onClick={onBack}
            className="p-2 bg-slate-900 border border-slate-800 hover:border-slate-700 text-slate-300 rounded-xl flex items-center gap-2 text-xs font-semibold transition"
          >
            <ArrowLeft className="w-4 h-4" />
            <span>Kembali</span>
          </button>

          <div>
            <div className="flex items-center gap-2">
              <h2 className="text-2xl font-black text-white tracking-tight">
                Bitcoin Puzzle #{puzzle.puzzle_number}
              </h2>
              <span className="px-2.5 py-0.5 rounded-full text-xs font-bold bg-slate-800 text-slate-300 border border-slate-700">
                {puzzle.bits}-bit
              </span>
              {isSolved ? (
                <span className="px-2.5 py-0.5 rounded-full text-xs font-bold bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 flex items-center gap-1">
                  <CheckCircle className="w-3.5 h-3.5" /> SOLVED
                </span>
              ) : isRunning ? (
                <span className="px-2.5 py-0.5 rounded-full text-xs font-bold bg-amber-500/20 text-amber-400 border border-amber-500/30 flex items-center gap-1.5 animate-pulse">
                  <span className="w-2 h-2 rounded-full bg-amber-400"></span>
                  RUNNING
                </span>
              ) : (
                <span className="px-2.5 py-0.5 rounded-full text-xs font-medium bg-slate-800/80 text-slate-400">
                  IDLE
                </span>
              )}
            </div>
            <p className="text-xs text-amber-400 font-medium mt-0.5">
              Hadiah: {puzzle.reward_btc > 0 ? `${puzzle.reward_btc} BTC` : 'TBA'}
            </p>
          </div>
        </div>

        {/* Action Button */}
        <div className="flex items-center gap-3 w-full sm:w-auto">
          {!isRunning && (
            <select
              value={windowBits}
              onChange={(e) => setWindowBits(Number(e.target.value))}
              disabled={isSolved || loading}
              className="bg-slate-900 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-300 focus:outline-none focus:border-amber-500"
            >
              <option value={20}>Window 2^20 (1 Juta keys)</option>
              <option value={24}>Window 2^24 (16 Juta keys)</option>
              <option value={28}>Window 2^28 (268 Juta keys)</option>
              <option value={30}>Window 2^30 (1 Miliar keys - Rekomendasi)</option>
              <option value={32}>Window 2^32 (4 Miliar keys)</option>
            </select>
          )}

          {isRunning ? (
            <button
              onClick={handleStop}
              disabled={loading}
              className="px-5 py-2.5 bg-rose-500/20 hover:bg-rose-500/30 text-rose-300 border border-rose-500/30 rounded-xl font-bold text-xs flex items-center gap-2 transition"
            >
              <Square className="w-4 h-4 fill-current" />
              <span>Jeda Hunter</span>
            </button>
          ) : (
            <button
              onClick={handleStart}
              disabled={isSolved || loading}
              className="px-6 py-2.5 bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold rounded-xl text-xs flex items-center gap-2 shadow-lg shadow-amber-500/10 transition disabled:opacity-40"
            >
              <Play className="w-4 h-4 fill-current" />
              <span>Mulai Hunt</span>
            </button>
          )}
        </div>
      </div>

      {/* Grid Metrics Live */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div className="p-4 bg-slate-900/60 border border-slate-800 rounded-2xl">
          <div className="flex items-center justify-between text-slate-400 text-xs mb-1">
            <div className="flex items-center gap-2">
              <Zap className="w-4 h-4 text-amber-400" />
              <span>Kecepatan Live</span>
            </div>
            {hunter?.active_workers && hunter.active_workers > 1 && (
              <span className="text-[10px] text-amber-400 font-bold bg-amber-500/10 px-1.5 py-0.5 rounded border border-amber-500/20" title={`Menggunakan ${hunter.active_workers} CPU worker threads paralel`}>
                ⚡ {hunter.active_workers} Core
              </span>
            )}
          </div>
          <div className="text-xl font-extrabold text-white font-mono">
            {hunter?.keys_per_second ? formatKeysSpeed(hunter.keys_per_second) : '0 k/s'}
          </div>
        </div>

        <div className="p-4 bg-slate-900/60 border border-slate-800 rounded-2xl">
          <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
            <Activity className="w-4 h-4 text-emerald-400" />
            <span>Ronde Berjalan</span>
          </div>
          <div className="text-xl font-extrabold text-white font-mono">
            #{hunter?.round || 0}
          </div>
        </div>

        <div className="p-4 bg-slate-900/60 border border-slate-800 rounded-2xl">
          <div className="flex items-center justify-between text-slate-400 text-xs mb-1">
            <div className="flex items-center gap-2">
              <Layers className="w-4 h-4 text-blue-400" />
              <span>Windows (Postgres)</span>
            </div>
          </div>
          <div className="text-xl font-extrabold text-white font-mono">
            {windowsCount.toLocaleString()}
          </div>
          <div
            className="text-xs text-blue-400 font-mono mt-1 flex items-center gap-1.5"
            title={`Total Kunci: ${totalHistoricalKeys.toLocaleString()} (${windowsCount.toLocaleString()} window × 2^${effectiveBits} keys)`}
          >
            <span className="text-slate-400 text-[11px]">Total Kunci:</span>
            <span className="font-bold">~{formatCompactKeys(totalHistoricalKeys)}</span>
          </div>
        </div>

        <div className="p-4 bg-slate-900/60 border border-slate-800 rounded-2xl">
          <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
            <Cpu className="w-4 h-4 text-purple-400" />
            <span>Total Kunci Sesi Ini</span>
          </div>
          <div
            className="text-xl font-extrabold text-white font-mono"
            title={hunter?.total_keys_checked ? `${hunter.total_keys_checked.toLocaleString()} keys` : '0 keys'}
          >
            {hunter?.total_keys_checked ? formatLargeNumber(hunter.total_keys_checked) : '0'}
          </div>
        </div>
      </div>

      {/* Box Window Hex Saat Ini */}
      {hunter?.last_window_hex && (
        <div className="p-4 bg-slate-950 border border-amber-500/30 rounded-2xl">
          <div className="flex items-center justify-between text-xs mb-1.5 flex-wrap gap-2">
            <span className="text-amber-400 font-semibold uppercase tracking-wider text-[11px] flex items-center gap-1.5">
              <span className="w-2 h-2 rounded-full bg-amber-400 animate-ping"></span>
              Window Sedang Diperiksa (Pollard's Kangaroo O(√N)):
            </span>
            <div className="flex items-center gap-3 text-[11px] text-slate-400">
              {hunter.round > 0 && hunter.total_keys_checked > 0 && (
                <span className="bg-slate-900 px-2 py-0.5 rounded-lg border border-slate-800">
                  ~{formatLargeNumber(Math.round(hunter.total_keys_checked / hunter.round))} keys/ronde
                </span>
              )}
              <span>{hunter.last_check_at}</span>
            </div>
          </div>
          <div className="font-mono text-xs sm:text-sm text-slate-200 bg-slate-900/80 p-3 rounded-xl border border-slate-800 break-all select-all">
            {hunter.last_window_hex}
          </div>
        </div>
      )}

      {/* Dua Kolom: Kiri Info Kriptografi, Kanan Terminal Log Live */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Kolom Kiri: Spesifikasi Puzzle */}
        <div className="bg-slate-900/50 border border-slate-800 rounded-2xl p-5 space-y-4">
          <h3 className="text-sm font-bold text-white uppercase tracking-wider flex items-center gap-2 border-b border-slate-800 pb-3">
            <ShieldCheck className="w-4 h-4 text-amber-400" />
            Spesifikasi Kriptografi & Blockchain
          </h3>

          <div className="space-y-3 text-xs">
            <div>
              <div className="text-[11px] text-slate-500 font-semibold uppercase mb-0.5">Bitcoin Address</div>
              <div className="flex items-center gap-2">
                <span className="font-mono text-slate-200">{puzzle.address}</span>
                <button onClick={copyAddress} className="text-slate-400 hover:text-white p-1">
                  {copiedAddr ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                </button>
                <a
                  href={`https://www.blockchain.com/explorer/addresses/btc/${puzzle.address}`}
                  target="_blank"
                  rel="noreferrer"
                  className="text-slate-500 hover:text-amber-400 transition"
                  title="Lihat di Blockchain.com Explorer"
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                </a>
              </div>
            </div>

            <div>
              <div className="flex items-center justify-between text-[11px] text-slate-500 font-semibold uppercase mb-0.5">
                <span>Compressed Public Key</span>
                {puzzle.pubkey_hex && puzzle.pubkey_hex.length === 66 ? (
                  <span className="text-emerald-400 font-normal">Terekspos (Kangaroo O(√N))</span>
                ) : (
                  <span className="text-amber-400 font-normal">Belum Terekspos (Address Scanner)</span>
                )}
              </div>
              <div className="flex items-center gap-2">
                <span className="font-mono text-slate-300 break-all bg-slate-950 p-2 rounded-lg border border-slate-800 text-[11px] w-full">
                  {puzzle.pubkey_hex && puzzle.pubkey_hex.length === 66 ? (
                    puzzle.pubkey_hex
                  ) : (
                    <span className="text-amber-400/80 font-sans italic">Public key belum terekspos — menggunakan mode pencarian Address Hash160</span>
                  )}
                </span>
                {puzzle.pubkey_hex && puzzle.pubkey_hex.length === 66 && (
                  <button onClick={copyPubkey} className="text-slate-400 hover:text-white p-1 shrink-0">
                    {copiedPub ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                  </button>
                )}
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3 pt-2">
              <div>
                <div className="text-[11px] text-slate-500 font-semibold uppercase mb-0.5">Range Start (HEX)</div>
                <div className="font-mono text-[11px] text-slate-300 bg-slate-950 p-2 rounded-lg border border-slate-800 break-all">
                  {puzzle.range_start_hex}
                </div>
              </div>
              <div>
                <div className="text-[11px] text-slate-500 font-semibold uppercase mb-0.5">Range End (HEX)</div>
                <div className="font-mono text-[11px] text-slate-300 bg-slate-950 p-2 rounded-lg border border-slate-800 break-all">
                  {puzzle.range_end_hex}
                </div>
              </div>
            </div>

            <div className="p-3 bg-slate-950/70 border border-slate-800 rounded-xl space-y-1 text-slate-400 text-[11px]">
              <div className="flex items-center gap-1.5 text-emerald-400 font-semibold">
                <Clock className="w-3.5 h-3.5" />
                <span>On-Chain Watchdog Aktif:</span>
              </div>
              <p>
                Sistem memeriksa status dompet puzzle setiap 5 menit ke API blockchain. Jika saldo ditarik oleh orang lain, hunter otomatis dihentikan agar menghemat CPU server.
              </p>
            </div>
          </div>
        </div>

        {/* Kolom Kanan: Live Terminal Log Console */}
        <div className="bg-slate-900/50 border border-slate-800 rounded-2xl p-5 flex flex-col h-full min-h-[350px]">
          <div className="flex items-center justify-between border-b border-slate-800 pb-3 mb-3">
            <h3 className="text-sm font-bold text-white uppercase tracking-wider flex items-center gap-2">
              <Terminal className="w-4 h-4 text-emerald-400" />
              Live WebSocket Terminal Stream
            </h3>
            <span className="text-[10px] text-slate-500 font-mono">Stream Real-time</span>
          </div>

          <div
            ref={logContainerRef}
            className="flex-1 bg-black/80 rounded-xl p-3 font-mono text-[11px] text-slate-300 overflow-y-auto space-y-1.5 max-h-[320px] border border-slate-800/80"
          >
            {logs.length === 0 ? (
              <div className="text-slate-600 text-center py-16">
                Menunggu hunter dimulai... Log ronde akan otomatis mengalir di sini via WebSocket.
              </div>
            ) : (
              logs.map((log, i) => (
                <div key={i} className="hover:text-amber-300 transition-colors">
                  {log}
                </div>
              ))
            )}
          </div>
        </div>
      </div>

      {/* Tabel Riwayat Scanned Windows dari PostgreSQL */}
      <div className="bg-slate-900/50 border border-slate-800 rounded-2xl p-5 space-y-4">
        <div className="flex items-center justify-between border-b border-slate-800 pb-3">
          <div className="flex items-center gap-2">
            <Database className="w-4 h-4 text-blue-400" />
            <h3 className="text-sm font-bold text-white uppercase tracking-wider">
              Riwayat Scanned Windows di PostgreSQL (Anti Duplikasi)
            </h3>
            <span className="text-xs bg-slate-800 px-2 py-0.5 rounded-full text-slate-400">
              {windows.length} entri terbaru
            </span>
          </div>

          <button
            onClick={fetchScannedWindows}
            disabled={loadingWindows}
            className="p-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg text-xs flex items-center gap-1.5 transition"
            title="Refresh Riwayat dari PostgreSQL"
          >
            <RotateCw className={`w-3.5 h-3.5 ${loadingWindows ? 'animate-spin' : ''}`} />
            <span>Refresh</span>
          </button>
        </div>

        {windows.length === 0 ? (
          <div className="text-center py-8 text-slate-500 text-xs">
            Belum ada window yang discan untuk puzzle ini di database PostgreSQL.
          </div>
        ) : (
          <div className="overflow-x-auto max-h-80 overflow-y-auto">
            <table className="w-full text-left text-xs font-mono">
              <thead className="bg-slate-950/80 text-slate-400 sticky top-0 border-b border-slate-800">
                <tr>
                  <th className="py-2 px-3">#ID</th>
                  <th className="py-2 px-3">Range Start (HEX)</th>
                  <th className="py-2 px-3">Range End (HEX)</th>
                  <th className="py-2 px-3">Kunci Diuji</th>
                  <th className="py-2 px-3">Durasi</th>
                  <th className="py-2 px-3">Waktu Simpan</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800/50 text-slate-300">
                {windows.map((w) => (
                  <tr key={w.id} className="hover:bg-slate-800/40 transition">
                    <td className="py-2 px-3 text-slate-500">#{w.id}</td>
                    <td className="py-2 px-3 text-amber-300/80 truncate max-w-xs">{w.range_start_hex}</td>
                    <td className="py-2 px-3 text-slate-400 truncate max-w-xs">{w.range_end_hex}</td>
                    <td className="py-2 px-3 text-slate-200">{w.keys_checked.toLocaleString()}</td>
                    <td className="py-2 px-3 text-emerald-400">{w.duration_secs.toFixed(3)}s</td>
                    <td className="py-2 px-3 text-slate-500 text-[11px]">
                      {new Date(w.created_at).toLocaleTimeString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
};

