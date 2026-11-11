import React, { useState } from 'react';
import { Lock, User, ShieldCheck, AlertCircle, Loader2, Eye, EyeOff, KeyRound, Server, ChevronDown, ChevronUp } from 'lucide-react';
import { setAuthToken, setAuthUser, getApiBaseUrl, setApiBaseUrl } from '../auth';

interface LoginPageProps {
  onLoginSuccess: () => void;
}

export const LoginPage: React.FC<LoginPageProps> = ({ onLoginSuccess }) => {
  const [username, setUsername] = useState('admin');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showServerConfig, setShowServerConfig] = useState(false);
  const [serverUrl, setServerUrl] = useState(getApiBaseUrl() || '');

  const handleServerSave = (val: string) => {
    setServerUrl(val);
    setApiBaseUrl(val);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      const res = await fetch('/api/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          username: username.trim(),
          password,
        }),
      });

      const data = await res.json();

      if (res.ok && data.success && data.token) {
        setAuthToken(data.token);
        setAuthUser(data.username || username.trim());
        onLoginSuccess();
      } else {
        setError(data.error || 'Username atau password salah. Silakan coba lagi.');
      }
    } catch (err) {
      console.error(err);
      setError('Gagal menghubungi backend API. Pastikan server Rust aktif di port 3001.');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-[#070b12] text-slate-100 flex items-center justify-center p-4 selection:bg-amber-500/30 selection:text-amber-200">
      {/* Container Card */}
      <div className="w-full max-w-md bg-slate-900/80 border border-slate-800/90 rounded-3xl p-6 sm:p-8 shadow-2xl shadow-black/80 backdrop-blur-xl relative overflow-hidden">
        {/* Subtle Ambient Glow */}
        <div className="absolute -top-24 -left-24 w-48 h-48 bg-amber-500/10 rounded-full blur-3xl pointer-events-none" />
        <div className="absolute -bottom-24 -right-24 w-48 h-48 bg-purple-500/10 rounded-full blur-3xl pointer-events-none" />

        {/* Brand Header */}
        <div className="flex flex-col items-center text-center mb-6">
          <div className="relative mb-3 group">
            <img
              src="/logo.svg"
              alt="Puzzle Hunt Logo"
              className="w-16 h-16 rounded-2xl shadow-xl shadow-amber-500/20 border border-amber-500/30 transition-transform group-hover:scale-105 duration-200"
            />
            <span className="absolute -bottom-1 -right-1 w-4 h-4 bg-emerald-400 border-2 border-slate-950 rounded-full animate-pulse" title="Security Active" />
          </div>

          <div className="flex items-center gap-2 mb-1">
            <h1 className="text-xl font-extrabold tracking-tight text-white">
              PUZZLE HUNT
            </h1>
            <span className="text-amber-400 font-mono text-[11px] px-2 py-0.5 bg-amber-400/10 border border-amber-400/20 rounded-md font-bold">
              RUST+PG+CUDA
            </span>
          </div>

          <div className="text-xs font-semibold text-slate-400 uppercase tracking-wider flex items-center gap-1.5 mt-1">
            <Lock className="w-3.5 h-3.5 text-amber-400" />
            <span>Cloud Access Control</span>
          </div>
          <p className="text-xs text-slate-500 mt-1 max-w-xs">
            Sistem terproteksi. Masukkan kredensial admin untuk mengakses dashboard dan controller puzzle.
          </p>
        </div>

        {/* Error Alert */}
        {error && (
          <div className="mb-5 p-3 bg-rose-500/10 border border-rose-500/30 rounded-xl flex items-start gap-2.5 text-xs text-rose-300 animate-shake">
            <AlertCircle className="w-4 h-4 text-rose-400 shrink-0 mt-0.5" />
            <div className="flex-1 font-medium">{error}</div>
          </div>
        )}

        {/* Login Form */}
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-semibold text-slate-300 mb-1.5 flex items-center justify-between">
              <span>Username Admin</span>
              <span className="text-[10px] text-slate-500 font-normal">Sesuai .env</span>
            </label>
            <div className="relative">
              <div className="absolute inset-y-0 left-0 pl-3.5 flex items-center pointer-events-none text-slate-500">
                <User className="w-4 h-4" />
              </div>
              <input
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                required
                placeholder="admin"
                className="w-full bg-slate-950 border border-slate-800 rounded-xl pl-10 pr-3.5 py-2.5 text-sm text-slate-200 placeholder-slate-600 focus:outline-none focus:border-amber-500 focus:ring-1 focus:ring-amber-500/50 transition font-mono"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-semibold text-slate-300 mb-1.5 flex items-center justify-between">
              <span>Password Admin</span>
              <span className="text-[10px] text-slate-500 font-normal">ADMIN_PASSWORD</span>
            </label>
            <div className="relative">
              <div className="absolute inset-y-0 left-0 pl-3.5 flex items-center pointer-events-none text-slate-500">
                <KeyRound className="w-4 h-4" />
              </div>
              <input
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
                placeholder="••••••••••••"
                className="w-full bg-slate-950 border border-slate-800 rounded-xl pl-10 pr-10 py-2.5 text-sm text-slate-200 placeholder-slate-600 focus:outline-none focus:border-amber-500 focus:ring-1 focus:ring-amber-500/50 transition"
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute inset-y-0 right-0 pr-3.5 flex items-center text-slate-500 hover:text-slate-300 transition"
                tabIndex={-1}
              >
                {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
              </button>
            </div>
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full mt-2 py-3 bg-gradient-to-r from-amber-500 to-amber-400 hover:from-amber-400 hover:to-amber-300 text-slate-950 font-extrabold text-sm rounded-xl shadow-lg shadow-amber-500/20 hover:shadow-amber-500/30 transition flex items-center justify-center gap-2 disabled:opacity-50"
          >
            {loading ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                <span>Memverifikasi Sesi...</span>
              </>
            ) : (
              <>
                <ShieldCheck className="w-4 h-4" />
                <span>Masuk ke Dashboard</span>
              </>
            )}
          </button>
        </form>

        {/* Backend Host Config (Untuk Vercel / Cloud Remote Hosting) */}
        <div className="mt-4 pt-3 border-t border-slate-800/60">
          <button
            type="button"
            onClick={() => setShowServerConfig(!showServerConfig)}
            className="w-full flex items-center justify-between text-xs text-slate-400 hover:text-amber-400 transition"
          >
            <span className="flex items-center gap-1.5">
              <Server className="w-3.5 h-3.5 text-amber-500" />
              <span>Target Backend API URL</span>
            </span>
            {showServerConfig ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
          </button>

          {showServerConfig && (
            <div className="mt-2 p-3 bg-slate-950/90 rounded-xl border border-slate-800 text-xs space-y-2">
              <label className="text-slate-400 block text-[11px]">
                Alamat IP / Domain GPU Cloud Backend:
              </label>
              <input
                type="text"
                value={serverUrl}
                onChange={(e) => handleServerSave(e.target.value)}
                placeholder="misal: http://123.45.67.89:3001"
                className="w-full bg-slate-900 border border-slate-700/80 rounded-lg px-2.5 py-1.5 text-xs text-amber-300 placeholder-slate-600 focus:outline-none focus:border-amber-500 font-mono"
              />
              <p className="text-[10px] text-slate-500">
                Kosongkan jika menggunakan proxy lokal (port 5173). Isi jika frontend di-hosting di Vercel.
              </p>
            </div>
          )}
        </div>

        {/* Security Note Footer */}
        <div className="mt-6 pt-4 border-t border-slate-800/80 text-center space-y-1 text-[11px] text-slate-500">
          <p className="flex items-center justify-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
            <span>HMAC-SHA256 Cryptographic Bearer Token</span>
          </p>
          <p className="text-[10px] text-slate-600">
            Kredensial dapat diubah melalui file <code className="text-amber-400/80 font-mono">.env</code> di server.
          </p>
        </div>
      </div>
    </div>
  );
};

