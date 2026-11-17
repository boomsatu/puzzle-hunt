import React, { useState } from 'react';
import {
  X,
  Play,
  Zap,
  Cpu,
  Sliders,
  Layers,
  Info,
  CheckCircle2,
  RefreshCw,
} from 'lucide-react';
import { Puzzle, HunterState } from '../types';

interface StartHuntModalProps {
  isOpen: boolean;
  onClose: () => void;
  puzzle: Puzzle;
  hunter?: HunterState;
  destinationAddress: string;
  onStarted: () => void;
}

export const StartHuntModal: React.FC<StartHuntModalProps> = ({
  isOpen,
  onClose,
  puzzle,
  destinationAddress,
  onStarted,
}) => {
  const hasPubkey = Boolean(puzzle.pubkey_hex && puzzle.pubkey_hex.trim().length === 66);

  // Default selection based on pubkey and standard
  const [deviceMode, setDeviceMode] = useState<'gpu' | 'cpu'>('cpu');
  const [workerCount, setWorkerCount] = useState<number>(32);
  const [windowBits, setWindowBits] = useState<number>(hasPubkey ? 24 : 18);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleDeviceChange = (mode: 'gpu' | 'cpu') => {
    setDeviceMode(mode);
    if (mode === 'gpu') {
      setWorkerCount(32);
      setWindowBits(hasPubkey ? 30 : 26);
    } else {
      setWorkerCount(32);
      setWindowBits(hasPubkey ? 24 : 18);
    }
  };

  const handleStart = async () => {
    setLoading(true);
    setError(null);

    try {
      const res = await fetch(`/api/hunt/${puzzle.puzzle_number}/start`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          window_bits: windowBits,
          destination: destinationAddress,
          device_mode: deviceMode,
          worker_count: workerCount,
        }),
      });

      const data = await res.json();
      if (res.ok && data.success) {
        onStarted();
        onClose();
      } else {
        setError(data.error || 'Gagal memulai hunter.');
      }
    } catch (e: any) {
      setError(`Koneksi error: ${e.message || e}`);
    } finally {
      setLoading(false);
    }
  };

  const keysPerWorker = Math.pow(2, windowBits);
  const totalKeysPerRound = keysPerWorker * workerCount;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-md p-4 overflow-y-auto">
      <div className="bg-slate-900 border border-slate-700/80 rounded-2xl w-full max-w-xl overflow-hidden shadow-2xl animate-in fade-in zoom-in-95 duration-150 my-auto">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-gradient-to-r from-slate-950 to-slate-900">
          <div>
            <div className="flex items-center gap-2">
              <h3 className="font-bold text-lg text-white">
                Mulai Hunt — Puzzle #{puzzle.puzzle_number}
              </h3>
              <span className="px-2 py-0.5 rounded-full text-[11px] font-mono font-bold bg-amber-500/20 text-amber-400 border border-amber-500/30">
                {puzzle.bits}-bit
              </span>
            </div>
            <p className="text-xs text-slate-400 mt-0.5 flex items-center gap-2">
              <span>Target: <code className="text-slate-300 font-mono">{puzzle.address.slice(0, 10)}...{puzzle.address.slice(-6)}</code></span>
              <span>•</span>
              <span className={hasPubkey ? 'text-purple-400 font-semibold' : 'text-blue-400 font-semibold'}>
                {hasPubkey ? "🦘 Kangaroo O(√N)" : "⚡ Address Scan"}
              </span>
            </p>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1.5 rounded-lg hover:bg-slate-800 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Body */}
        <div className="p-6 space-y-5">
          {/* Error Message */}
          {error && (
            <div className="p-3 bg-rose-500/10 border border-rose-500/30 rounded-xl text-xs text-rose-400">
              <span className="font-bold">Error: </span> {error}
            </div>
          )}

          {/* 1. Pilih Mesin Komputasi */}
          <div>
            <label className="text-xs font-semibold uppercase tracking-wider text-slate-300 block mb-2 flex items-center gap-1.5">
              <Sliders className="w-3.5 h-3.5 text-amber-400" />
              <span>1. Pilih Mesin Komputasi (Engine)</span>
            </label>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              {/* Option: GPU */}
              <div
                onClick={() => handleDeviceChange('gpu')}
                className={`p-4 rounded-xl border cursor-pointer transition flex flex-col justify-between ${
                  deviceMode === 'gpu'
                    ? 'bg-purple-950/30 border-purple-500/80 shadow-md shadow-purple-500/10'
                    : 'bg-slate-950/60 border-slate-800 hover:border-slate-700'
                }`}
              >
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="flex items-center gap-2 text-sm font-bold text-white">
                      <Zap className="w-4 h-4 text-purple-400" />
                      <span>GPU Accelerator</span>
                    </span>
                    <span className="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-purple-500/20 text-purple-300">
                      CUDA Dual RTX 5090
                    </span>
                  </div>
                  <p className="text-xs text-slate-400 leading-relaxed">
                    cuBitCrack multi-stream parallel GPU engine. Brute-force kecepatan tinggi (miliaran keys/s).
                  </p>
                </div>
                <div className="mt-3 text-[11px] font-mono text-purple-300 flex items-center gap-1">
                  {deviceMode === 'gpu' && <CheckCircle2 className="w-3.5 h-3.5 text-purple-400" />}
                  <span>Default: 32 Streams, Window 2^{hasPubkey ? 30 : 26}</span>
                </div>
              </div>

              {/* Option: CPU */}
              <div
                onClick={() => handleDeviceChange('cpu')}
                className={`p-4 rounded-xl border cursor-pointer transition flex flex-col justify-between ${
                  deviceMode === 'cpu'
                    ? 'bg-emerald-950/30 border-emerald-500/80 shadow-md shadow-emerald-500/10'
                    : 'bg-slate-950/60 border-slate-800 hover:border-slate-700'
                }`}
              >
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="flex items-center gap-2 text-sm font-bold text-white">
                      <Cpu className="w-4 h-4 text-emerald-400" />
                      <span>CPU Server</span>
                    </span>
                    <span className="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-emerald-500/20 text-emerald-300">
                      128 Cores Server
                    </span>
                  </div>
                  <p className="text-xs text-slate-400 leading-relaxed">
                    Memanfaatkan core CPU server yang menganggur. Bebas gangguan ke GPU RTX 5090.
                  </p>
                </div>
                <div className="mt-3 text-[11px] font-mono text-emerald-300 flex items-center gap-1">
                  {deviceMode === 'cpu' && <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />}
                  <span>Default: 32 Workers, Window 2^{hasPubkey ? 24 : 18}</span>
                </div>
              </div>
            </div>
          </div>

          {/* 2. Jumlah Worker & Window Size */}
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            {/* Worker Count */}
            <div>
              <div className="flex items-center justify-between mb-1.5">
                <label className="text-xs font-semibold text-slate-300">
                  {deviceMode === 'gpu' ? 'Jumlah CUDA Streams' : 'Jumlah Worker CPU (Threads)'}
                </label>
                <span className="text-[11px] font-mono text-slate-400 font-bold">
                  {workerCount} {deviceMode === 'gpu' ? 'streams' : 'threads'}
                </span>
              </div>
              <div className="flex items-center gap-2 mb-2">
                {[16, 32, 64].map((cnt) => (
                  <button
                    key={cnt}
                    type="button"
                    onClick={() => setWorkerCount(cnt)}
                    className={`flex-1 py-1 px-2 rounded-lg text-xs font-mono font-semibold border transition ${
                      workerCount === cnt
                        ? 'bg-amber-500 text-slate-950 border-amber-500 font-bold'
                        : 'bg-slate-950 text-slate-400 border-slate-800 hover:border-slate-700'
                    }`}
                  >
                    {cnt}
                  </button>
                ))}
              </div>
              <input
                type="number"
                min={1}
                max={128}
                value={workerCount}
                onChange={(e) => setWorkerCount(Math.max(1, Math.min(128, parseInt(e.target.value) || 1)))}
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-1.5 text-xs font-mono text-white focus:outline-none focus:border-amber-500"
              />
              <p className="text-[10px] text-slate-500 mt-1">
                {deviceMode === 'cpu'
                  ? `Mengalokasikan ${workerCount} dari 128 core CPU. Sisa ${128 - workerCount} core untuk OS & GPU coordinator.`
                  : '32 streams terdistribusi otomatis di kedua GPU RTX 5090.'}
              </p>
            </div>

            {/* Window Bits */}
            <div>
              <div className="flex items-center justify-between mb-1.5">
                <label className="text-xs font-semibold text-slate-300">Ukuran Window per Worker</label>
                <span className="text-[11px] font-mono text-amber-400 font-bold">
                  2^{windowBits} keys
                </span>
              </div>
              <select
                value={windowBits}
                onChange={(e) => setWindowBits(Number(e.target.value))}
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs text-slate-200 focus:outline-none focus:border-amber-500"
              >
                <option value={18}>2^18 (~262 Ribu) — Super Cepat (CPU Address Scan)</option>
                <option value={20}>2^20 (~1.05 Juta) — Ringan (1-2s di CPU)</option>
                <option value={24}>2^24 (~16.7 Juta) — Cepat (Rekomendasi Kangaroo CPU)</option>
                <option value={26}>2^26 (~67.1 Juta) — Standar GPU (~3s per round)</option>
                <option value={28}>2^28 (~268 Juta) — Menengah GPU</option>
                <option value={30}>2^30 (~1.07 Miliar) — Optimal GPU Kangaroo</option>
                <option value={32}>2^32 (~4.29 Miliar) — Maksimal GPU</option>
              </select>
              <p className="text-[10px] text-slate-500 mt-1">
                Ukuran blok pencarian tiap worker dalam satu ronde sebelum simpan ke PostgreSQL.
              </p>
            </div>
          </div>

          {/* 3. Estimasi Batch Summary Box */}
          <div className="p-3.5 bg-slate-950/80 border border-slate-800 rounded-xl space-y-2 text-xs">
            <div className="flex items-center justify-between text-slate-400">
              <span className="flex items-center gap-1.5">
                <Layers className="w-3.5 h-3.5 text-blue-400" />
                <span>Total Keys per Ronde:</span>
              </span>
              <span className="font-mono text-white font-bold">
                {totalKeysPerRound.toLocaleString()} keys
              </span>
            </div>
            <div className="flex items-center justify-between text-slate-400">
              <span className="flex items-center gap-1.5">
                <Info className="w-3.5 h-3.5 text-amber-400" />
                <span>Engine Terpilih:</span>
              </span>
              <span className="font-mono text-emerald-400 font-bold flex items-center gap-1">
                {deviceMode === 'gpu' ? (
                  <>⚡ GPU CUDA Dual RTX 5090 ({workerCount} Streams)</>
                ) : (
                  <>🖥️ CPU Server Multi-Core ({workerCount} Cores)</>
                )}
              </span>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="px-6 py-4 border-t border-slate-800 bg-slate-950 flex items-center justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            disabled={loading}
            className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs font-semibold rounded-xl transition"
          >
            Batal
          </button>

          <button
            type="button"
            onClick={handleStart}
            disabled={loading}
            className={`px-5 py-2 font-bold rounded-xl text-xs flex items-center gap-2 transition shadow-lg ${
              deviceMode === 'gpu'
                ? 'bg-purple-600 hover:bg-purple-500 text-white shadow-purple-600/20'
                : 'bg-emerald-500 hover:bg-emerald-400 text-slate-950 shadow-emerald-500/20'
            }`}
          >
            {loading ? (
              <>
                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                <span>Menyiapkan Engine...</span>
              </>
            ) : (
              <>
                <Play className="w-3.5 h-3.5 fill-current" />
                <span>Mulai Hunting Sekarang ({deviceMode.toUpperCase()})</span>
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
};
