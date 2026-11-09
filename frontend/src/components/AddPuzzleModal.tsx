import React, { useState } from 'react';
import { X, Plus, AlertCircle, CheckCircle2, Search, Loader2, KeyRound, Hash, Sparkles } from 'lucide-react';

interface AddPuzzleModalProps {
  isOpen: boolean;
  onClose: () => void;
  onAdded: () => void;
}

export const AddPuzzleModal: React.FC<AddPuzzleModalProps> = ({ isOpen, onClose, onAdded }) => {
  const [puzzleNumber, setPuzzleNumber] = useState<number | ''>('');
  const [bits, setBits] = useState<number | ''>('');
  const [address, setAddress] = useState('');
  const [pubkeyHex, setPubkeyHex] = useState('');
  const [isUnexposed, setIsUnexposed] = useState(false);
  const [rangeStartHex, setRangeStartHex] = useState('');
  const [rangeEndHex, setRangeEndHex] = useState('');
  const [rewardBtc, setRewardBtc] = useState<number | ''>('');
  const [loading, setLoading] = useState(false);
  const [lookupLoading, setLookupLoading] = useState(false);
  const [lookupMsg, setLookupMsg] = useState<{ type: 'success' | 'info' | 'error'; text: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!isOpen) return null;

  // Hitung rentang start dan end default dari bits secara otomatis (2^(bits-1) s/d 2^bits - 1)
  const calculateDefaultRanges = (b: number) => {
    if (b > 0 && b <= 256) {
      try {
        const start = 1n << BigInt(b - 1);
        const end = (1n << BigInt(b)) - 1n;
        return {
          start: '0x' + start.toString(16),
          end: '0x' + end.toString(16),
        };
      } catch {
        return { start: '', end: '' };
      }
    }
    return { start: '', end: '' };
  };

  const handlePuzzleNumberChange = (val: number | '') => {
    setPuzzleNumber(val);
    if (val !== '' && (bits === '' || bits === puzzleNumber)) {
      setBits(val);
      const ranges = calculateDefaultRanges(Number(val));
      setRangeStartHex(ranges.start);
      setRangeEndHex(ranges.end);
    }
  };

  const handleBitsChange = (val: number | '') => {
    setBits(val);
    if (val !== '') {
      const ranges = calculateDefaultRanges(Number(val));
      setRangeStartHex(ranges.start);
      setRangeEndHex(ranges.end);
    } else {
      setRangeStartHex('');
      setRangeEndHex('');
    }
  };

  const handleResetRanges = () => {
    if (bits !== '') {
      const ranges = calculateDefaultRanges(Number(bits));
      setRangeStartHex(ranges.start);
      setRangeEndHex(ranges.end);
    }
  };

  const handleLookupPubkey = async () => {
    const cleanAddr = address.trim();
    if (!cleanAddr) {
      setError('Masukkan Bitcoin Address terlebih dahulu untuk cek pubkey!');
      return;
    }

    setLookupLoading(true);
    setLookupMsg(null);
    setError(null);

    try {
      const res = await fetch(`/api/puzzles/lookup-pubkey/${cleanAddr}`);
      const data = await res.json();

      if (data.found && data.pubkey_hex) {
        setPubkeyHex(data.pubkey_hex);
        setIsUnexposed(false);
        setLookupMsg({
          type: 'success',
          text: `Public key ditemukan on-chain: ${data.pubkey_hex.slice(0, 14)}...`,
        });
      } else {
        setIsUnexposed(true);
        setPubkeyHex('');
        setLookupMsg({
          type: 'info',
          text: 'Alamat belum pernah mengirim transaksi. Mode otomatis diubah ke: Public Key Belum Terekspos.',
        });
      }
    } catch (e: any) {
      setLookupMsg({
        type: 'error',
        text: 'Gagal mengecek ke blockchain node.',
      });
    } finally {
      setLookupLoading(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!puzzleNumber || !bits || !address) {
      setError('Harap isi semua kolom wajib!');
      return;
    }

    if (!isUnexposed && !pubkeyHex.trim()) {
      setError('Harap isi Public Key atau centang pilihan "Public Key Belum Terekspos"!');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const res = await fetch('/api/puzzles', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          puzzle_number: Number(puzzleNumber),
          bits: Number(bits),
          address: address.trim(),
          pubkey_hex: isUnexposed ? '' : pubkeyHex.trim(),
          reward_btc: rewardBtc ? Number(rewardBtc) : 0.0,
          range_start_hex: rangeStartHex.trim() || undefined,
          range_end_hex: rangeEndHex.trim() || undefined,
        }),
      });

      const data = await res.json();
      if (!res.ok) {
        throw new Error(data.error || 'Gagal menambahkan puzzle');
      }

      onAdded();
      onClose();
      // Reset form
      setPuzzleNumber('');
      setBits('');
      setAddress('');
      setPubkeyHex('');
      setIsUnexposed(false);
      setRangeStartHex('');
      setRangeEndHex('');
      setRewardBtc('');
      setLookupMsg(null);
    } catch (err: any) {
      setError(err.message || 'Terjadi kesalahan');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4 overflow-y-auto">
      <div className="bg-slate-900 border border-slate-800 rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl animate-in fade-in zoom-in-95 duration-150 my-6">
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800">
          <div className="flex items-center gap-2">
            <div className="p-2 bg-amber-500/10 text-amber-400 rounded-lg">
              <Plus className="w-5 h-5" />
            </div>
            <h3 className="font-semibold text-lg text-white">Tambah Puzzle Baru</h3>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {error && (
            <div className="p-3 bg-rose-500/10 border border-rose-500/30 rounded-xl text-rose-400 text-sm flex items-center gap-2">
              <AlertCircle className="w-4 h-4 shrink-0" />
              <span>{error}</span>
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1">
                Nomor Puzzle <span className="text-amber-400">*</span>
              </label>
              <input
                type="number"
                placeholder="misal: 20 atau 66"
                value={puzzleNumber}
                onChange={(e) => handlePuzzleNumberChange(e.target.value ? Number(e.target.value) : '')}
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-white text-sm focus:outline-none focus:border-amber-500"
                required
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1">
                Tingkat Bits <span className="text-amber-400">*</span>
              </label>
              <input
                type="number"
                placeholder="misal: 20 atau 66"
                value={bits}
                onChange={(e) => handleBitsChange(e.target.value ? Number(e.target.value) : '')}
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-white text-sm focus:outline-none focus:border-amber-500"
                required
              />
            </div>
          </div>

          {/* Input Range Start & End HEX */}
          <div className="p-3.5 bg-slate-950/60 border border-slate-800/80 rounded-xl space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-300">
                <Hash className="w-3.5 h-3.5 text-amber-400" />
                <span>Rentang Kunci (Key Range HEX)</span>
              </div>
              {bits !== '' && (
                <button
                  type="button"
                  onClick={handleResetRanges}
                  className="text-[11px] text-amber-400 hover:text-amber-300 flex items-center gap-1 transition"
                  title="Hitung ulang rentang standar 2^(bits-1) s/d 2^bits-1"
                >
                  <Sparkles className="w-3 h-3" />
                  <span>Auto-Hitung dari Bits</span>
                </button>
              )}
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-[11px] font-medium text-slate-400 mb-1">
                  Range Start (HEX)
                </label>
                <input
                  type="text"
                  placeholder="misal: 0x80000"
                  value={rangeStartHex}
                  onChange={(e) => setRangeStartHex(e.target.value)}
                  className="w-full bg-slate-900 border border-slate-800 rounded-lg px-2.5 py-1.5 text-white text-xs font-mono focus:outline-none focus:border-amber-500"
                />
              </div>
              <div>
                <label className="block text-[11px] font-medium text-slate-400 mb-1">
                  Range End (HEX)
                </label>
                <input
                  type="text"
                  placeholder="misal: 0xfffff"
                  value={rangeEndHex}
                  onChange={(e) => setRangeEndHex(e.target.value)}
                  className="w-full bg-slate-900 border border-slate-800 rounded-lg px-2.5 py-1.5 text-white text-xs font-mono focus:outline-none focus:border-amber-500"
                />
              </div>
            </div>
            <p className="text-[10px] text-slate-500">
              Otomatis terisi saat Bits diisi. Kamu juga bisa paste rentang kustom (misal: sub-range tertentu).
            </p>
          </div>

          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="block text-xs font-medium text-slate-400">
                Bitcoin Address Target <span className="text-amber-400">*</span>
              </label>
              <button
                type="button"
                onClick={handleLookupPubkey}
                disabled={lookupLoading || !address.trim()}
                className="text-xs text-amber-400 hover:text-amber-300 flex items-center gap-1 disabled:opacity-40 transition"
              >
                {lookupLoading ? (
                  <Loader2 className="w-3 h-3 animate-spin" />
                ) : (
                  <Search className="w-3 h-3" />
                )}
                <span>Cek On-Chain Pubkey</span>
              </button>
            </div>
            <input
              type="text"
              placeholder="1HsMJxNiV7TLxmoF6uJNkydxPFDog4NQum"
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-white text-sm font-mono focus:outline-none focus:border-amber-500"
              required
            />
          </div>

          {lookupMsg && (
            <div
              className={`p-2.5 rounded-xl text-xs flex items-center gap-2 ${
                lookupMsg.type === 'success'
                  ? 'bg-emerald-500/10 border border-emerald-500/30 text-emerald-400'
                  : lookupMsg.type === 'info'
                  ? 'bg-amber-500/10 border border-amber-500/30 text-amber-300'
                  : 'bg-rose-500/10 border border-rose-500/30 text-rose-400'
              }`}
            >
              <CheckCircle2 className="w-4 h-4 shrink-0" />
              <span>{lookupMsg.text}</span>
            </div>
          )}

          {/* Toggle Pubkey Belum Terekspos */}
          <div className="p-3 bg-slate-950/70 border border-slate-800 rounded-xl">
            <label className="flex items-center gap-2 cursor-pointer select-none">
              <input
                type="checkbox"
                checked={isUnexposed}
                onChange={(e) => {
                  setIsUnexposed(e.target.checked);
                  if (e.target.checked) {
                    setPubkeyHex('');
                  }
                }}
                className="w-4 h-4 rounded text-amber-500 focus:ring-amber-500 bg-slate-900 border-slate-700"
              />
              <span className="text-sm font-medium text-slate-200">
                Public Key Belum Terekspos (Hanya Bitcoin Address)
              </span>
            </label>
            <p className="text-[11px] text-slate-400 mt-1 pl-6">
              Centang jika puzzle ini belum pernah mengirim dana on-chain. Engine akan otomatis menggunakan{' '}
              <strong className="text-amber-400">Address Hash160 Scanner</strong>.
            </p>
          </div>

          {!isUnexposed && (
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1">
                Compressed Public Key (HEX) <span className="text-amber-400">*</span>
              </label>
              <input
                type="text"
                placeholder="033c4a45cbd643ff97d77f41ea37e843648d50fd894b864b0d52febc62f6454f7c"
                value={pubkeyHex}
                onChange={(e) => setPubkeyHex(e.target.value)}
                className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-white text-sm font-mono focus:outline-none focus:border-amber-500"
                required={!isUnexposed}
              />
              <p className="text-[11px] text-slate-500 mt-1">
                Kangaroo solver O(√N) menggunakan public key 66 karakter hex.
              </p>
            </div>
          )}

          <div>
            <label className="block text-xs font-medium text-slate-400 mb-1">
              Hadiah Reward (BTC)
            </label>
            <input
              type="number"
              step="0.000001"
              placeholder="0.02"
              value={rewardBtc}
              onChange={(e) => setRewardBtc(e.target.value ? Number(e.target.value) : '')}
              className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-white text-sm focus:outline-none focus:border-amber-500"
            />
          </div>

          <div className="p-3 bg-slate-950/80 border border-slate-800/80 rounded-xl text-xs text-slate-400 space-y-1">
            <div className="flex items-center gap-1.5 text-emerald-400 font-medium">
              <KeyRound className="w-3.5 h-3.5" />
              <span>Dukungan Algoritma Ganda:</span>
            </div>
            <p>
              • <strong>Public Key Ada</strong>: Menggunakan <em>Pollard's Kangaroo Engine O(√N)</em>.<br />
              • <strong>Public Key Belum Ada</strong>: Menggunakan <em>Address Hash160 Scanner Engine</em>.
            </p>
          </div>

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-sm transition"
            >
              Batal
            </button>
            <button
              type="submit"
              disabled={loading}
              className="px-5 py-2 bg-amber-500 hover:bg-amber-400 text-slate-950 font-semibold rounded-xl text-sm transition disabled:opacity-50"
            >
              {loading ? 'Menyimpan...' : 'Simpan ke Database'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
