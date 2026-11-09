import React, { useState } from 'react';
import { X, Send, Bot, CheckCircle2, AlertCircle, ExternalLink } from 'lucide-react';

interface TelegramModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const TelegramModal: React.FC<TelegramModalProps> = ({ isOpen, onClose }) => {
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);

  if (!isOpen) return null;

  const handleTestAlert = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const res = await fetch('/api/telegram/test', { method: 'POST' });
      const data = await res.json();
      if (res.ok) {
        setTestResult({ success: true, message: data.message || 'Pesan test terkirim!' });
      } else {
        setTestResult({ success: false, message: data.error || 'Gagal mengirim notifikasi' });
      }
    } catch (e: any) {
      setTestResult({ success: false, message: e.message || 'Koneksi error' });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4">
      <div className="bg-slate-900 border border-blue-500/30 rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl animate-in fade-in zoom-in-95 duration-150">
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-blue-950/20">
          <div className="flex items-center gap-2.5">
            <div className="p-2 bg-blue-500/20 text-blue-400 rounded-lg">
              <Bot className="w-5 h-5" />
            </div>
            <div>
              <h3 className="font-semibold text-lg text-white">Notifikasi Instan Telegram</h3>
              <p className="text-xs text-slate-400">Alert otomatis begitu kunci ditemukan & transfer sukses</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800 transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <div className="p-6 space-y-4 text-xs text-slate-300">
          <div className="p-3.5 bg-slate-950 border border-slate-800 rounded-xl space-y-2">
            <div className="font-semibold text-amber-400 flex items-center gap-1.5">
              <span>Cara Menghubungkan Bot Telegram:</span>
            </div>
            <ol className="list-decimal list-inside space-y-1.5 text-slate-400 text-[11px]">
              <li>
                Buka Telegram dan chat <a href="https://t.me/BotFather" target="_blank" rel="noreferrer" className="text-blue-400 hover:underline inline-flex items-center gap-0.5">@BotFather <ExternalLink className="w-3 h-3" /></a>, lalu ketik <code className="text-amber-300">/newbot</code> untuk membuat bot baru.
              </li>
              <li>
                Salin <b>Bot Token</b> yang diberikan oleh BotFather.
              </li>
              <li>
                Chat <a href="https://t.me/userinfobot" target="_blank" rel="noreferrer" className="text-blue-400 hover:underline inline-flex items-center gap-0.5">@userinfobot <ExternalLink className="w-3 h-3" /></a> untuk melihat <b>Chat ID</b> akun Telegram Anda.
              </li>
              <li>
                Buka file <code className="text-amber-300 font-mono">.env</code> di root project dan masukkan nilainya:
                <div className="mt-1 p-2 bg-slate-900 rounded font-mono text-[10px] text-slate-300">
                  TELEGRAM_BOT_TOKEN=123456789:ABCdefGHIjk...<br />
                  TELEGRAM_CHAT_ID=987654321
                </div>
              </li>
              <li>
                Pastikan Anda sudah menekan tombol <b>Start</b> pada bot Anda di Telegram!
              </li>
            </ol>
          </div>

          {testResult && (
            <div
              className={`p-3 rounded-xl border text-xs flex items-center gap-2 ${
                testResult.success
                  ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
                  : 'bg-rose-500/10 border-rose-500/30 text-rose-400'
              }`}
            >
              {testResult.success ? (
                <CheckCircle2 className="w-4 h-4 shrink-0" />
              ) : (
                <AlertCircle className="w-4 h-4 shrink-0" />
              )}
              <span>{testResult.message}</span>
            </div>
          )}

          <div className="p-3 bg-blue-950/30 border border-blue-500/20 rounded-xl space-y-1 text-slate-400 text-[11px]">
            <p className="font-semibold text-blue-300">Data yang Dikirim ke Telegram saat Solved:</p>
            <p>
              Nomor Puzzle, Tingkat Bits, Bitcoin Address, Private Key WIF, Private Key HEX/DEC, TxID Auto-Transfer, dan link langsung ke Blockchain.com Explorer!
            </p>
          </div>

          <div className="flex justify-end gap-3 pt-2">
            <button
              onClick={handleTestAlert}
              disabled={testing}
              className="px-4 py-2 bg-blue-500 hover:bg-blue-400 text-slate-950 font-bold rounded-xl text-xs flex items-center gap-2 transition disabled:opacity-50"
            >
              <Send className="w-3.5 h-3.5" />
              <span>{testing ? 'Mengirim Test...' : 'Kirim Pesan Uji Coba'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

