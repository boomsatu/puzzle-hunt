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
} from 'lucide-react';
import { HunterState, Puzzle, formatLargeNumber, formatKeysSpeed } from '../types';

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

  const isRunning = hunter?.status === 'Running';
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
        isRunning
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
            ) : isRunning ? (
              <span className="px-2 py-0.5 rounded-full text-[11px] font-bold bg-amber-500/20 text-amber-400 border border-amber-500/30 flex items-center gap-1.5 animate-pulse">
                <span className="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
                RUNNING
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
          disabled={isRunning || loading}
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
        {hunter && (
          <div className="grid grid-cols-2 gap-2 pt-2 border-t border-slate-800/60 text-xs">
            <div className="p-2.5 bg-slate-950/50 border border-slate-800/60 rounded-xl">
              <div className="flex items-center justify-between text-slate-400 text-[11px] mb-1">
                <div className="flex items-center gap-1.5">
                  <Zap className="w-3.5 h-3.5 text-amber-400" />
                  <span>Speed / dt</span>
                </div>
                {hunter.active_workers && hunter.active_workers > 1 && (
                  <span className="text-[10px] text-amber-400 font-semibold bg-amber-500/10 px-1 rounded border border-amber-500/20" title={`Menggunakan ${hunter.active_workers} CPU worker threads paralel`}>
                    ⚡ {hunter.active_workers} Core
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
            </div>

            <div className="col-span-2 p-2.5 bg-slate-950/50 border border-slate-800/60 rounded-xl">
              <div className="flex items-center justify-between text-[11px] text-slate-400 mb-1">
                <div className="flex items-center gap-1.5">
                  <Activity className="w-3.5 h-3.5 text-emerald-400" />
                  <span>Ronde {hunter.round}</span>
                </div>
                <span>Total: {formatLargeNumber(hunter.total_keys_checked)}</span>
              </div>
              {hunter.last_window_hex && (
                <div className="font-mono text-[10px] text-slate-400 truncate mt-1">
                  {hunter.last_window_hex}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Action Controls */}
        <div className="pt-2 space-y-2">
          {isRunning ? (
            <button
              onClick={handleStop}
              disabled={loading}
              className="w-full py-2.5 bg-rose-500/20 hover:bg-rose-500/30 text-rose-300 border border-rose-500/30 rounded-xl font-semibold text-xs flex items-center justify-center gap-2 transition"
            >
              <Square className="w-4 h-4 fill-current" />
              <span>Jeda / Hentikan Hunter</span>
            </button>
          ) : (
            <div className="flex items-center gap-2">
              <select
                value={windowBits}
                onChange={(e) => setWindowBits(Number(e.target.value))}
                disabled={isSolved || loading}
                className="bg-slate-950 border border-slate-800 rounded-xl px-2 py-2 text-xs text-slate-300 focus:outline-none focus:border-amber-500"
              >
                <option value={20}>Window 2^20 (1M keys)</option>
                <option value={24}>Window 2^24 (16M keys)</option>
                <option value={28}>Window 2^28 (268M keys)</option>
                <option value={30}>Window 2^30 (1B keys - Optimal)</option>
                <option value={32}>Window 2^32 (4B keys)</option>
              </select>

              <button
                onClick={handleStart}
                disabled={isSolved || loading}
                className="flex-1 py-2 bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold rounded-xl text-xs flex items-center justify-center gap-1.5 transition disabled:opacity-40"
              >
                <Play className="w-3.5 h-3.5 fill-current" />
                <span>Mulai Hunt</span>
              </button>
            </div>
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
    </div>
  );
};

