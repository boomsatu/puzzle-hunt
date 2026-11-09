import React, { useState } from 'react';
import { X, Key, Copy, Check, ExternalLink } from 'lucide-react';
import { FoundKey } from '../types';

interface FoundKeysModalProps {
  isOpen: boolean;
  onClose: () => void;
  keys: FoundKey[];
}

export const FoundKeysModal: React.FC<FoundKeysModalProps> = ({ isOpen, onClose, keys }) => {
  const [copiedWif, setCopiedWif] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleCopy = (wif: string) => {
    navigator.clipboard.writeText(wif);
    setCopiedWif(wif);
    setTimeout(() => setCopiedWif(null), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4">
      <div className="bg-slate-900 border border-emerald-500/30 rounded-2xl w-full max-w-3xl overflow-hidden shadow-2xl animate-in fade-in zoom-in-95 duration-150">
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-emerald-950/20">
          <div className="flex items-center gap-2">
            <div className="p-2 bg-emerald-500/20 text-emerald-400 rounded-lg">
              <Key className="w-5 h-5" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-white">Daftar Private Key Ditemukan!</h3>
              <p className="text-xs text-slate-400">
                Tersimpan permanen di tabel PostgreSQL <code className="text-emerald-400 font-mono">found_keys</code>
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-6 max-h-[70vh] overflow-y-auto space-y-4">
          {keys.length === 0 ? (
            <div className="text-center py-12 text-slate-500">
              <Key className="w-12 h-12 mx-auto mb-3 opacity-30" />
              <p className="text-sm">Belum ada kunci yang ditemukan saat ini.</p>
              <p className="text-xs text-slate-600 mt-1">
                Kunci akan otomatis muncul di sini dan tersimpan di database saat terjadi collision!
              </p>
            </div>
          ) : (
            keys.map((k) => (
              <div
                key={k.id}
                className="p-4 bg-slate-950 border border-emerald-500/40 rounded-xl space-y-3"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="px-2.5 py-0.5 bg-emerald-500/20 text-emerald-400 rounded-full text-xs font-bold">
                      Puzzle #{k.puzzle_number} SOLVED
                    </span>
                    <span className="text-xs text-slate-400">
                      {new Date(k.found_at).toLocaleString()}
                    </span>
                  </div>
                  <a
                    href={`https://www.blockchain.com/explorer/addresses/btc/${k.address}`}
                    target="_blank"
                    rel="noreferrer"
                    className="text-xs text-slate-400 hover:text-emerald-400 flex items-center gap-1 transition"
                    title="Lihat di Blockchain.com Explorer"
                  >
                    Explorer <ExternalLink className="w-3 h-3" />
                  </a>
                </div>

                <div>
                  <div className="text-[11px] text-slate-400 uppercase tracking-wider font-semibold">
                    Bitcoin Address
                  </div>
                  <div className="text-xs font-mono text-slate-200">{k.address}</div>
                </div>

                <div>
                  <div className="text-[11px] text-slate-400 uppercase tracking-wider font-semibold">
                    Private Key WIF (Compressed) — Langsung Import ke Wallet
                  </div>
                  <div className="flex items-center gap-2 mt-1">
                    <input
                      type="text"
                      readOnly
                      value={k.wif}
                      className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-1.5 text-xs font-mono text-emerald-300 focus:outline-none"
                    />
                    <button
                      onClick={() => handleCopy(k.wif)}
                      className="px-3 py-1.5 bg-emerald-500 hover:bg-emerald-400 text-slate-950 text-xs font-semibold rounded-lg flex items-center gap-1.5 transition shrink-0"
                    >
                      {copiedWif === k.wif ? (
                        <>
                          <Check className="w-3.5 h-3.5" /> Disalin
                        </>
                      ) : (
                        <>
                          <Copy className="w-3.5 h-3.5" /> Salin WIF
                        </>
                      )}
                    </button>
                  </div>
                </div>

                <div className="grid grid-cols-2 gap-2 text-xs">
                  <div>
                    <span className="text-slate-500 text-[10px] uppercase">Private Key HEX:</span>
                    <div className="font-mono text-slate-400 truncate text-[11px]">
                      0x{k.private_key_hex}
                    </div>
                  </div>
                  <div>
                    <span className="text-slate-500 text-[10px] uppercase">TxID Auto-Transfer:</span>
                    <div className="font-mono text-slate-400 truncate text-[11px]">
                      {k.txid ? (
                        <a
                          href={`https://www.blockchain.com/explorer/transactions/btc/${k.txid}`}
                          target="_blank"
                          rel="noreferrer"
                          className="text-emerald-400 hover:underline inline-flex items-center gap-1"
                          title="Lihat Transaksi di Blockchain.com"
                        >
                          {k.txid.slice(0, 18)}... <ExternalLink className="w-2.5 h-2.5" />
                        </a>
                      ) : (
                        'Tidak ada (Manual Sweep)'
                      )}
                    </div>
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
};

