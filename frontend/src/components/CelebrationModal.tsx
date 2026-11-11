import React, { useEffect, useState, useMemo } from 'react';
import confetti from 'canvas-confetti';
import {
  Trophy,
  CheckCircle2,
  Clock,
  Loader2,
  AlertCircle,
  Copy,
  Check,
  ExternalLink,
  X,
  Zap,
  ArrowRight,
  ShieldCheck,
  Sparkles,
  Wallet,
  Lock,
  Layers,
  Flame,
  Volume2,
  VolumeX,
} from 'lucide-react';
import { SweepTimelineEvent, TimelineStage } from '../types';
import {
  playJackpotFanfare,
  playSweepSuccessFanfare,
  playStepSuccessSound,
  isSoundEnabled,
  toggleSound,
} from '../sound';

interface CelebrationModalProps {
  isOpen: boolean;
  onClose: () => void;
  puzzleNumber: number;
  events: SweepTimelineEvent[];
  destinationAddress: string;
}

interface StepDefinition {
  stage: TimelineStage;
  num: number;
  label: string;
  defaultTitle: string;
  defaultDesc: string;
}

const STEPS: StepDefinition[] = [
  {
    stage: 'key_found',
    num: 1,
    label: 'Collision Key',
    defaultTitle: 'Private Key Ditemukan',
    defaultDesc: 'Kangaroo collision trap terdeteksi dan private key berhasil dipecahkan.',
  },
  {
    stage: 'fetch_utxo',
    num: 2,
    label: 'UTXO & Saldo',
    defaultTitle: 'Pengecekan UTXO On-Chain',
    defaultDesc: 'Memeriksa koin unspent dan saldo aktif di alamat Bitcoin puzzle.',
  },
  {
    stage: 'prepare_tx',
    num: 3,
    label: 'Sign Transaksi',
    defaultTitle: 'Penandatanganan Raw Transaksi',
    defaultDesc: 'Menghitung priority fee anti front-running dan menandatangani dengan ECDSA secp256k1.',
  },
  {
    stage: 'broadcast_mempool',
    num: 4,
    label: 'Broadcast Mempool',
    defaultTitle: 'Penyiaran ke Jaringan Bitcoin',
    defaultDesc: 'Menyiarkan transaksi langsung ke node Bitcoin RPC dan mempool publik.',
  },
  {
    stage: 'sweep_completed',
    num: 5,
    label: 'Auto-Transfer Sukses',
    defaultTitle: 'Dana Berhasil Disapu',
    defaultDesc: 'Transaksi sukses diterima jaringan dan dialihkan ke dompet tujuan Anda.',
  },
  {
    stage: 'confirmed',
    num: 6,
    label: 'Konfirmasi Blok',
    defaultTitle: 'Terkonfirmasi di Blok Bitcoin',
    defaultDesc: 'Transaksi telah masuk ke dalam blok penambang Bitcoin.',
  },
];

