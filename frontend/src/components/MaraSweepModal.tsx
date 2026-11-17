import React, { useState, useEffect } from 'react';
import {
  X,
  Zap,
  Key,
  Database,
  FileText,
  Copy,
  Check,
  ExternalLink,
  AlertTriangle,
  ArrowRight,
  ShieldCheck,
  RefreshCw,
  Send,
  Lock,
  Search,
  CheckCircle2,
  Sliders,
} from 'lucide-react';
import { SavedKeyItem, SweepPreview, ManualSweepResult } from '../types';

interface MaraSweepModalProps {
  isOpen: boolean;
  onClose: () => void;
  destinationAddress: string;
  onSuccess?: () => void;
}

export const MaraSweepModal: React.FC<MaraSweepModalProps> = ({
  isOpen,
  onClose,
  destinationAddress: initialDestination,
  onSuccess,
}) => {
  // Source selection
  const [sourceType, setSourceType] = useState<'manual' | 'saved'>('saved');
  const [manualKey, setManualKey] = useState<string>('');
  const [puzzleNumber, setPuzzleNumber] = useState<number>(71);

  // Saved keys
  const [savedKeys, setSavedKeys] = useState<SavedKeyItem[]>([]);
  const [selectedKey, setSelectedKey] = useState<SavedKeyItem | null>(null);
  const [isLoadingKeys, setIsLoadingKeys] = useState<boolean>(false);
  const [searchFilter, setSearchFilter] = useState<string>('');

  // Transfer options
  const [destinationAddress, setDestinationAddress] = useState<string>(
    initialDestination || 'bc1qz08nklntedluzzr9w0g3qhxrkvytayentqdshc'
  );
  const [priorityFeeRate, setPriorityFeeRate] = useState<number>(350);
  const [maraOnly, setMaraOnly] = useState<boolean>(true);
  const [showAdvanced, setShowAdvanced] = useState<boolean>(false);

  // Preview & Execution
  const [preview, setPreview] = useState<SweepPreview | null>(null);
  const [isLoadingPreview, setIsLoadingPreview] = useState<boolean>(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  const [isExecuting, setIsExecuting] = useState<boolean>(false);
  const [executionResult, setExecutionResult] = useState<ManualSweepResult | null>(null);
  const [executionError, setExecutionError] = useState<string | null>(null);
  const [showConfirm, setShowConfirm] = useState<boolean>(false);

  // Copy helper
  const [copiedText, setCopiedText] = useState<string | null>(null);

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedText(text);
    setTimeout(() => setCopiedText(null), 2000);
  };

  // Sync initialDestination
  useEffect(() => {
    if (initialDestination) {
      setDestinationAddress(initialDestination);
    }
  }, [initialDestination]);

  // Load saved keys on modal open
  useEffect(() => {
    if (isOpen) {
      fetchSavedKeys();
      // Reset errors & results
      setPreviewError(null);
      setExecutionError(null);
      setExecutionResult(null);
      setShowConfirm(false);
    }
  }, [isOpen]);

  const fetchSavedKeys = async () => {
    setIsLoadingKeys(true);
    try {
      const res = await fetch('/api/mara/keys');
      if (res.ok) {
        const data = await res.json();
        const keys: SavedKeyItem[] = data.keys || [];
        setSavedKeys(keys);
        if (keys.length > 0 && !selectedKey) {
          setSelectedKey(keys[0]);
        }
      }
    } catch (e) {
      console.error('Gagal mengambil kunci tersimpan:', e);
    } finally {
      setIsLoadingKeys(false);
    }
  };

  // Determine current active private key
  const getActiveKey = (): string => {
    if (sourceType === 'manual') {
      return manualKey.trim();
    } else {
      return selectedKey?.wif || selectedKey?.private_key_hex || '';
    }
  };

  // Detect key format for manual input
  const detectKeyFormat = (val: string): { type: string; badge: string; color: string } => {
    const clean = val.trim();
    if (!clean) return { type: 'none', badge: 'Menunggu input', color: 'text-slate-500' };
    if (clean.startsWith('5') || clean.startsWith('K') || clean.startsWith('L')) {
      return { type: 'wif', badge: 'Format: WIF (Base58)', color: 'text-emerald-400' };
    }
    const hexClean = clean.replace(/^0x/i, '');
    if (hexClean.length === 64 && /^[0-9a-fA-F]+$/.test(hexClean)) {
      return { type: 'hex', badge: 'Format: 64-Hex Scalar', color: 'text-blue-400' };
    }
    if (/^\d+$/.test(clean)) {
      return { type: 'dec', badge: 'Format: Big Decimal', color: 'text-purple-400' };
    }
    return { type: 'unknown', badge: 'Format belum lengkap/valid', color: 'text-amber-400' };
  };

  // Preview sweep
  const handlePreview = async () => {
    const key = getActiveKey();
    if (!key) {
      setPreviewError('Private key belum dipilih atau diisi.');
      return;
    }

    setIsLoadingPreview(true);
    setPreviewError(null);
    setPreview(null);
    setExecutionResult(null);

    try {
      const res = await fetch('/api/mara/preview', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          source_type: sourceType === 'manual' ? 'manual' : 'saved_key',
          private_key: key,
          destination_address: destinationAddress,
          priority_fee_rate: priorityFeeRate,
        }),
      });

      const data = await res.json();
      if (res.ok && data.success) {
        setPreview(data);
      } else {
        setPreviewError(data.error || 'Gagal memeriksa preview saldo & fee.');
      }
    } catch (e: any) {
      setPreviewError(`Koneksi error: ${e.message || e}`);
    } finally {
      setIsLoadingPreview(false);
    }
  };

  // Execute sweep
  const handleExecute = async () => {
    const key = getActiveKey();
    if (!key) return;

    setShowConfirm(false);
    setIsExecuting(true);
    setExecutionError(null);

    const pNum = sourceType === 'manual' ? puzzleNumber : selectedKey?.puzzle_number || 71;

    try {
      const res = await fetch('/api/mara/sweep', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          source_type: sourceType === 'manual' ? 'manual' : 'saved_key',
          private_key: key,
          puzzle_number: pNum,
          destination_address: destinationAddress,
          priority_fee_rate: priorityFeeRate,
          mara_only: maraOnly,
        }),
      });

      const data = await res.json();
      if (res.ok && data.success) {
        setExecutionResult(data);
        fetchSavedKeys(); // refresh list to show updated txid
        if (onSuccess) onSuccess();
      } else {
        setExecutionError(data.error || 'Eksekusi sweep ke MARA Slipstream ditolak/gagal.');
      }
    } catch (e: any) {
      setExecutionError(`Koneksi error: ${e.message || e}`);
    } finally {
      setIsExecuting(false);
    }
  };

  if (!isOpen) return null;

  // Filtered saved keys
  const filteredKeys = savedKeys.filter((k) => {
    const q = searchFilter.toLowerCase();
    return (
      k.puzzle_number.toString().includes(q) ||
      k.address.toLowerCase().includes(q) ||
      k.wif.toLowerCase().includes(q) ||
      k.private_key_hex.toLowerCase().includes(q) ||
      (k.file_name && k.file_name.toLowerCase().includes(q))
    );
  });

  const activeKeyStr = getActiveKey();
  const formatInfo = detectKeyFormat(activeKeyStr);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-md p-3 sm:p-4 overflow-y-auto">
      <div className="bg-slate-900 border border-orange-500/30 rounded-2xl w-full max-w-3xl overflow-hidden shadow-2xl animate-in fade-in zoom-in-95 duration-150 my-auto flex flex-col max-h-[92vh]">
        {/* Modal Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-gradient-to-r from-orange-950/40 to-slate-900 shrink-0">
          <div className="flex items-center gap-3">
            <div className="p-2.5 bg-orange-500/20 text-orange-400 rounded-xl border border-orange-500/30">
              <Zap className="w-5 h-5 animate-pulse" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h3 className="font-bold text-lg text-white">MARA Slipstream Sweep Engine</h3>
                <span className="px-2 py-0.5 bg-orange-500/20 text-orange-400 border border-orange-500/30 rounded-full text-[10px] font-mono font-bold">
                  Bypass Mempool
                </span>
              </div>
              <p className="text-xs text-slate-400 mt-0.5">
                Kirim saldo langsung ke penambang privat MARA Slipstream (bebas front-running bot)
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1.5 rounded-lg hover:bg-slate-800 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Modal Body */}
        <div className="p-5 sm:p-6 overflow-y-auto space-y-5 flex-1">
          {/* Result Banner if already executed */}
          {executionResult && (
            <div className="p-4 bg-emerald-950/40 border border-emerald-500/50 rounded-xl space-y-3">
              <div className="flex items-center gap-2 text-emerald-400 font-bold text-sm">
                <CheckCircle2 className="w-5 h-5 text-emerald-400" />
                <span>Transaksi Berhasil Dikirim!</span>
                <span className="ml-auto text-xs font-mono px-2 py-0.5 bg-emerald-500/20 rounded">
                  {executionResult.is_mara_accepted ? '🔒 MARA Accepted' : '📡 Mempool Broadcast'}
                </span>
              </div>
              <p className="text-xs text-slate-300">{executionResult.message}</p>

              <div className="space-y-1.5 pt-2 border-t border-emerald-800/40">
                <div className="text-[11px] text-slate-400">Transaction ID (TxID):</div>
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    readOnly
                    value={executionResult.txid}
                    className="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-1.5 text-xs font-mono text-emerald-300"
                  />
                  <button
                    onClick={() => handleCopy(executionResult.txid)}
                    className="px-2.5 py-1.5 bg-emerald-500/20 hover:bg-emerald-500/30 text-emerald-400 text-xs rounded-lg flex items-center gap-1 transition shrink-0"
                  >
                    {copiedText === executionResult.txid ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                  </button>
                  <a
                    href={executionResult.explorer_url}
                    target="_blank"
                    rel="noreferrer"
                    className="px-2.5 py-1.5 bg-blue-500/20 hover:bg-blue-500/30 text-blue-400 text-xs rounded-lg flex items-center gap-1 transition shrink-0"
                  >
                    Mempool <ExternalLink className="w-3.5 h-3.5" />
                  </a>
                </div>
              </div>

              <div className="grid grid-cols-2 sm:grid-cols-3 gap-2 pt-2 text-xs">
                <div className="bg-slate-900/80 p-2 rounded-lg border border-slate-800">
                  <div className="text-slate-400 text-[10px]">Net Terkirim</div>
                  <div className="font-mono text-emerald-400 font-bold">
                    {executionResult.amount_btc.toFixed(8)} BTC
                  </div>
                </div>
                <div className="bg-slate-900/80 p-2 rounded-lg border border-slate-800">
                  <div className="text-slate-400 text-[10px]">Biaya Miner</div>
                  <div className="font-mono text-amber-400 font-bold">
                    {executionResult.fee_sats.toLocaleString()} sat ({executionResult.fee_rate_sat_vb} sat/vB)
                  </div>
                </div>
                <div className="bg-slate-900/80 p-2 rounded-lg border border-slate-800 col-span-2 sm:col-span-1">
                  <div className="text-slate-400 text-[10px]">Tujuan</div>
                  <div className="font-mono text-slate-300 text-[11px] truncate" title={executionResult.destination_address}>
                    {executionResult.destination_address.slice(0, 8)}...{executionResult.destination_address.slice(-6)}
                  </div>
                </div>
              </div>

              <div className="pt-2 flex justify-end">
                <button
                  onClick={() => {
                    setExecutionResult(null);
                    setPreview(null);
                  }}
                  className="text-xs text-slate-400 hover:text-white underline transition"
                >
                  Sapu Kunci Lain
                </button>
              </div>
            </div>
          )}

          {/* Key Source Tabs */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-300 flex items-center gap-2">
                <span>1. Pilih Sumber Private Key</span>
              </label>
              <span className={`text-[11px] font-mono font-medium ${formatInfo.color}`}>
                {formatInfo.badge}
              </span>
            </div>

            <div className="grid grid-cols-2 gap-2 p-1 bg-slate-950 border border-slate-800 rounded-xl">
              <button
                type="button"
                onClick={() => {
                  setSourceType('saved');
                  setPreview(null);
                  setPreviewError(null);
                }}
                className={`py-2 px-3 rounded-lg text-xs font-semibold flex items-center justify-center gap-2 transition ${
                  sourceType === 'saved'
                    ? 'bg-orange-500 text-slate-950 font-bold shadow-md shadow-orange-500/20'
                    : 'text-slate-400 hover:text-slate-200'
                }`}
              >
                <Database className="w-3.5 h-3.5" />
                <span>Kunci Tersimpan ({savedKeys.length})</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  setSourceType('manual');
                  setPreview(null);
                  setPreviewError(null);
                }}
                className={`py-2 px-3 rounded-lg text-xs font-semibold flex items-center justify-center gap-2 transition ${
                  sourceType === 'manual'
                    ? 'bg-orange-500 text-slate-950 font-bold shadow-md shadow-orange-500/20'
                    : 'text-slate-400 hover:text-slate-200'
                }`}
              >
                <Key className="w-3.5 h-3.5" />
                <span>Input Manual (Hex / WIF)</span>
              </button>
            </div>
          </div>

          {/* Tab 1 Content: Saved Keys List */}
          {sourceType === 'saved' && (
            <div className="space-y-3">
              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
                  <input
                    type="text"
                    value={searchFilter}
                    onChange={(e) => setSearchFilter(e.target.value)}
                    placeholder="Cari berdasarkan puzzle #, address, WIF, atau nama berkas..."
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl pl-9 pr-3 py-1.5 text-xs text-slate-200 placeholder-slate-600 focus:outline-none focus:border-orange-500/50"
                  />
                </div>
                <button
                  type="button"
                  onClick={fetchSavedKeys}
                  disabled={isLoadingKeys}
                  className="px-2.5 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-xs flex items-center gap-1 transition"
                  title="Refresh daftar kunci tersimpan"
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${isLoadingKeys ? 'animate-spin' : ''}`} />
                </button>
              </div>

              <div className="max-h-52 overflow-y-auto space-y-2 pr-1">
                {isLoadingKeys ? (
                  <div className="text-center py-8 text-xs text-slate-500">
                    <RefreshCw className="w-5 h-5 mx-auto mb-2 animate-spin text-orange-400" />
                    Memuat daftar kunci dari PostgreSQL & disk...
                  </div>
                ) : filteredKeys.length === 0 ? (
                  <div className="text-center py-8 text-xs text-slate-500 bg-slate-950/50 border border-slate-800/80 rounded-xl p-4">
                    <FileText className="w-8 h-8 mx-auto mb-2 opacity-30 text-slate-400" />
                    Belum ada kunci yang ditemukan di PostgreSQL maupun berkas backup disk.
                    <div className="mt-2 text-slate-400">
                      Anda bisa beralih ke tab <span className="text-orange-400 font-semibold">Input Manual</span> jika memegang private key secara langsung.
                    </div>
                  </div>
                ) : (
                  filteredKeys.map((k, idx) => {
                    const isSelected =
                      selectedKey?.private_key_hex === k.private_key_hex ||
                      selectedKey?.wif === k.wif;
                    return (
                      <div
                        key={idx}
                        onClick={() => {
                          setSelectedKey(k);
                          setPreview(null);
                        }}
                        className={`p-3 rounded-xl border cursor-pointer transition flex flex-col gap-1.5 ${
                          isSelected
                            ? 'bg-orange-500/10 border-orange-500/60 shadow-sm shadow-orange-500/10'
                            : 'bg-slate-950/70 border-slate-800 hover:border-slate-700'
                        }`}
                      >
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-2">
                            <span className="px-2 py-0.5 bg-emerald-500/20 text-emerald-400 rounded-md text-[10px] font-bold">
                              Puzzle #{k.puzzle_number}
                            </span>
                            <span
                              className={`px-2 py-0.5 rounded-md text-[10px] font-mono flex items-center gap-1 ${
                                k.source === 'database'
                                  ? 'bg-blue-500/20 text-blue-400 border border-blue-500/30'
                                  : 'bg-amber-500/20 text-amber-400 border border-amber-500/30'
                              }`}
                            >
                              {k.source === 'database' ? (
                                <>
                                  <Database className="w-2.5 h-2.5" /> PostgreSQL
                                </>
                              ) : (
                                <>
                                  <FileText className="w-2.5 h-2.5" /> {k.file_name || 'File Backup'}
                                </>
                              )}
                            </span>
                            {k.txid && (
                              <span className="px-2 py-0.5 bg-purple-500/20 text-purple-300 rounded-md text-[10px] font-mono">
                                Sudah Disapu
                              </span>
                            )}
                          </div>
                          <span className="text-[10px] text-slate-500">
                            {new Date(k.found_at).toLocaleDateString()}
                          </span>
                        </div>

                        <div className="flex items-center justify-between text-xs">
                          <span className="font-mono text-slate-300">{k.address}</span>
                          <span className="font-mono text-[11px] text-slate-500 truncate max-w-[200px]">
                            {k.wif}
                          </span>
                        </div>
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          )}

          {/* Tab 2 Content: Manual Key Input */}
          {sourceType === 'manual' && (
            <div className="space-y-3">
              <div className="flex items-center gap-3">
                <div className="w-32">
                  <label className="text-[11px] text-slate-400 block mb-1">Puzzle #</label>
                  <input
                    type="number"
                    value={puzzleNumber}
                    onChange={(e) => setPuzzleNumber(parseInt(e.target.value) || 71)}
                    className="w-full bg-slate-950 border border-slate-800 rounded-xl px-3 py-2 text-xs font-mono text-white focus:outline-none focus:border-orange-500/50"
                  />
                </div>
                <div className="flex-1">
                  <label className="text-[11px] text-slate-400 block mb-1">
                    Format Kunci yang Didukung:
                  </label>
                  <div className="text-[11px] text-slate-400 flex items-center gap-2">
                    <span className="px-1.5 py-0.5 bg-slate-800 rounded text-slate-300 font-mono">WIF (Kw/5...)</span>
                    <span className="px-1.5 py-0.5 bg-slate-800 rounded text-slate-300 font-mono">64-Hex (0x...)</span>
                    <span className="px-1.5 py-0.5 bg-slate-800 rounded text-slate-300 font-mono">Desimal</span>
                  </div>
                </div>
              </div>

              <div>
                <textarea
                  rows={3}
                  value={manualKey}
                  onChange={(e) => {
                    setManualKey(e.target.value);
                    setPreview(null);
                  }}
                  placeholder="Tempelkan private key di sini (WIF, Hex, atau Desimal)..."
                  className="w-full bg-slate-950 border border-slate-800 rounded-xl p-3 text-xs font-mono text-emerald-300 placeholder-slate-600 focus:outline-none focus:border-orange-500/50 resize-none"
                />
              </div>
            </div>
          )}

          {/* Destination & Fee Settings (Collapsible / Summary) */}
          <div className="bg-slate-950/60 border border-slate-800/80 rounded-xl p-3 space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold text-slate-300 flex items-center gap-2">
                <Sliders className="w-3.5 h-3.5 text-orange-400" />
                <span>2. Parameter MARA Slipstream & Fee</span>
              </span>
              <button
                type="button"
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="text-[11px] text-orange-400 hover:text-orange-300 transition"
              >
                {showAdvanced ? 'Sembunyikan Opsi' : 'Ubah Opsi'}
              </button>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-xs">
              <div>
                <div className="text-[10px] text-slate-400 uppercase font-semibold">Alamat Penerima</div>
                <div className="font-mono text-emerald-400 font-bold truncate mt-0.5" title={destinationAddress}>
                  {destinationAddress}
                </div>
              </div>

              <div>
                <div className="text-[10px] text-slate-400 uppercase font-semibold">Rute & Fee</div>
                <div className="font-mono text-slate-300 mt-0.5 flex items-center gap-1.5">
                  <span className="px-1.5 py-0.2 bg-orange-500/20 text-orange-400 rounded text-[10px] font-bold">
                    {maraOnly ? 'MARA Only (0% Mempool)' : 'MARA + Fallback RPC'}
                  </span>
                  <span className="text-amber-400 font-bold">{priorityFeeRate} sat/vB</span>
                </div>
              </div>
            </div>

            {showAdvanced && (
              <div className="pt-3 border-t border-slate-800 space-y-3 animate-in fade-in duration-100">
                <div>
                  <label className="text-[11px] text-slate-400 block mb-1">
                    Alamat Bitcoin Penerima (Destination)
                  </label>
                  <input
                    type="text"
                    value={destinationAddress}
                    onChange={(e) => {
                      setDestinationAddress(e.target.value.trim());
                      setPreview(null);
                    }}
                    className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-1.5 text-xs font-mono text-emerald-300 focus:outline-none focus:border-orange-500"
                  />
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div>
                    <label className="text-[11px] text-slate-400 block mb-1">
                      Priority Fee Rate (sat/vB)
                    </label>
                    <input
                      type="number"
                      min={10}
                      max={2000}
                      value={priorityFeeRate}
                      onChange={(e) => {
                        setPriorityFeeRate(parseInt(e.target.value) || 350);
                        setPreview(null);
                      }}
                      className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-1.5 text-xs font-mono text-white focus:outline-none focus:border-orange-500"
                    />
                  </div>

                  <div className="flex items-center gap-2 pt-4">
                    <input
                      type="checkbox"
                      id="maraOnlyCheckbox"
                      checked={maraOnly}
                      onChange={(e) => setMaraOnly(e.target.checked)}
                      className="rounded border-slate-700 text-orange-500 focus:ring-0 w-4 h-4"
                    />
                    <label htmlFor="maraOnlyCheckbox" className="text-xs text-slate-300 cursor-pointer">
                      <span className="font-semibold text-orange-400">MARA Slipstream Only</span>
                      <span className="block text-[10px] text-slate-500">
                        Jangan siarkan ke mempool jika MARA offline (proteksi bot)
                      </span>
                    </label>
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Action: Preview Button */}
          <div>
            <button
              type="button"
              onClick={handlePreview}
              disabled={isLoadingPreview || !activeKeyStr}
              className={`w-full py-2.5 px-4 rounded-xl text-xs font-bold flex items-center justify-center gap-2 transition ${
                isLoadingPreview || !activeKeyStr
                  ? 'bg-slate-800 text-slate-500 cursor-not-allowed'
                  : 'bg-gradient-to-r from-orange-500 to-amber-500 hover:from-orange-400 hover:to-amber-400 text-slate-950 shadow-md shadow-orange-500/20'
              }`}
            >
              {isLoadingPreview ? (
                <>
                  <RefreshCw className="w-4 h-4 animate-spin" />
                  <span>Memeriksa Saldo On-Chain & Menghitung Fee...</span>
                </>
              ) : (
                <>
                  <Zap className="w-4 h-4" />
                  <span>🔍 Preview Saldo & Estimasi Fee MARA</span>
                </>
              )}
            </button>
          </div>

          {/* Preview Error Display */}
          {previewError && (
            <div className="p-3 bg-rose-500/10 border border-rose-500/30 rounded-xl text-xs text-rose-400 flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
              <div>
                <span className="font-bold">Error: </span>
                <span>{previewError}</span>
              </div>
            </div>
          )}

          {/* Preview Panel Card */}
          {preview && (
            <div className="p-4 bg-slate-950 border border-orange-500/40 rounded-xl space-y-3 animate-in fade-in duration-150">
              <div className="flex items-center justify-between border-b border-slate-800 pb-2">
                <div className="text-xs font-bold text-white flex items-center gap-2">
                  <ShieldCheck className="w-4 h-4 text-emerald-400" />
                  <span>Hasil Verifikasi & Preview Sweep</span>
                </div>
                <span
                  className={`px-2 py-0.5 rounded-full text-[10px] font-bold ${
                    preview.can_sweep
                      ? 'bg-emerald-500/20 text-emerald-400'
                      : 'bg-amber-500/20 text-amber-400'
                  }`}
                >
                  {preview.can_sweep ? '✅ Siap Disapu' : '⚠️ Saldo Tidak Cukup'}
                </span>
              </div>

              <div>
                <div className="text-[10px] text-slate-400 uppercase font-semibold">Alamat Sumber (Tervalidasi)</div>
                <div className="flex items-center justify-between">
                  <span className="font-mono text-xs text-slate-200">{preview.source_address}</span>
                  <a
                    href={`https://mempool.space/address/${preview.source_address}`}
                    target="_blank"
                    rel="noreferrer"
                    className="text-[11px] text-blue-400 hover:text-blue-300 flex items-center gap-1"
                  >
                    Mempool <ExternalLink className="w-3 h-3" />
                  </a>
                </div>
              </div>

              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2 text-xs pt-1">
                <div className="bg-slate-900 p-2.5 rounded-lg border border-slate-800">
                  <div className="text-slate-400 text-[10px]">Saldo On-Chain</div>
                  <div className="font-mono text-white font-bold">{preview.balance_btc.toFixed(8)} BTC</div>
                  <div className="text-[10px] text-slate-500 font-mono">
                    {preview.balance_sats.toLocaleString()} sats ({preview.utxo_count} UTXO)
                  </div>
                </div>

                <div className="bg-slate-900 p-2.5 rounded-lg border border-slate-800">
                  <div className="text-slate-400 text-[10px]">Estimasi Fee Miner</div>
                  <div className="font-mono text-amber-400 font-bold">{preview.estimated_fee_btc.toFixed(8)} BTC</div>
                  <div className="text-[10px] text-slate-500 font-mono">
                    {preview.estimated_fee_sats.toLocaleString()} sat ({preview.fee_rate_sat_vb} sat/vB)
                  </div>
                </div>

                <div className="bg-slate-900 p-2.5 rounded-lg border border-emerald-500/30 col-span-2">
                  <div className="text-emerald-400 text-[10px] font-semibold flex items-center gap-1">
                    <span>Estimasi Bersih Diterima di Tujuan</span>
                    <ArrowRight className="w-3 h-3" />
                  </div>
                  <div className="font-mono text-emerald-400 font-bold text-sm">
                    {preview.net_send_btc.toFixed(8)} BTC
                  </div>
                  <div className="text-[10px] text-slate-400 font-mono truncate" title={preview.destination_address}>
                    Ke: {preview.destination_address}
                  </div>
                </div>
              </div>

              <p className="text-[11px] text-slate-400 italic pt-1">{preview.message}</p>

              {/* Sweep Trigger Button */}
              {preview.can_sweep && !showConfirm && (
                <div className="pt-2">
                  <button
                    type="button"
                    onClick={() => setShowConfirm(true)}
                    className="w-full py-2.5 px-4 bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-bold rounded-xl text-xs flex items-center justify-center gap-2 shadow-lg shadow-emerald-500/20 transition"
                  >
                    <Send className="w-4 h-4" />
                    <span>🚀 Kirim Sekarang ke MARA Slipstream</span>
                  </button>
                </div>
              )}

              {/* Confirm Step */}
              {showConfirm && (
                <div className="p-3 bg-amber-500/10 border border-amber-500/40 rounded-xl space-y-2.5 animate-in fade-in duration-100">
                  <div className="flex items-center gap-2 text-xs font-bold text-amber-400">
                    <AlertTriangle className="w-4 h-4" />
                    <span>Konfirmasi Pengiriman MARA Slipstream</span>
                  </div>
                  <p className="text-xs text-slate-300">
                    Transaksi akan langsung ditandatangani dan dikirim ke template penambangan MARA Slipstream.
                    Tindakan ini permanen dan tidak dapat dibatalkan.
                  </p>
                  <div className="flex items-center justify-end gap-2 pt-1">
                    <button
                      type="button"
                      onClick={() => setShowConfirm(false)}
                      className="px-3 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs font-semibold rounded-lg transition"
                    >
                      Batal
                    </button>
                    <button
                      type="button"
                      onClick={handleExecute}
                      disabled={isExecuting}
                      className="px-4 py-1.5 bg-emerald-500 hover:bg-emerald-400 text-slate-950 text-xs font-bold rounded-lg flex items-center gap-1.5 transition shadow-md shadow-emerald-500/20"
                    >
                      {isExecuting ? (
                        <>
                          <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                          <span>Menyapu...</span>
                        </>
                      ) : (
                        <>
                          <Check className="w-3.5 h-3.5" />
                          <span>YA, Eksekusi Sekarang!</span>
                        </>
                      )}
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Execution Error */}
          {executionError && (
            <div className="p-3 bg-rose-500/10 border border-rose-500/30 rounded-xl text-xs text-rose-400 flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
              <div>
                <span className="font-bold">Gagal Eksekusi: </span>
                <span>{executionError}</span>
              </div>
            </div>
          )}
        </div>

        {/* Modal Footer */}
        <div className="px-6 py-3.5 border-t border-slate-800 bg-slate-950 flex items-center justify-between text-xs text-slate-500 shrink-0">
          <div className="flex items-center gap-1.5">
            <Lock className="w-3.5 h-3.5 text-emerald-400" />
            <span>Private Mempool Protection (MARA Slipstream)</span>
          </div>
          <button
            onClick={onClose}
            className="px-4 py-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg text-xs font-semibold transition"
          >
            Tutup
          </button>
        </div>
      </div>
    </div>
  );
};
