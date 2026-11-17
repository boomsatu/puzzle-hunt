import React, { useState } from 'react';
import {
  Play,
  Square,
  Trash2,
  ExternalLink,
  Cpu,
  Layers,
  Zap,
  Activity,
  CheckCircle,
  Copy,
  Check,
  Loader2,
} from 'lucide-react';
import {
  HunterState,
  Puzzle,
  formatLargeNumber,
  formatCompactKeys,
  formatKeysSpeed,
  calculateRangePercentage,
  parseWindowHexRange,
} from '../types';
import { StartHuntModal } from './StartHuntModal';

interface PuzzleCardProps {
  puzzle: Puzzle;
  hunter?: HunterState;
  destinationAddress: string;
  onSelect: () => void;
  onRefresh: () => void;
}

export const PuzzleCard: React.FC<PuzzleCardProps> = ({ puzzle, hunter, destinationAddress, onSelect, onRefresh }) => {
  const [windowBits, setWindowBits] = useState(30);
  const [loading, setLoading] = useState(false);
  const [copiedAddr, setCopiedAddr] = useState(false);
  const [isStartModalOpen, setIsStartModalOpen] = useState(false);

  const isRunning = hunter?.status === 'Running';
  const isPausing = hunter?.status === 'Pausing';
  const isSolved = puzzle.is_solved || hunter?.status === 'SolvedByUs' || hunter?.status === 'SolvedByOther';

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
    } catch (e) {
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const handleDelete = async () => {
    if (!confirm(`Yakin ingin menghapus Puzzle #${puzzle.puzzle_number} dari database?`)) return;
    setLoading(true);
    try {
      await fetch(`/api/puzzles/${puzzle.puzzle_number}`, { method: 'DELETE' });
      onRefresh();
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

  return (
    <div
      className={`rounded-2xl border transition-all duration-200 overflow-hidden ${
        isPausing
          ? 'bg-slate-900/90 border-amber-500/30 shadow-lg shadow-amber-500/5 ring-1 ring-amber-500/10'
          : isRunning
          ? 'bg-slate-900/90 border-amber-500/50 shadow-lg shadow-amber-500/5 ring-1 ring-amber-500/20'
          : isSolved
          ? 'bg-slate-900/70 border-emerald-500/40'
          : 'bg-slate-900/40 border-slate-800/80 hover:border-slate-700'
      }`}
    >
      {/* Header Card */}
      <div className="p-5 border-b border-slate-800/60 flex items-start justify-between">
        <div>
          <div className="flex items-center gap-2 mb-1">
            <h4 className="text-xl font-bold text-white tracking-tight">
              Puzzle #{puzzle.puzzle_number}
            </h4>
            <span className="px-2 py-0.5 rounded-full text-[11px] font-semibold bg-slate-800 text-slate-300">
              {puzzle.bits}-bit
            </span>
            {isSolved ? (
              <span className="px-2 py-0.5 rounded-full text-[11px] font-bold bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 flex items-center gap-1">
                <CheckCircle className="w-3 h-3" /> SOLVED
              </span>
            ) : isPausing ? (
              <span className="px-2 py-0.5 rounded-full text-[11px] font-bold bg-amber-500/15 text-amber-300 border border-amber-500/30 flex items-center gap-1.5 animate-pulse">
                <Loader2 className="w-3 h-3 animate-spin text-amber-400" />
                MENJEDA...
              </span>
            ) : isRunning ? (
              <div className="flex items-center gap-1.5">
                <span className="px-2 py-0.5 rounded-full text-[11px] font-bold bg-amber-500/20 text-amber-400 border border-amber-500/30 flex items-center gap-1.5 animate-pulse">
                  <span className="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
                  RUNNING
                </span>
                <span
                  className={`px-2 py-0.5 rounded-full text-[10px] font-bold border flex items-center gap-1 ${
                    hunter?.device_mode === 'gpu' || Boolean(hunter?.gpu_info)
                      ? 'bg-purple-500/20 text-purple-300 border-purple-500/30'
                      : 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30'
                  }`}
                  title={
                    hunter?.device_mode === 'gpu' || Boolean(hunter?.gpu_info)
                      ? 'Berjalan di GPU (NVIDIA CUDA)'
                      : 'Berjalan di CPU Server (Multi-Core)'
                  }
                >
                  {hunter?.device_mode === 'gpu' || Boolean(hunter?.gpu_info) ? (
                    <>
                      <Zap className="w-2.5 h-2.5 text-purple-400" /> GPU
                    </>
                  ) : (
                    <>
                      <Cpu className="w-2.5 h-2.5 text-emerald-400" /> CPU
                    </>
                  )}
                </span>
              </div>
            ) : hunter?.status === 'Paused' ? (
              <span className="px-2 py-0.5 rounded-full text-[11px] font-medium bg-amber-500/10 text-amber-300/80 border border-amber-500/20">
                PAUSED
              </span>
            ) : (
              <span className="px-2 py-0.5 rounded-full text-[11px] font-medium bg-slate-800/60 text-slate-400">
                IDLE
              </span>
            )}
          </div>
          <div className="text-xs text-amber-400 font-medium">
            Reward: {puzzle.reward_btc > 0 ? `${puzzle.reward_btc} BTC` : 'TBA'}
          </div>
        </div>

        <button
          onClick={handleDelete}
          disabled={isRunning || isPausing || loading}
          title="Hapus puzzle dari database"
          className="text-slate-500 hover:text-rose-400 p-1.5 rounded-lg hover:bg-slate-800 transition disabled:opacity-30"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>

      {/* Body Info */}
      <div className="p-5 space-y-4">
        {/* Address & Public Key */}
        <div className="space-y-2 text-xs">
          <div>
            <div className="text-[11px] text-slate-500 uppercase tracking-wider font-semibold mb-0.5">
              Target Address
            </div>
            <div className="flex items-center gap-2">
              <span className="font-mono text-slate-300 truncate">{puzzle.address}</span>
              <button
                onClick={copyAddress}
                className="text-slate-400 hover:text-white transition p-0.5"
                title="Salin Address"
              >
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
            <div className="flex items-center justify-between text-[11px] text-slate-500 uppercase tracking-wider font-semibold mb-0.5">
              <span>Public Key</span>
              {puzzle.pubkey_hex && puzzle.pubkey_hex.length === 66 ? (
                <span className="text-[10px] text-emerald-400 font-sans font-normal">Exposed (Kangaroo)</span>
              ) : (
                <span className="text-[10px] text-amber-400 font-sans font-normal">Belum Terekspos (Address Scan)</span>
              )}
            </div>
            <div className="font-mono text-slate-400 truncate text-[11px] bg-slate-950/60 px-2 py-1 rounded-lg border border-slate-800/60">
              {puzzle.pubkey_hex && puzzle.pubkey_hex.length === 66 ? (
                puzzle.pubkey_hex
              ) : (
                <span className="text-amber-400/80 font-sans italic">Belum Terekspos On-Chain</span>
              )}
            </div>
          </div>
        </div>

        {/* Live Hunter Metrics */}
        {hunter && (() => {
          const isGpuMode = hunter.device_mode === 'gpu' || Boolean(hunter.gpu_info);
          const cardKeysPerBlock = hunter.window_keys
            ? hunter.window_keys
            : Math.pow(
                2,
                hunter.window_bits
                  ? hunter.window_bits
                  : isGpuMode
                  ? 30
                  : puzzle.pubkey_hex && puzzle.pubkey_hex.length === 66
                  ? windowBits
                  : Math.min(windowBits, 18)
              );
          const cardTotalHistoricalKeys = Math.max(
            hunter.windows_scanned * cardKeysPerBlock,
            hunter.total_keys_checked || 0
          );

          return (
            <div className="grid grid-cols-2 gap-2 pt-2 border-t border-slate-800/60 text-xs">
              <div className="p-2.5 bg-slate-950/50 border border-slate-800/60 rounded-xl">
                <div className="flex items-center justify-between text-slate-400 text-[11px] mb-1">
                  <div className="flex items-center gap-1.5">
                    <Zap className={`w-3.5 h-3.5 ${isGpuMode ? 'text-purple-400 fill-purple-400/30' : 'text-amber-400'}`} />
                    <span>Speed / dt</span>
                  </div>
                  {hunter.active_workers && hunter.active_workers > 1 && (
                    <span
                      className={`text-[10px] font-semibold px-1 rounded border ${isGpuMode ? 'text-purple-300 bg-purple-500/15 border-purple-500/30' : 'text-amber-400 bg-amber-500/10 border-amber-500/20'}`}
                      title={isGpuMode ? `Menggunakan ${hunter.active_workers} CUDA Grid Streams paralel pada NVIDIA GPU` : `Menggunakan ${hunter.active_workers} CPU worker threads paralel`}
                    >
                      ⚡ {hunter.active_workers} {isGpuMode ? 'Streams' : 'Core'}
                    </span>
                  )}
                </div>
                <div className="text-sm font-bold text-white font-mono">
                  {hunter.keys_per_second > 0
                    ? formatKeysSpeed(hunter.keys_per_second)
                    : '0 k/s'}
                </div>
              </div>

              <div className="p-2.5 bg-slate-950/50 border border-slate-800/60 rounded-xl">
                <div className="flex items-center gap-1.5 text-slate-400 text-[11px] mb-1">
                  <Layers className="w-3.5 h-3.5 text-blue-400" />
                  <span>Windows (Postgres)</span>
                </div>
                <div className="text-sm font-bold text-white font-mono">
                  {hunter.windows_scanned.toLocaleString()}
                </div>
                <div
                  className="text-[10px] text-blue-400 font-mono mt-0.5 truncate"
                  title={`Total Kunci: ${cardTotalHistoricalKeys.toLocaleString()} keys (${hunter.windows_scanned.toLocaleString()} window × ${cardKeysPerBlock.toLocaleString()} keys)`}
                >
                  ~{formatCompactKeys(cardTotalHistoricalKeys)} keys
                </div>
              </div>

              <div className="col-span-2 p-2.5 bg-slate-950/50 border border-slate-800/60 rounded-xl">
                <div className="flex items-center justify-between text-[11px] text-slate-400 mb-1">
                  <div className="flex items-center gap-1.5">
                    <Activity className="w-3.5 h-3.5 text-emerald-400" />
                    <span>{isGpuMode ? `Dispatch #${hunter.round}` : `Ronde ${hunter.round}`}</span>
                  </div>
                  <span>Sesi ini: {formatLargeNumber(hunter.total_keys_checked)}</span>
                </div>
                {hunter.last_window_hex && (() => {
                  const parsed = parseWindowHexRange(hunter.last_window_hex);
                  const progress = parsed.startHex
                    ? calculateRangePercentage(parsed.startHex, puzzle.range_start_hex, puzzle.range_end_hex, 2)
                    : null;

                  return (
                    <div className="mt-1 space-y-1">
                      <div className="flex items-center justify-between text-[10px] gap-1">
                        <span className="font-mono text-slate-400 truncate" title={hunter.last_window_hex}>
                          {hunter.last_window_hex}
                        </span>
                        {progress && (
                          <span className="font-mono text-amber-400 font-bold bg-amber-500/10 px-1.5 py-0.5 rounded border border-amber-500/20 shrink-0">
                            {progress.formatted}
                          </span>
                        )}
                      </div>
                    </div>
                  );
                })()}

                {/* Live Round Batch Progress Bar */}
                {hunter.keys_per_round && hunter.keys_per_round > 0 && isRunning && (
                  <div className="mt-2 pt-2 border-t border-slate-800/60 space-y-1.5">
                    <div className="flex items-center justify-between text-[10px] font-mono flex-wrap gap-1">
                      <span className="flex items-center gap-1.5 text-cyan-300 font-semibold">
                        <span className="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-ping"></span>
                        Ronde #{hunter.round}: {(hunter.round_progress_pct || 0).toFixed(0)}%
                        {hunter.round_phase === 'dispatching' && (
                          <span className="text-[9px] px-1 py-0.2 rounded bg-blue-500/20 text-blue-300 border border-blue-500/30">CPU</span>
                        )}
                        {hunter.round_phase === 'computing' && (
                          <span className="text-[9px] px-1 py-0.2 rounded bg-purple-500/20 text-purple-300 border border-purple-500/30 animate-pulse">GPU</span>
                        )}
                        {hunter.round_phase === 'committing' && (
                          <span className="text-[9px] px-1 py-0.2 rounded bg-amber-500/20 text-amber-300 border border-amber-500/30">DB</span>
                        )}
                      </span>
                      <span className="text-slate-400">
                        ~{formatCompactKeys(hunter.round_keys_checked || 0)} / {formatCompactKeys(hunter.keys_per_round)}
                      </span>
                    </div>
                    <div className="h-2 bg-slate-900 rounded-full border border-slate-800 overflow-hidden">
                      <div
                        className={`h-full rounded-full transition-all duration-150 ease-out ${
                          hunter.round_phase === 'committing'
                            ? 'bg-gradient-to-r from-purple-500 to-amber-400'
                            : hunter.round_phase === 'completed'
                            ? 'bg-gradient-to-r from-cyan-500 to-emerald-400'
                            : 'bg-gradient-to-r from-blue-500 via-purple-500 to-cyan-400'
                        }`}
                        style={{ width: `${Math.min(100, Math.max(hunter.round_progress_pct || 0, 3))}%` }}
                      />
                    </div>
                  </div>
                )}
              </div>
            </div>
          );
        })()}

        {/* Action Controls */}
        <div className="pt-2 space-y-2">
          {isPausing ? (
            <button
              disabled
              className="w-full py-2.5 bg-amber-500/15 text-amber-300 border border-amber-500/30 rounded-xl font-semibold text-xs flex items-center justify-center gap-2 cursor-wait"
            >
              <Loader2 className="w-4 h-4 animate-spin text-amber-400" />
              <span>Menyelesaikan Window...</span>
            </button>
          ) : isRunning ? (
            <button
              onClick={handleStop}
              disabled={loading}
              className="w-full py-2.5 bg-rose-500/20 hover:bg-rose-500/30 text-rose-300 border border-rose-500/30 rounded-xl font-semibold text-xs flex items-center justify-center gap-2 transition"
              title="Smart Pause: menyelesaikan window aktif lalu simpan riwayat"
            >
              <Square className="w-4 h-4 fill-current" />
              <span>Jeda Hunter</span>
            </button>
          ) : (
            <button
              onClick={() => setIsStartModalOpen(true)}
              disabled={isSolved || loading}
              className="w-full py-2 bg-gradient-to-r from-amber-500 to-amber-400 hover:from-amber-400 hover:to-amber-300 text-slate-950 font-bold rounded-xl text-xs flex items-center justify-center gap-1.5 transition disabled:opacity-40 shadow-md shadow-amber-500/10"
            >
              <Play className="w-3.5 h-3.5 fill-current" />
              <span>Mulai Hunt (Pilih GPU / CPU)</span>
            </button>
          )}

          <button
            onClick={onSelect}
            className="w-full py-2 bg-slate-800/80 hover:bg-slate-800 border border-slate-700/60 hover:border-slate-600 text-slate-200 rounded-xl font-semibold text-xs flex items-center justify-center gap-1.5 transition"
          >
            <Activity className="w-3.5 h-3.5 text-amber-400" />
            <span>Buka Detail Monitor & Log</span>
          </button>
        </div>
      </div>

      {/* Modal Pemilihan Engine GPU vs CPU & Worker Setting */}
      <StartHuntModal
        isOpen={isStartModalOpen}
        onClose={() => setIsStartModalOpen(false)}
        puzzle={puzzle}
        hunter={hunter}
        destinationAddress={destinationAddress}
        onStarted={onRefresh}
      />
    </div>
  );
};

