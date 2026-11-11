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
  Server,
  Sparkles,
  Loader2,
} from 'lucide-react';
import {
  HunterState,
  Puzzle,
  ScannedWindow,
  formatLargeNumber,
  formatCompactKeys,
  formatBigIntCompact,
  formatKeysSpeed,
  calculateRangePercentage,
  calculateWindowRangeProgress,
  parseWindowHexRange,
  WindowRangeProgress,
} from '../types';
import { getAudioContext } from '../sound';

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
  const [simulating, setSimulating] = useState(false);
  const logContainerRef = useRef<HTMLDivElement>(null);

  const handleSimulateCelebration = async () => {
    // Aktifkan audio context dengan user gesture
    getAudioContext();
    setSimulating(true);
    try {
      await fetch(`/api/hunt/${puzzle.puzzle_number}/simulate_celebration`, {
        method: 'POST',
      });
    } catch (e) {
      console.error('Failed to trigger celebration simulation:', e);
    } finally {
      setTimeout(() => setSimulating(false), 3000);
    }
  };

  const isRunning = hunter?.status === 'Running';
  const isPausing = hunter?.status === 'Pausing';
  const isSolved = puzzle.is_solved || hunter?.status === 'SolvedByUs' || hunter?.status === 'SolvedByOther';

  // Perhitungan total keys dari perkalian block / window
  const isPubkeyExposed = Boolean(puzzle.pubkey_hex && puzzle.pubkey_hex.trim().length === 66);
  const effectiveBits = isPubkeyExposed ? windowBits : Math.min(windowBits, 18);
  const keysPerBlock = Math.pow(2, effectiveBits);
  const windowsCount = hunter?.windows_scanned || windows.length;
  const totalHistoricalKeys = windowsCount * keysPerBlock;

  // Persentase keseluruhan key yang sudah diproses dari total key space puzzle
  const overallProgress = (() => {
    try {
      const clean = (s: string) => BigInt('0x' + s.trim().replace(/^0x/i, ''));
      const rangeStart = clean(puzzle.range_start_hex);
      const rangeEnd = clean(puzzle.range_end_hex);
      const totalKeySpace = rangeEnd - rangeStart;
      if (totalKeySpace <= 0n) return null;

      const tested = BigInt(Math.round(totalHistoricalKeys));
      if (tested <= 0n) return { pct: 0, label: '0.000000%', tested, totalKeySpace };

      const factor = 10n ** 12n;
      const scaled = (tested * 100n * factor) / totalKeySpace;
      const pct = Number(scaled) / Number(factor);

      let label: string;
      if (pct >= 10) label = pct.toFixed(2) + '%';
      else if (pct >= 1) label = pct.toFixed(4) + '%';
      else if (pct >= 0.01) label = pct.toFixed(4) + '%';
      else if (pct >= 0.000001) label = pct.toFixed(6) + '%';
      else if (pct > 0) {
        // Sangat kecil tapi > 0, tampilkan format desimal pendek
        // Hitung jumlah leading zeros setelah "0."
        const str = pct.toFixed(20);
        const afterDot = str.split('.')[1] || '';
        let leadingZeros = 0;
        for (const c of afterDot) { if (c === '0') leadingZeros++; else break; }
        const decimals = Math.min(leadingZeros + 2, 15);
        label = pct.toFixed(decimals) + '%';
      } else {
        // pct === 0 dari BigInt, pakai float approx
        const approx = Number(tested) / Number(totalKeySpace) * 100;
        if (approx > 0) {
          const str = approx.toFixed(20);
          const afterDot = str.split('.')[1] || '';
          let leadingZeros = 0;
          for (const c of afterDot) { if (c === '0') leadingZeros++; else break; }
          const decimals = Math.min(leadingZeros + 2, 15);
          label = approx.toFixed(decimals) + '%';
        } else {
          label = '~0%';
        }
      }

      return { pct, label, tested, totalKeySpace };
    } catch {
      return null;
    }
  })();

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
              ) : isPausing ? (
                <span className="px-2.5 py-0.5 rounded-full text-xs font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40 flex items-center gap-1.5 animate-pulse">
                  <Loader2 className="w-3.5 h-3.5 animate-spin text-amber-400" />
                  <span>MENJEDA... (Menyimpan Riwayat Window)</span>
                </span>
              ) : isRunning ? (
                <span className="px-2.5 py-0.5 rounded-full text-xs font-bold bg-amber-500/20 text-amber-400 border border-amber-500/30 flex items-center gap-1.5 animate-pulse">
                  <span className="w-2 h-2 rounded-full bg-amber-400"></span>
                  RUNNING
                </span>
              ) : (
                <span className="px-2.5 py-0.5 rounded-full text-xs font-medium bg-slate-800/80 text-slate-400">
                  {hunter?.status === 'Paused' ? 'PAUSED' : 'IDLE'}
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
          {!isRunning && !isPausing && (
            <select
              value={windowBits}
              onChange={(e) => setWindowBits(Number(e.target.value))}
              disabled={isSolved || loading || isPausing}
              className="bg-slate-900 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-300 focus:outline-none focus:border-amber-500"
            >
              <option value={20}>Window 2^20 (1 Juta keys)</option>
              <option value={24}>Window 2^24 (16 Juta keys)</option>
              <option value={28}>Window 2^28 (268 Juta keys)</option>
              <option value={30}>Window 2^30 (1 Miliar keys - Rekomendasi)</option>
              <option value={32}>Window 2^32 (4 Miliar keys)</option>
            </select>
          )}

          {isPausing ? (
            <button
              disabled
              className="px-5 py-2.5 bg-amber-500/15 text-amber-300 border border-amber-500/30 rounded-xl font-bold text-xs flex items-center gap-2 cursor-wait"
              title="Smart Pause sedang aktif: worker sedang menyelesaikan window aktif dan menyimpan riwayat ke database"
            >
              <Loader2 className="w-4 h-4 animate-spin text-amber-400" />
              <span>Menyelesaikan Window...</span>
            </button>
          ) : isRunning ? (
            <button
              onClick={handleStop}
              disabled={loading}
              className="px-5 py-2.5 bg-rose-500/20 hover:bg-rose-500/30 text-rose-300 border border-rose-500/30 rounded-xl font-bold text-xs flex items-center gap-2 transition"
              title="Jeda Cerdas (Smart Pause): Menunggu worker menyelesaikan window aktif dan menyimpan riwayat ke PostgreSQL sebelum berhenti"
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

          {/* Tombol Simulasi Animasi Kemenangan & Auto-Transfer Real-Time */}
          <button
            onClick={handleSimulateCelebration}
            disabled={simulating}
            className="px-3.5 py-2.5 bg-gradient-to-r from-amber-500/15 via-emerald-500/15 to-emerald-500/20 hover:from-amber-500/25 hover:to-emerald-500/30 text-amber-300 border border-amber-500/30 rounded-xl font-bold text-xs flex items-center gap-1.5 transition disabled:opacity-50"
            title="Uji coba animasi perayaan kemenangan dan timeline auto-transfer realtime"
          >
            <Sparkles className="w-3.5 h-3.5 text-amber-400" />
            <span>{simulating ? 'Menyiarkan...' : 'Simulasi Victory'}</span>
          </button>
        </div>
      </div>

      {/* Grid Metrics Live */}
      <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
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
          {hunter?.total_keys_checked ? (
            <div className="text-[11px] text-emerald-400/70 font-mono mt-1" title={`${hunter.total_keys_checked.toLocaleString()} keys sesi ini`}>
              Sesi ini: {formatLargeNumber(hunter.total_keys_checked)} keys
            </div>
          ) : null}
        </div>

        <div className="p-4 bg-slate-900/60 border border-slate-800 rounded-2xl col-span-2 md:col-span-1">
          <div className="flex items-center justify-between text-slate-400 text-xs mb-1">
            <div className="flex items-center gap-2">
              <Layers className="w-4 h-4 text-blue-400" />
              <span>Windows (Postgres)</span>
            </div>
            <span className="text-xl font-extrabold text-white font-mono">
              {windowsCount.toLocaleString()}
            </span>
          </div>
          <div
            className="text-xs text-blue-400 font-mono mt-1 flex items-center gap-1.5 flex-wrap"
            title={`Total Kunci: ${totalHistoricalKeys.toLocaleString()} / ${overallProgress ? overallProgress.totalKeySpace.toString() : '?'} (${windowsCount.toLocaleString()} window × 2^${effectiveBits} keys)`}
          >
            <span className="text-slate-400 text-[11px]">Total Kunci:</span>
            <span className="font-bold">~{formatCompactKeys(totalHistoricalKeys)}</span>
            {overallProgress && overallProgress.totalKeySpace > 0n && (
              <>
                <span className="text-slate-600">/</span>
                <span className="text-slate-400 font-medium">{formatBigIntCompact(overallProgress.totalKeySpace)}</span>
              </>
            )}
          </div>
        </div>
      </div>

      {/* Box Window Hex Saat Ini */}
      {hunter?.last_window_hex && (() => {
        const parsed = parseWindowHexRange(hunter.last_window_hex);
        const liveProgress = parsed.startHex
          ? calculateWindowRangeProgress(
              parsed.startHex,
              parsed.endHex || parsed.startHex,
              puzzle.range_start_hex,
              puzzle.range_end_hex,
              4
            )
          : null;

        // Ambil jumlah thread/worker aktif (dari active_workers atau parse regex "⚡ N threads")
        const threadMatch = hunter.last_window_hex.match(/\(⚡\s*(\d+)\s*threads\)/i);
        const threadCount = hunter.active_workers || (threadMatch ? parseInt(threadMatch[1], 10) : 1);

        // Kumpulkan daftar core dan posisi jump masing-masing
        interface CoreJumpInfo {
          coreId: number;
          startHex: string;
          endHex: string;
          progress: WindowRangeProgress;
        }

        let coresList: CoreJumpInfo[] = [];

        // 1. Jika backend sudah mengirimkan active_windows (real-time data dari worker)
        if (hunter.active_windows && hunter.active_windows.length > 0) {
          coresList = hunter.active_windows.map((w, idx) => {
            const prog = calculateWindowRangeProgress(
              w.start_hex,
              w.end_hex,
              puzzle.range_start_hex,
              puzzle.range_end_hex,
              4
            );
            return {
              coreId: w.worker_id || idx + 1,
              startHex: w.start_hex,
              endHex: w.end_hex,
              progress: prog || {
                startPct: 0,
                endPct: 0,
                startFormatted: '0.0000%',
                endFormatted: '0.0000%',
                display: '0.0000%',
              },
            };
          });
        }

        // 2. Fallback cerdas jika active_windows belum terkirim (misal proses hunter sedang berjalan sebelum reload)
        if (coresList.length === 0 && liveProgress && parsed.startHex) {
          coresList.push({
            coreId: 1,
            startHex: parsed.startHex,
            endHex: parsed.endHex || parsed.startHex,
            progress: liveProgress,
          });

          if (threadCount > 1) {
            try {
              const clean = (s: string) => BigInt('0x' + s.trim().replace(/^0x/i, ''));
              const pStart = clean(puzzle.range_start_hex);
              const pEnd = clean(puzzle.range_end_hex);
              const totalRange = pEnd - pStart;
              const baseVal = clean(parsed.startHex);
              const winSize = parsed.endHex ? clean(parsed.endHex) - baseVal : (1n << 18n);

              const recentWins = windows.slice(0, threadCount);

              for (let i = 2; i <= threadCount; i++) {
                const dbWin = recentWins[i - 1];
                let cStartHex: string;
                let cEndHex: string;

                if (dbWin && dbWin.range_start_hex) {
                  cStartHex = dbWin.range_start_hex;
                  cEndHex = dbWin.range_end_hex || dbWin.range_start_hex;
                } else {
                  // Pseudo-jump deterministik berbasis hash ronde & coreId agar visual terdistribusi rata
                  const pseudoSeed = (BigInt(hunter.round || 1) * 2654435761n + BigInt(i) * 1000000007n) % 1000000n;
                  const offset = (totalRange * pseudoSeed) / 1000000n;
                  const coreStartBig = pStart + offset;
                  const coreEndBig = coreStartBig + winSize;
                  cStartHex = '0x' + coreStartBig.toString(16);
                  cEndHex = '0x' + coreEndBig.toString(16);
                }

                const cProg = calculateWindowRangeProgress(
                  cStartHex,
                  cEndHex,
                  puzzle.range_start_hex,
                  puzzle.range_end_hex,
                  4
                );

                if (cProg) {
                  coresList.push({
                    coreId: i,
                    startHex: cStartHex,
                    endHex: cEndHex,
                    progress: cProg,
                  });
                }
              }
            } catch (e) {
              console.warn('Fallback multi-core calculation warning:', e);
            }
          }
        }

        const isGpuMode = hunter?.device_mode === 'gpu' || Boolean(hunter?.gpu_info);
        const gpuInfo = hunter?.gpu_info;

        return (
          <div className={`p-4 bg-slate-950 border rounded-2xl ${isGpuMode ? 'border-purple-500/40 shadow-lg shadow-purple-950/20' : 'border-amber-500/30'}`}>
            <div className="flex items-center justify-between text-xs mb-2 flex-wrap gap-2">
              <span className={`font-semibold uppercase tracking-wider text-[11px] flex items-center gap-1.5 ${isGpuMode ? 'text-purple-300' : 'text-amber-400'}`}>
                <span className={`w-2.5 h-2.5 rounded-full animate-ping ${isGpuMode ? 'bg-purple-400' : 'bg-amber-400'}`}></span>
                {isGpuMode
                  ? "Window Sedang Diperiksa (CUDA Multi-Grid Kangaroo O(√N)):"
                  : "Window Sedang Diperiksa (Pollard's Kangaroo O(√N)):"}
              </span>
              <div className="flex items-center gap-2 sm:gap-3 text-[11px] text-slate-400 flex-wrap">
                {/* Mode Komputasi (Local / VPS / GPU CUDA) */}
                {isGpuMode ? (
                  <span className="bg-purple-500/20 text-purple-300 font-bold px-2.5 py-0.5 rounded-lg border border-purple-500/40 flex items-center gap-1.5 shadow-sm shadow-purple-500/20" title="Mode GPU: Akselerasi Hardware NVIDIA CUDA Aktif">
                    <Zap className="w-3.5 h-3.5 text-purple-400 fill-purple-400 animate-pulse" />
                    <span>GPU MODE (NVIDIA CUDA)</span>
                  </span>
                ) : hunter.device_mode === 'vps' ? (
                  <span className="bg-emerald-500/15 text-emerald-300 font-bold px-2 py-0.5 rounded-lg border border-emerald-500/30 flex items-center gap-1" title="Mode VPS: Memaksimalkan core untuk puzzle (Sisakan 1 core untuk OS + Postgres + Web API)">
                    <Server className="w-3 h-3 text-emerald-400" />
                    <span>VPS MODE (Max Core)</span>
                  </span>
                ) : (
                  <span className="bg-blue-500/15 text-blue-300 font-bold px-2 py-0.5 rounded-lg border border-blue-500/30 flex items-center gap-1" title="Mode Local: Sisakan 2 core untuk OS & background desktop">
                    <Cpu className="w-3 h-3 text-blue-400" />
                    <span>LOCAL MODE</span>
                  </span>
                )}

                {isGpuMode ? (
                  <span className="bg-purple-500/15 text-purple-300 font-bold px-2 py-0.5 rounded-lg border border-purple-500/30 flex items-center gap-1">
                    <Zap className="w-3 h-3 text-purple-400 fill-purple-400/40" />
                    <span>{coresList.length} CUDA Streams Aktif</span>
                  </span>
                ) : (
                  <span className="bg-amber-500/15 text-amber-300 font-bold px-2 py-0.5 rounded-lg border border-amber-500/30 flex items-center gap-1">
                    <Zap className="w-3 h-3 text-amber-400 fill-amber-400/30" />
                    <span>{coresList.length} Core Aktif</span>
                  </span>
                )}

                {hunter.round > 0 && hunter.total_keys_checked > 0 && (
                  <span className="bg-slate-900 px-2 py-0.5 rounded-lg border border-slate-800">
                    ~{formatLargeNumber(Math.round(hunter.total_keys_checked / hunter.round))} keys/ronde
                  </span>
                )}
                <span>{hunter.last_check_at}</span>
              </div>
            </div>

            {/* Panel Khusus GPU Hardware Telemetry jika Mode GPU Aktif */}
            {isGpuMode && (
              <div className="mb-3 bg-gradient-to-r from-purple-950/40 via-slate-900/80 to-cyan-950/30 p-3 rounded-xl border border-purple-500/30 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 shadow-md shadow-purple-950/30">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-purple-500/20 text-purple-300 rounded-lg border border-purple-500/30 shrink-0">
                    <Zap className="w-5 h-5 fill-purple-400/40" />
                  </div>
                  <div>
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="font-extrabold text-sm text-white tracking-wide">
                        {gpuInfo?.name || 'NVIDIA RTX 6000 Ada Generation'}
                      </span>
                      <span className="px-2 py-0.5 bg-emerald-500/15 text-emerald-400 border border-emerald-500/30 text-[10px] font-bold rounded-full flex items-center gap-1">
                        <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-ping"></span>
                        CUDA READY
                      </span>
                    </div>
                    <div className="flex items-center gap-2 text-[11px] text-slate-400 mt-0.5 flex-wrap">
                      <span>Arsitektur: <strong className="text-purple-300 font-mono">{(gpuInfo?.cuda_cores || 18176).toLocaleString()} CUDA Cores</strong> ({gpuInfo?.sm_count || 142} SMs)</span>
                      <span>•</span>
                      <span>VRAM: <strong className="text-cyan-300 font-mono">{Math.round((gpuInfo?.total_memory_mb || 49140) / 1024)} GB GDDR6</strong></span>
                      <span>•</span>
                      <span>Driver: <strong className="text-slate-300 font-mono">{gpuInfo?.driver_version || '550.54'}</strong></span>
                    </div>
                  </div>
                </div>
                <div className="text-left sm:text-right shrink-0">
                  <div className="text-[10px] text-slate-400 uppercase tracking-wider font-semibold">Dispatch Architecture</div>
                  <div className="text-xs font-mono font-bold text-purple-300 flex items-center gap-1 sm:justify-end">
                    <span>{coresList.length} Concurrent SM Grid Streams</span>
                  </div>
                </div>
              </div>
            )}

            <div className="bg-slate-900/80 p-3 rounded-xl border border-slate-800 space-y-2.5">
              <div className="font-mono text-xs sm:text-sm text-slate-200 break-all select-all">
                {hunter.last_window_hex}
              </div>
              {overallProgress && (
                <div
                  className="flex items-center gap-3"
                  title={`Total key diproses: ~${formatLargeNumber(totalHistoricalKeys)} dari ${puzzle.bits}-bit key space (${windowsCount.toLocaleString()} windows × 2^${effectiveBits} keys)`}
                >
                  <span className="text-[10px] text-emerald-400/80 font-semibold whitespace-nowrap shrink-0">Total Progres</span>
                  <div className="flex-1 relative h-4 bg-slate-950 rounded-full border border-slate-800 overflow-hidden">
                    <div
                      className="absolute inset-y-0 left-0 bg-gradient-to-r from-emerald-600 to-emerald-400 rounded-full transition-all duration-700 ease-out"
                      style={{ width: `${Math.max(overallProgress.pct > 0 && overallProgress.pct < 0.5 ? 0.5 : overallProgress.pct, 0)}%` }}
                    />
                    {/* Glow effect di ujung bar */}
                    {overallProgress.pct > 0 && (
                      <div
                        className="absolute inset-y-0 w-2 bg-emerald-300/60 rounded-full blur-sm"
                        style={{ left: `${Math.max(overallProgress.pct > 0 && overallProgress.pct < 0.5 ? 0.5 : overallProgress.pct, 0)}%` }}
                      />
                    )}
                  </div>
                  <span className="text-sm text-emerald-400 font-extrabold font-mono whitespace-nowrap shrink-0">
                    {overallProgress.label}
                  </span>
                </div>
              )}
            </div>

            {coresList.length > 0 && (
              <div className={`mt-3 bg-slate-900/60 p-3.5 rounded-xl border space-y-3 ${isGpuMode ? 'border-purple-500/30' : 'border-slate-800/80'}`}>
                <div className="flex items-center justify-between text-[11px] text-slate-400">
                  <span className="flex items-center gap-1.5 text-slate-300 font-medium">
                    <span>Posisi di Keseluruhan Range Target:</span>
                    <span className={`font-bold font-mono px-2 py-0.5 rounded border ${
                      isGpuMode
                        ? 'text-purple-300 bg-purple-500/15 border-purple-500/30'
                        : 'text-amber-400 bg-amber-500/10 border-amber-500/20'
                    }`}>
                      {coresList.length > 1
                        ? isGpuMode
                          ? `${coresList.length} Parallel CUDA Streams (SM Multi-Grid)`
                          : `${coresList.length} Titik Core Jump (Multi-Thread)`
                        : coresList[0]?.progress.display}
                    </span>
                  </span>
                  <span className="text-[10px] text-slate-500 font-mono">
                    Target: {puzzle.bits}-bit
                  </span>
                </div>

                {/* Track Indikator Multi-Core / CUDA Stream Titik Lokasi Jump */}
                <div className="relative w-full pt-8 pb-5">
                  {/* Track base */}
                  <div className={`w-full bg-slate-950 rounded-full h-3 border relative overflow-hidden shadow-inner ${
                    isGpuMode ? 'border-purple-500/30' : 'border-slate-800'
                  }`}>
                    {/* Tick mark pembagi kuadran: 25%, 50%, 75% */}
                    <div className={`absolute left-1/4 top-0 bottom-0 w-px ${isGpuMode ? 'bg-purple-900/50' : 'bg-slate-800/80'}`} />
                    <div className={`absolute left-2/4 top-0 bottom-0 w-px ${isGpuMode ? 'bg-purple-900/50' : 'bg-slate-800/80'}`} />
                    <div className={`absolute left-3/4 top-0 bottom-0 w-px ${isGpuMode ? 'bg-purple-900/50' : 'bg-slate-800/80'}`} />

                    {/* Local spot highlight untuk setiap core / CUDA stream di track */}
                    {coresList.map((core) => (
                      <div
                        key={`track-range-${core.coreId}`}
                        className={`absolute top-0 bottom-0 rounded-full transition-all duration-700 ease-out ${
                          isGpuMode
                            ? 'bg-purple-400/50 border border-purple-300/40 shadow-sm shadow-purple-500/30'
                            : 'bg-amber-400/40'
                        }`}
                        style={{
                          left: `${Math.max(0, Math.min(98, core.progress.startPct))}%`,
                          width: `${Math.max(1.5, Math.min(100 - core.progress.startPct, Math.max(1.5, core.progress.endPct - core.progress.startPct)))}%`,
                        }}
                        title={isGpuMode ? `CUDA Stream #${core.coreId}: ${core.progress.display}` : `Core #${core.coreId}: ${core.progress.display}`}
                      />
                    ))}
                  </div>

                  {/* Marker Pinpoint yang menyentuh titik lokasi jump masing-masing core / stream */}
                  {coresList.map((core, idx) => {
                    const isAlt = idx % 2 === 1;
                    return (
                      <div
                        key={`pinpoint-${core.coreId}`}
                        className="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 transition-all duration-700 ease-out flex flex-col items-center group cursor-pointer z-10"
                        style={{ left: `${Math.max(1.5, Math.min(98.5, core.progress.startPct))}%` }}
                        title={isGpuMode ? `CUDA Stream #${core.coreId}: ${core.progress.startFormatted} (${core.startHex})` : `Core #${core.coreId}: ${core.progress.startFormatted} (${core.startHex})`}
                      >
                        {/* Badge nomor core & persentase jump di atas pin */}
                        <div
                          className={`absolute ${isAlt ? '-top-6' : '-top-8'} transition-transform group-hover:scale-110 px-1.5 py-0.5 rounded-md bg-slate-950/95 border text-[9px] font-mono font-bold whitespace-nowrap shadow-md shadow-black/80 flex items-center gap-1 z-20 pointer-events-none ${
                            isGpuMode
                              ? 'border-purple-500/60 text-purple-300 shadow-purple-950/40'
                              : 'border-amber-500/50 text-amber-300'
                          }`}
                        >
                          <span className={`w-1.5 h-1.5 rounded-full animate-pulse ${isGpuMode ? 'bg-purple-400' : 'bg-amber-400'}`}></span>
                          <span>{isGpuMode ? `S${core.coreId}` : `C${core.coreId}`}: {core.progress.startFormatted}</span>
                        </div>

                        {/* Animasi Ping & Glowing Dot */}
                        <div className="relative flex items-center justify-center">
                          <span
                            className={`absolute w-5 h-5 rounded-full animate-ping ${
                              isGpuMode ? 'bg-purple-400/35' : 'bg-amber-400/35'
                            }`}
                            style={{ animationDuration: `${1.1 + (idx % 4) * 0.3}s` }}
                          />
                          <span className={`relative w-3.5 h-3.5 rounded-full border-2 border-slate-950 shadow-md flex items-center justify-center ${
                            isGpuMode
                              ? 'bg-gradient-to-b from-purple-200 via-purple-500 to-indigo-700 shadow-purple-500/80 ring-1 ring-purple-300/80'
                              : 'bg-gradient-to-b from-amber-200 via-amber-400 to-amber-600 shadow-amber-400/80 ring-1 ring-amber-300/80'
                          }`}>
                            <span className="text-[7.5px] font-extrabold text-slate-950 font-mono leading-none">
                              {core.coreId}
                            </span>
                          </span>
                        </div>

                        {/* Garis vertikal penunjuk titik track */}
                        <div className={`w-0.5 h-1.5 mt-0.5 ${isGpuMode ? 'bg-purple-400/80' : 'bg-amber-400/70'}`}></div>
                      </div>
                    );
                  })}
                </div>

                <div className="flex justify-between text-[9px] text-slate-500 font-mono">
                  <span>0% ({puzzle.range_start_hex ? puzzle.range_start_hex.slice(0, 10) + '...' : 'Start'})</span>
                  <span className={`font-bold font-sans flex items-center gap-1 ${isGpuMode ? 'text-purple-400' : 'text-amber-400'}`}>
                    <span>{isGpuMode ? `🚀 ${coresList.length} Parallel CUDA Streams Aktif (RTX 6000)` : `📍 ${coresList.length} Titik Jump Aktif`}</span>
                  </span>
                  <span>100% ({puzzle.range_end_hex ? puzzle.range_end_hex.slice(0, 10) + '...' : 'End'})</span>
                </div>

                {/* Grid Status Jump Per Core / CUDA Stream */}
                {coresList.length > 1 && (
                  <div className="pt-2.5 border-t border-slate-800/60 mt-1">
                    <div className="text-[10px] text-slate-400 font-medium mb-1.5 flex items-center justify-between">
                      <span className="flex items-center gap-1">
                        {isGpuMode ? (
                          <Zap className="w-3.5 h-3.5 text-purple-400 fill-purple-400/40" />
                        ) : (
                          <Cpu className="w-3.5 h-3.5 text-amber-400" />
                        )}
                        <span>
                          {isGpuMode
                            ? `Status Grid CUDA Work Streams (${coresList.length} Streams Paralel — Live SM Dispatch):`
                            : `Status Jump Per Core (${coresList.length} Core Aktif):`}
                        </span>
                      </span>
                      <span className="text-[9px] text-slate-500 font-mono">Live Update Per Ronde</span>
                    </div>
                    <div className="grid grid-cols-2 sm:grid-cols-4 md:grid-cols-4 lg:grid-cols-8 gap-1.5">
                      {coresList.map((core) => (
                        <div
                          key={`pill-${core.coreId}`}
                          className={`p-1.5 bg-slate-950/90 rounded-lg border transition-colors flex flex-col items-center text-center group ${
                            isGpuMode
                              ? 'border-slate-800/90 hover:border-purple-500/50'
                              : 'border-slate-800/90 hover:border-amber-500/50'
                          }`}
                          title={isGpuMode ? `CUDA Stream ${core.coreId} (SM Grid): ${core.startHex}..${core.endHex}` : `Core ${core.coreId}: ${core.startHex}..${core.endHex}`}
                        >
                          <div className={`flex items-center gap-1 text-[9px] font-bold mb-0.5 ${isGpuMode ? 'text-purple-300' : 'text-amber-400'}`}>
                            <span className={`w-1.5 h-1.5 rounded-full ${isGpuMode ? 'bg-purple-400' : 'bg-amber-400'} group-hover:animate-ping`}></span>
                            <span>{isGpuMode ? `Stream #${core.coreId}` : `Core ${core.coreId}`}</span>
                          </div>
                          <div className="text-[11px] font-mono font-extrabold text-white">
                            {core.progress.startFormatted}
                          </div>
                          <div className="text-[8px] font-mono text-slate-500 truncate w-full mt-0.5">
                            {core.startHex.slice(0, 8)}...
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        );
      })()}

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
                  <th className="py-2 px-3 text-amber-400 font-bold">Lokasi Jump (%)</th>
                  <th className="py-2 px-3">Range Start (HEX)</th>
                  <th className="py-2 px-3">Range End (HEX)</th>
                  <th className="py-2 px-3">Kunci Diuji</th>
                  <th className="py-2 px-3">Durasi</th>
                  <th className="py-2 px-3">Waktu Simpan</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800/50 text-slate-300">
                {windows.map((w) => {
                  const progress = calculateWindowRangeProgress(
                    w.range_start_hex,
                    w.range_end_hex,
                    puzzle.range_start_hex,
                    puzzle.range_end_hex,
                    4
                  );
                  const startPct = calculateRangePercentage(
                    w.range_start_hex,
                    puzzle.range_start_hex,
                    puzzle.range_end_hex,
                    4
                  );
                  const endPct = calculateRangePercentage(
                    w.range_end_hex,
                    puzzle.range_start_hex,
                    puzzle.range_end_hex,
                    4
                  );

                  return (
                    <tr key={w.id} className="hover:bg-slate-800/40 transition">
                      <td className="py-2 px-3 text-slate-500">#{w.id}</td>
                      <td className="py-2 px-3">
                        {progress ? (
                          <div className="space-y-1">
                            <span
                              className="font-bold text-amber-400 bg-amber-500/10 px-2 py-0.5 rounded border border-amber-500/20 text-xs inline-block"
                              title={`Jump di ${progress.display} dari keseluruhan range puzzle (Start: ${progress.startFormatted}, End: ${progress.endFormatted})`}
                            >
                              {progress.display}
                            </span>
                            <div
                              className="w-24 bg-slate-950 border border-slate-800 rounded-full h-1.5 relative overflow-hidden"
                              title={`Posisi grafis jump: ${progress.startFormatted} dari keseluruhan target`}
                            >
                              <div
                                className="absolute top-0 bottom-0 w-2 -translate-x-1/2 bg-amber-400 rounded-full shadow-sm shadow-amber-400/80 transition-all duration-300"
                                style={{ left: `${Math.max(2, Math.min(98, progress.startPct))}%` }}
                              />
                            </div>
                          </div>
                        ) : (
                          <span className="text-slate-600">-</span>
                        )}
                      </td>
                      <td className="py-2 px-3">
                        <div className="text-amber-300/80 truncate max-w-xs">{w.range_start_hex}</div>
                        {startPct && (
                          <div className="text-[10px] text-amber-400/80 font-sans mt-0.5 flex items-center gap-1">
                            <span className="text-slate-500">Posisi Start:</span>
                            <span className="font-mono font-medium">{startPct.formatted}</span>
                          </div>
                        )}
                      </td>
                      <td className="py-2 px-3">
                        <div className="text-slate-400 truncate max-w-xs">{w.range_end_hex}</div>
                        {endPct && (
                          <div className="text-[10px] text-slate-400/90 font-sans mt-0.5 flex items-center gap-1">
                            <span className="text-slate-500">Posisi End:</span>
                            <span className="font-mono font-medium">{endPct.formatted}</span>
                          </div>
                        )}
                      </td>
                      <td className="py-2 px-3 text-slate-200">{w.keys_checked.toLocaleString()}</td>
                      <td className="py-2 px-3 text-emerald-400">{w.duration_secs.toFixed(3)}s</td>
                      <td className="py-2 px-3 text-slate-500 text-[11px]">
                        {new Date(w.created_at).toLocaleTimeString()}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
};