export const CelebrationModal: React.FC<CelebrationModalProps> = ({
  isOpen,
  onClose,
  puzzleNumber,
  events,
  destinationAddress,
}) => {
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [soundOn, setSoundOn] = useState<boolean>(() => isSoundEnabled());
  const [lastPlayedStage, setLastPlayedStage] = useState<string | null>(null);

  // Trigger confetti burst & fanfare sound saat modal terbuka
  useEffect(() => {
    if (!isOpen) return;

    // Mainkan fanfare kemegahan jackpot
    playJackpotFanfare();

    // First burst
    confetti({
      particleCount: 80,
      spread: 70,
      origin: { y: 0.6 },
      colors: ['#10b981', '#f59e0b', '#3b82f6', '#ec4899', '#ffffff'],
    });

    const timer1 = setTimeout(() => {
      confetti({
        particleCount: 50,
        angle: 60,
        spread: 55,
        origin: { x: 0 },
        colors: ['#10b981', '#f59e0b', '#fbbf24'],
      });
      confetti({
        particleCount: 50,
        angle: 120,
        spread: 55,
        origin: { x: 1 },
        colors: ['#10b981', '#f59e0b', '#fbbf24'],
      });
    }, 400);

    return () => clearTimeout(timer1);
  }, [isOpen]);

  // Mainkan efek suara & confetti sesuai tahapan timeline real-time
  useEffect(() => {
    if (!isOpen || events.length === 0) return;
    const latest = events[events.length - 1];
    if (!latest || latest.status !== 'success') return;

    const eventKey = `${latest.stage}_${latest.status}`;
    if (eventKey === lastPlayedStage) return;
    setLastPlayedStage(eventKey);

    if (latest.stage === 'sweep_completed') {
      playSweepSuccessFanfare();
      confetti({
        particleCount: 110,
        spread: 100,
        origin: { y: 0.4 },
        colors: ['#10b981', '#34d399', '#fbbf24', '#f59e0b'],
      });
    } else if (latest.stage !== 'key_found') {
      playStepSuccessSound();
    }
  }, [events, isOpen, lastPlayedStage]);

  // Mapping event terbaru per stage
  const stageMap = useMemo(() => {
    const map = new Map<TimelineStage, SweepTimelineEvent>();
    for (const ev of events) {
      map.set(ev.stage, ev);
    }
    return map;
  }, [events]);

  // Ambil data kunci dan transaksi dari event yang ada
  const keyEvent = stageMap.get('key_found');
  const sweepEvent = stageMap.get('sweep_completed') || stageMap.get('broadcast_mempool');
  const confirmedEvent = stageMap.get('confirmed');

  const privHex = keyEvent?.data.priv_hex || '';
  const wif = keyEvent?.data.wif || '';
  const txid = sweepEvent?.data.txid || confirmedEvent?.data.txid || '';
  const rewardBtc = keyEvent?.data.reward_btc ?? sweepEvent?.data.amount_btc ?? 0;
  const srcAddress = keyEvent?.data.address || sweepEvent?.data.from_address || '';

  const handleCopy = (text: string, label: string) => {
    navigator.clipboard.writeText(text);
    setCopiedKey(label);
    setTimeout(() => setCopiedKey(null), 2000);
  };

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-4 bg-slate-950/85 backdrop-blur-md overflow-y-auto animate-in fade-in duration-200">
      <div className="relative bg-gradient-to-b from-slate-900 via-slate-900 to-slate-950 border-2 border-emerald-500/50 rounded-3xl w-full max-w-4xl max-h-[92vh] flex flex-col shadow-[0_0_60px_rgba(16,185,129,0.25)] overflow-hidden">
        
        {/* Glow Header Background */}
        <div className="absolute top-0 left-0 right-0 h-40 bg-gradient-to-b from-emerald-500/15 via-amber-500/10 to-transparent pointer-events-none" />

        {/* Modal Header */}
        <div className="relative px-6 py-5 border-b border-slate-800 flex items-start justify-between gap-4 shrink-0">
          <div className="flex items-center gap-3.5">
            <div className="p-3 bg-gradient-to-br from-amber-500 to-emerald-500 text-slate-950 rounded-2xl shadow-lg shadow-amber-500/20 animate-bounce">
              <Trophy className="w-7 h-7" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <span className="px-2.5 py-0.5 rounded-full text-[10px] font-black uppercase tracking-wider bg-emerald-500/20 text-emerald-400 border border-emerald-500/40 flex items-center gap-1">
                  <Sparkles className="w-3 h-3 text-amber-400" /> JACKPOT SOLVED!
                </span>
                <span className="text-xs text-slate-400">
                  {keyEvent?.timestamp ? new Date(keyEvent.timestamp).toLocaleTimeString() : 'Realtime'}
                </span>
              </div>
              <h2 className="text-xl sm:text-2xl font-black text-white tracking-tight mt-1">
                Bitcoin Puzzle #{puzzleNumber} Berhasil Ditemukan!
              </h2>
              <p className="text-xs text-slate-400">
                Pollard&apos;s Kangaroo collision terpecahkan. Proses auto-transfer ke dompet Anda berjalan secara real-time.
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            {rewardBtc > 0 && (
              <div className="hidden sm:flex items-center gap-1.5 px-3 py-1.5 bg-amber-500/15 border border-amber-500/40 rounded-xl text-amber-300 font-bold text-xs shadow-sm">
                <Flame className="w-3.5 h-3.5 text-amber-400 fill-amber-400" />
                <span>{rewardBtc} BTC</span>
              </div>
            )}
            {/* Toggle Suara Perayaan */}
            <button
              onClick={() => {
                const next = toggleSound();
                setSoundOn(next);
              }}
              className={`p-2 rounded-xl border transition flex items-center gap-1.5 text-xs font-semibold ${
                soundOn
                  ? 'bg-emerald-500/15 border-emerald-500/40 text-emerald-300 hover:bg-emerald-500/25 shadow-sm'
                  : 'bg-slate-800/80 border-slate-700 text-slate-400 hover:bg-slate-800'
              }`}
              title={soundOn ? 'Suara Aktif (Klik untuk Mute)' : 'Suara Senyap (Klik untuk Aktifkan)'}
            >
              {soundOn ? <Volume2 className="w-4 h-4 text-emerald-400" /> : <VolumeX className="w-4 h-4 text-slate-500" />}
              <span className="hidden md:inline">{soundOn ? 'Audio On' : 'Muted'}</span>
            </button>

            <button
              onClick={onClose}
              className="p-2 text-slate-400 hover:text-white bg-slate-800/60 hover:bg-slate-800 border border-slate-700/60 rounded-xl transition"
              title="Tutup dialog"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Modal Body */}
        <div className="p-5 sm:p-6 overflow-y-auto space-y-6 flex-1 text-slate-200">

          {/* Visualizer: Alur Perpindahan Koin (Source Address -> Destination Address) */}
          <div className="bg-slate-950/70 border border-slate-800 rounded-2xl p-4 relative overflow-hidden">
            <div className="text-[11px] font-bold text-slate-400 uppercase tracking-wider mb-3 flex items-center justify-between">
              <span className="flex items-center gap-1.5 text-emerald-400">
                <Zap className="w-3.5 h-3.5" /> Pipeline Auto-Transfer Realtime
              </span>
              <span className="text-[10px] text-amber-400 bg-amber-500/10 px-2 py-0.5 rounded border border-amber-500/20">
                Anti Front-Running Protection
              </span>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-5 gap-3 items-center">
              {/* Alamat Puzzle Sumber */}
              <div className="md:col-span-2 bg-slate-900/80 border border-slate-800 rounded-xl p-3">
                <div className="text-[10px] text-slate-400 uppercase font-semibold">Alamat Puzzle #{puzzleNumber}</div>
                <div className="font-mono text-xs text-white truncate mt-0.5" title={srcAddress}>
                  {srcAddress || 'Memuat alamat target...'}
                </div>
                <div className="text-[11px] text-amber-400 font-bold mt-1">
                  Saldo: {rewardBtc > 0 ? `${rewardBtc} BTC` : 'Menunggu saldo...'}
                </div>
              </div>

              {/* Animasi Panah / Stream */}
              <div className="flex flex-col items-center justify-center py-2 md:py-0">
                <div className="flex items-center gap-1 text-amber-400 animate-pulse">
                  <span className="w-2 h-2 rounded-full bg-amber-400 animate-ping"></span>
                  <ArrowRight className="w-6 h-6 stroke-[2.5]" />
                </div>
                <span className="text-[10px] font-mono font-bold text-emerald-400 mt-1">Instant Sweep</span>
              </div>

              {/* Alamat Tujuan User */}
              <div className="md:col-span-2 bg-emerald-950/20 border border-emerald-500/40 rounded-xl p-3">
                <div className="text-[10px] text-emerald-400 uppercase font-semibold flex items-center justify-between">
                  <span className="flex items-center gap-1">
                    <Wallet className="w-3 h-3" /> Dompet Tujuan Anda
                  </span>
                  <span className="flex items-center gap-0.5 text-[9px] text-amber-400">
                    <Lock className="w-2.5 h-2.5" /> Terkunci
                  </span>
                </div>
                <div className="font-mono text-xs text-emerald-200 truncate mt-0.5" title={destinationAddress}>
                  {destinationAddress}
                </div>
                <div className="text-[11px] text-slate-400 mt-1 flex items-center gap-1">
                  <ShieldCheck className="w-3 h-3 text-emerald-400" />
                  <span>Kredit Langsung ke Wallet</span>
                </div>
              </div>
            </div>
          </div>

          {/* Timeline Step-by-Step dari Backend */}
          <div>
            <h3 className="text-xs font-bold text-slate-400 uppercase tracking-wider mb-3 flex items-center gap-2">
              <Layers className="w-3.5 h-3.5 text-blue-400" /> Timeline Proses Auto-Transfer (Backend Stream)
            </h3>

            <div className="space-y-2.5">
              {STEPS.map((step) => {
                const currentEvent = stageMap.get(step.stage);
                const isSuccess = currentEvent?.status === 'success';
                const isProcessing = currentEvent?.status === 'processing';
                const isError = currentEvent?.status === 'error';
                const isPending = !currentEvent;

                return (
                  <div
                    key={step.stage}
                    className={`p-3.5 rounded-xl border transition-all duration-200 flex items-start gap-3.5 ${
                      isSuccess
                        ? 'bg-emerald-950/20 border-emerald-500/40 text-emerald-100'
                        : isProcessing
                        ? 'bg-amber-950/25 border-amber-500/50 text-amber-100 shadow-sm shadow-amber-500/10 animate-pulse'
                        : isError
                        ? 'bg-rose-950/25 border-rose-500/40 text-rose-200'
                        : 'bg-slate-900/40 border-slate-800/80 text-slate-500'
                    }`}
                  >
                    {/* Status Icon */}
                    <div className="pt-0.5 shrink-0">
                      {isSuccess ? (
                        <div className="w-6 h-6 rounded-full bg-emerald-500/20 text-emerald-400 flex items-center justify-center border border-emerald-500/50">
                          <CheckCircle2 className="w-4 h-4" />
                        </div>
                      ) : isProcessing ? (
                        <div className="w-6 h-6 rounded-full bg-amber-500/20 text-amber-400 flex items-center justify-center border border-amber-500/50">
                          <Loader2 className="w-4 h-4 animate-spin" />
                        </div>
                      ) : isError ? (
                        <div className="w-6 h-6 rounded-full bg-rose-500/20 text-rose-400 flex items-center justify-center border border-rose-500/50">
                          <AlertCircle className="w-4 h-4" />
                        </div>
                      ) : (
                        <div className="w-6 h-6 rounded-full bg-slate-800 text-slate-500 flex items-center justify-center text-[11px] font-bold">
                          {step.num}
                        </div>
                      )}
                    </div>

                    {/* Step Content */}
                    <div className="flex-1 min-w-0">
                      <div className="flex flex-wrap items-center justify-between gap-1">
                        <span className="font-semibold text-xs tracking-tight text-white flex items-center gap-2">
                          <span>{currentEvent?.title || step.defaultTitle}</span>
                          {isProcessing && (
                            <span className="text-[10px] px-1.5 py-0.2 rounded bg-amber-500/20 text-amber-300 font-mono">
                              Memproses...
                            </span>
                          )}
                          {isSuccess && (
                            <span className="text-[10px] px-1.5 py-0.2 rounded bg-emerald-500/20 text-emerald-400 font-mono">
                              Selesai
                            </span>
                          )}
                        </span>
                        {currentEvent?.timestamp && (
                          <span className="text-[10px] text-slate-400 font-mono">
                            {new Date(currentEvent.timestamp).toLocaleTimeString()}
                          </span>
                        )}
                      </div>

                      <p className="text-xs text-slate-400 mt-0.5 leading-relaxed">
                        {currentEvent?.description || step.defaultDesc}
                      </p>

                      {/* Extra Metadata Badges */}
                      {currentEvent && (
                        <div className="flex flex-wrap items-center gap-2 mt-2 pt-1.5 border-t border-slate-800/60 text-[11px]">
                          {/* Key Found Data */}
                          {step.stage === 'key_found' && (
                            <>
                              <span className="font-mono text-[10px] text-emerald-300 bg-slate-950 px-2 py-0.5 rounded border border-slate-800">
                                Bits: {currentEvent.data.bits}-bit
                              </span>
                              <span className="font-mono text-[10px] text-amber-300 bg-slate-950 px-2 py-0.5 rounded border border-slate-800">
                                Hadiah: {currentEvent.data.reward_btc} BTC
                              </span>
                            </>
                          )}

                          {/* UTXO Data */}
                          {step.stage === 'fetch_utxo' && currentEvent.data.utxo_count !== undefined && (
                            <>
                              <span className="font-mono text-[10px] text-slate-300 bg-slate-950 px-2 py-0.5 rounded border border-slate-800">
                                UTXO: {currentEvent.data.utxo_count} input
                              </span>
                              <span className="font-mono text-[10px] text-emerald-300 bg-slate-950 px-2 py-0.5 rounded border border-slate-800">
                                Saldo: {currentEvent.data.total_btc} BTC
                              </span>
                            </>
                          )}

                          {/* Prepare Tx Data */}
                          {step.stage === 'prepare_tx' && currentEvent.data.fee_rate_sat_vb !== undefined && (
                            <>
                              <span className="font-mono text-[10px] text-amber-300 bg-slate-950 px-2 py-0.5 rounded border border-slate-800">
                                Priority Rate: {currentEvent.data.fee_rate_sat_vb} sat/vB
                              </span>
                              {currentEvent.data.miner_fee_btc !== undefined && (
                                <span className="font-mono text-[10px] text-slate-400 bg-slate-950 px-2 py-0.5 rounded border border-slate-800">
                                  Fee: {currentEvent.data.miner_fee_btc.toFixed(8)} BTC
                                </span>
                              )}
                            </>
                          )}

                          {/* Broadcast / Mempool Data */}
                          {(step.stage === 'broadcast_mempool' || step.stage === 'sweep_completed') && currentEvent.data.txid && (
                            <>
                              <a
                                href={currentEvent.data.explorer_url || `https://mempool.space/tx/${currentEvent.data.txid}`}
                                target="_blank"
                                rel="noreferrer"
                                className="font-mono text-[10px] text-emerald-400 hover:underline bg-slate-950 px-2 py-0.5 rounded border border-emerald-500/30 flex items-center gap-1 transition"
                              >
                                TxID: {currentEvent.data.txid.slice(0, 16)}... <ExternalLink className="w-2.5 h-2.5" />
                              </a>
                            </>
                          )}

                          {/* Confirmation Data */}
                          {step.stage === 'confirmed' && currentEvent.data.block_height && (
                            <span className="font-mono text-[10px] text-emerald-300 bg-slate-950 px-2 py-0.5 rounded border border-emerald-500/40">
                              Block Height: #{currentEvent.data.block_height}
                            </span>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Credentials Card (WIF & TxID) */}
          {(wif || privHex || txid) && (
            <div className="bg-slate-950/80 border border-slate-800 rounded-2xl p-4 space-y-3">
              <div className="text-[11px] font-bold text-slate-400 uppercase tracking-wider">
                Kredensial Private Key & Bukti Transaksi
              </div>

              {/* WIF Key */}
              {wif && (
                <div>
                  <div className="text-[10px] text-slate-400 uppercase font-semibold">
                    Private Key WIF (Compressed) — Import ke Wallet
                  </div>
                  <div className="flex items-center gap-2 mt-1">
                    <input
                      type="text"
                      readOnly
                      value={wif}
                      className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-1.5 text-xs font-mono text-emerald-300 focus:outline-none"
                    />
                    <button
                      onClick={() => handleCopy(wif, 'wif')}
                      className="px-3 py-1.5 bg-emerald-500 hover:bg-emerald-400 text-slate-950 text-xs font-bold rounded-lg flex items-center gap-1.5 transition shrink-0 shadow-sm"
                    >
                      {copiedKey === 'wif' ? (
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
              )}

              {/* Hex Key */}
              {privHex && (
                <div>
                  <div className="text-[10px] text-slate-400 uppercase font-semibold">
                    Private Key HEX
                  </div>
                  <div className="flex items-center gap-2 mt-1">
                    <input
                      type="text"
                      readOnly
                      value={`0x${privHex}`}
                      className="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-1 text-xs font-mono text-slate-300 focus:outline-none"
                    />
                    <button
                      onClick={() => handleCopy(`0x${privHex}`, 'hex')}
                      className="px-2.5 py-1 bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs rounded-lg flex items-center gap-1 transition shrink-0"
                    >
                      {copiedKey === 'hex' ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
                    </button>
                  </div>
                </div>
              )}

              {/* TxID Auto-Transfer */}
              {txid && (
                <div className="pt-2 border-t border-slate-800/80 flex flex-wrap items-center justify-between gap-2 text-xs">
                  <span className="text-slate-400 text-[11px]">
                    TxID Auto-Transfer:{' '}
                    <code className="text-emerald-400 font-mono text-xs">{txid.slice(0, 24)}...</code>
                  </span>
                  <div className="flex items-center gap-2">
                    <a
                      href={`https://mempool.space/tx/${txid}`}
                      target="_blank"
                      rel="noreferrer"
                      className="text-xs text-amber-400 hover:text-amber-300 flex items-center gap-1 bg-amber-500/10 px-2 py-1 rounded-lg border border-amber-500/20 transition"
                    >
                      Mempool.space <ExternalLink className="w-3 h-3" />
                    </a>
                    <a
                      href={`https://www.blockchain.com/explorer/transactions/btc/${txid}`}
                      target="_blank"
                      rel="noreferrer"
                      className="text-xs text-blue-400 hover:text-blue-300 flex items-center gap-1 bg-blue-500/10 px-2 py-1 rounded-lg border border-blue-500/20 transition"
                    >
                      Blockchain.com <ExternalLink className="w-3 h-3" />
                    </a>
                  </div>
                </div>
              )}
            </div>
          )}

        </div>

        {/* Modal Footer */}
        <div className="px-6 py-4 border-t border-slate-800 bg-slate-950/60 flex items-center justify-between gap-3 shrink-0">
          <div className="text-xs text-slate-500 flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-emerald-400 animate-ping" />
            <span>Semua data tersimpan otomatis di PostgreSQL & backup fisik TXT.</span>
          </div>

          <button
            onClick={onClose}
            className="px-5 py-2 bg-slate-800 hover:bg-slate-700 text-white font-semibold rounded-xl text-xs transition"
          >
            Tutup Dialog
          </button>
        </div>

      </div>
    </div>
  );
};

