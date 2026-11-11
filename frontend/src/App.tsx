import React, { useState, useEffect } from 'react';
import {
  Flame,
  Plus,
  Key,
  Database,
  RefreshCw,
  Search,
  Zap,
  ShieldAlert,
  Server,
  Wallet,
  Globe,
  Bot,
  Lock,
  ShieldCheck,
  LayoutGrid,
  List,
  CheckCircle,
  ExternalLink,
  Copy,
  Check,
  ChevronRight,
  Layers,
  Sparkles,
  LogOut,
  Loader2,
} from 'lucide-react';
import { Puzzle, HunterState, FoundKey, SweepTimelineEvent, formatKeysSpeed, formatCompactKeys } from './types';
import { PuzzleCard } from './components/PuzzleCard';
import { AddPuzzleModal } from './components/AddPuzzleModal';
import { FoundKeysModal } from './components/FoundKeysModal';
import { CelebrationModal } from './components/CelebrationModal';
import { PuzzleDetailPage } from './components/PuzzleDetailPage';
import { TelegramModal } from './components/TelegramModal';
import { AnimatedBackground } from './components/AnimatedBackground';
import { LoginPage } from './components/LoginPage';
import { verifyCurrentToken, clearAuthToken, getAuthToken, getAuthUser, getApiBaseUrl } from './auth';

export type PubkeyFilter = 'all' | 'with_pubkey' | 'no_pubkey';
export type ViewMode = 'grid' | 'table';

export const App: React.FC = () => {
  const [puzzles, setPuzzles] = useState<Puzzle[]>([]);
  const [hunters, setHunters] = useState<Record<number, HunterState>>({});
  const [foundKeys, setFoundKeys] = useState<FoundKey[]>([]);
  const [selectedPuzzleNumber, setSelectedPuzzleNumber] = useState<number | null>(null);
  const [isAddModalOpen, setIsAddModalOpen] = useState(false);
  const [isFoundModalOpen, setIsFoundModalOpen] = useState(false);
  const [isTelegramModalOpen, setIsTelegramModalOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [dbConnected, setDbConnected] = useState(true);
  const [wsConnected, setWsConnected] = useState(false);
  const [lastRefreshed, setLastRefreshed] = useState<Date>(new Date());
  const [copiedAddressId, setCopiedAddressId] = useState<number | null>(null);

  // State Autentikasi Cloud
  const [isAuthenticated, setIsAuthenticated] = useState<boolean>(false);
  const [isAuthChecking, setIsAuthChecking] = useState<boolean>(true);

  const handleLogout = () => {
    if (confirm(`Yakin ingin keluar dari sesi admin (${getAuthUser()})?`)) {
      clearAuthToken();
      setIsAuthenticated(false);
    }
  };

  // State Celebration & Auto-Transfer Real-Time Timeline
  const [timelineEvents, setTimelineEvents] = useState<Record<number, SweepTimelineEvent[]>>({});
  const [activeCelebrationPuzzle, setActiveCelebrationPuzzle] = useState<number | null>(null);
  const [isCelebrationModalOpen, setIsCelebrationModalOpen] = useState(false);

  // Filter Tab Pubkey & Mode Tampilan
  const [pubkeyFilter, setPubkeyFilter] = useState<PubkeyFilter>('all');
  const [viewMode, setViewMode] = useState<ViewMode>(() => {
    return (localStorage.getItem('puzzle_view_mode') as ViewMode) || 'grid';
  });

  const handleCopyAddr = (e: React.MouseEvent, id: number, addr: string) => {
    e.stopPropagation();
    navigator.clipboard.writeText(addr);
    setCopiedAddressId(id);
    setTimeout(() => setCopiedAddressId(null), 2000);
  };

  const handleViewModeChange = (mode: ViewMode) => {
    setViewMode(mode);
    localStorage.setItem('puzzle_view_mode', mode);
  };

  // Alamat tujuan auto-transfer permanen (Terkunci demi keamanan, sinkron dari .env via /api/config)
  const [destinationAddress, setDestinationAddress] = useState(
    'bc1qz08nklntedluzzr9w0g3qhxrkvytayentqdshc'
  );

  const fetchConfig = async () => {
    try {
      const res = await fetch('/api/config');
      if (res.ok) {
        const data = await res.json();
        if (data.destination_address) {
          setDestinationAddress(data.destination_address);
        }
      }
    } catch {
      // Fallback tetap menggunakan default state
    }
  };

  // Ambil data puzzle dari PostgreSQL API
  const fetchPuzzles = async () => {
    try {
      const res = await fetch('/api/puzzles');
      if (res.ok) {
        const data = await res.json();
        setPuzzles(data.puzzles || []);
        setDbConnected(true);
      } else {
        setDbConnected(false);
      }
    } catch {
      setDbConnected(false);
    }
  };

  // Ambil status hunter real-time
  const fetchHuntersStatus = async () => {
    try {
      const res = await fetch('/api/hunt/status');
      if (res.ok) {
        const data = await res.json();
        const map: Record<number, HunterState> = {};
        for (const h of data.hunters || []) {
          map[h.puzzle_num] = h;
        }
        setHunters(map);
      }
    } catch (e) {
      console.error(e);
    }
  };

  // Ambil daftar kunci yang sudah ditemukan
  const fetchFoundKeys = async () => {
    try {
      const res = await fetch('/api/found');
      if (res.ok) {
        const data = await res.json();
        setFoundKeys(data.found_keys || []);
      }
    } catch (e) {
      console.error(e);
    }
  };

  const refreshAll = () => {
    fetchConfig();
    fetchPuzzles();
    fetchHuntersStatus();
    fetchFoundKeys();
    setLastRefreshed(new Date());
  };

  // Pemeriksaan Sesi Autentikasi Cloud saat Startup
  useEffect(() => {
    const checkAuth = async () => {
      const { valid } = await verifyCurrentToken();
      setIsAuthenticated(valid);
      setIsAuthChecking(false);
      if (valid) {
        refreshAll();
      }
    };

    checkAuth();

    // Listener otomatis saat token kedaluwarsa dari respons API (401)
    const handleExpired = () => {
      setIsAuthenticated(false);
    };
    window.addEventListener('puzzle_auth_expired', handleExpired);

    return () => {
      window.removeEventListener('puzzle_auth_expired', handleExpired);
    };
  }, []);

  // WebSocket Live Stream Connection (Real-time Progress)
  useEffect(() => {
    if (!isAuthenticated) return;

    let ws: WebSocket | null = null;
    let reconnectTimeout: any = null;

    const connectWs = () => {
      const baseUrl = getApiBaseUrl();
      let wsBase = '';
      if (baseUrl) {
        wsBase = baseUrl.replace(/^http:/i, 'ws:').replace(/^https:/i, 'wss:');
      } else {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        wsBase = `${protocol}//${window.location.host}`;
      }
      const token = getAuthToken();
      const wsUrl = `${wsBase}/api/ws?token=${token || ''}`;

      ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        setWsConnected(true);
      };

      ws.onmessage = (event) => {
        try {
          const raw = JSON.parse(event.data);

          // 1. Tangkap event broadcast timeline auto-transfer
          if (raw && raw.event_type === 'sweep_timeline') {
            const ev = raw as SweepTimelineEvent;
            setTimelineEvents((prev) => {
              const current = prev[ev.puzzle_num] || [];
              const filtered = current.filter((x) => x.stage !== ev.stage);
              return {
                ...prev,
                [ev.puzzle_num]: [...filtered, ev],
              };
            });
            setActiveCelebrationPuzzle(ev.puzzle_num);
            setIsCelebrationModalOpen(true);
            fetchFoundKeys();
            return;
          }

          // 2. Tangkap event update hunter state reguler
          const updatedHunter: HunterState = raw;
          if (updatedHunter && updatedHunter.puzzle_num) {
            setHunters((prev) => ({
              ...prev,
              [updatedHunter.puzzle_num]: updatedHunter,
            }));

            // Buka modal selebrasi jika status solved oleh kita
            if (updatedHunter.status === 'SolvedByUs') {
              setActiveCelebrationPuzzle(updatedHunter.puzzle_num);
              setIsCelebrationModalOpen(true);
              fetchFoundKeys();
            }
          }
        } catch (e) {
          console.error('WS error:', e);
        }
      };

      ws.onclose = () => {
        setWsConnected(false);
        reconnectTimeout = setTimeout(connectWs, 2000);
      };

      ws.onerror = () => {
        ws?.close();
      };
    };

    connectWs();

    return () => {
      if (reconnectTimeout) clearTimeout(reconnectTimeout);
      ws?.close();
    };
  }, [isAuthenticated]);

  // Fallback Polling setiap 5 detik
  useEffect(() => {
    if (!isAuthenticated) return;
    const timer = setInterval(() => {
      if (!wsConnected) {
        fetchHuntersStatus();
      }
    }, 5000);
    return () => clearInterval(timer);
  }, [wsConnected, isAuthenticated]);

  // Hitung jumlah puzzle dengan pubkey terbuka vs belum
  const countWithPubkey = puzzles.filter(
    (p) => p.pubkey_hex && p.pubkey_hex.trim().length === 66
  ).length;
  const countNoPubkey = puzzles.filter(
    (p) => !p.pubkey_hex || p.pubkey_hex.trim().length !== 66
  ).length;

  // Filter puzzle berdasarkan tab pubkey dan search query
  const filteredPuzzles = puzzles.filter((p) => {
    const hasPub = Boolean(p.pubkey_hex && p.pubkey_hex.trim().length === 66);
    if (pubkeyFilter === 'with_pubkey' && !hasPub) return false;
    if (pubkeyFilter === 'no_pubkey' && hasPub) return false;

    const q = searchQuery.toLowerCase().trim();
    if (!q) return true;
    return (
      p.puzzle_number.toString().includes(q) ||
      p.bits.toString().includes(q) ||
      p.address.toLowerCase().includes(q)
    );
  });

  const activeHuntersCount = Object.values(hunters).filter((h) => h.status === 'Running').length;
  const totalSpeed = Object.values(hunters).reduce(
    (acc, h) => (h.status === 'Running' ? acc + h.keys_per_second : acc),
    0
  );

  // Layar Tunggu Verifikasi Sesi Cloud
  if (isAuthChecking) {
    return (
      <div className="min-h-screen bg-[#070b12] text-slate-100 flex flex-col items-center justify-center p-4">
        <div className="relative mb-3">
          <img
            src="/logo.svg"
            alt="Puzzle Hunt"
            className="w-14 h-14 rounded-2xl animate-pulse shadow-lg shadow-amber-500/20 border border-amber-500/30"
          />
        </div>
        <div className="flex items-center gap-2 text-xs text-slate-400">
          <Loader2 className="w-4 h-4 animate-spin text-amber-400" />
          <span>Memeriksa Otorisasi Cloud...</span>
        </div>
      </div>
    );
  }

  // Layar Login jika Belum Terotentikasi
  if (!isAuthenticated) {
    return (
      <LoginPage
        onLoginSuccess={() => {
          setIsAuthenticated(true);
          refreshAll();
        }}
      />
    );
  }

  return (
    <div className="min-h-screen relative text-slate-100 flex flex-col selection:bg-amber-500/30">
      <AnimatedBackground />

      {/* Navbar Header */}
      <header className="border-b border-slate-800/80 bg-slate-900/40 backdrop-blur-md sticky top-0 z-30">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 py-4 flex flex-col sm:flex-row items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div
              className="relative group cursor-pointer shrink-0"
              onClick={() => setSelectedPuzzleNumber(null)}
              title="Puzzle Hunter Dashboard"
            >
              <img
                src="/logo.svg"
                alt="Puzzle Hunter Logo"
                className="w-11 h-11 rounded-2xl shadow-lg shadow-amber-500/20 group-hover:scale-105 transition-all duration-200"
              />
              <span className="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-emerald-400 border-2 border-slate-950 rounded-full animate-pulse" title="Engine Online"></span>
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h1 className="font-extrabold text-xl tracking-tight text-white">
                  PUZZLE HUNT <span className="text-amber-400 font-mono text-sm px-1.5 py-0.5 bg-amber-400/10 rounded">RUST+PG</span>
                </h1>
                <span className="flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-medium">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-ping"></span>
                  Online
                </span>
                <span
                  className={`flex items-center gap-1.5 text-[11px] px-2.5 py-0.5 rounded-full font-medium border transition ${
                    wsConnected
                      ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
                      : 'bg-amber-500/10 text-amber-400 border-amber-500/20'
                  }`}
                  title={wsConnected ? 'WebSocket Live Stream Aktif' : 'Menghubungkan ke WebSocket...'}
                >
                  <span
                    className={`w-1.5 h-1.5 rounded-full ${
                      wsConnected ? 'bg-emerald-400 animate-ping' : 'bg-amber-400'
                    }`}
                  ></span>
                  <span>{wsConnected ? 'WSS Live' : 'WSS Reconnecting'}</span>
                </span>
              </div>
              <p className="text-xs text-slate-400">
                Pollard's Kangaroo O(√N) Controller & PostgreSQL Database
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2.5 w-full sm:w-auto justify-end">
            <button
              onClick={() => setIsTelegramModalOpen(true)}
              className="px-3.5 py-2 bg-blue-500/10 hover:bg-blue-500/20 text-blue-400 border border-blue-500/30 rounded-xl text-xs font-semibold flex items-center gap-2 transition"
              title="Konfigurasi & Test Notifikasi Telegram"
            >
              <Bot className="w-4 h-4" />
              <span>Telegram Alert</span>
            </button>

            <button
              onClick={() => setIsFoundModalOpen(true)}
              className="px-3.5 py-2 bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 rounded-xl text-xs font-semibold flex items-center gap-2 transition"
            >
              <Key className="w-4 h-4" />
              <span>Found Keys</span>
              {foundKeys.length > 0 && (
                <span className="px-1.5 py-0.2 bg-emerald-500 text-slate-950 font-bold rounded-full text-[10px]">
                  {foundKeys.length}
                </span>
              )}
            </button>

            <button
              onClick={() => setIsAddModalOpen(true)}
              className="px-4 py-2 bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold rounded-xl text-xs flex items-center gap-1.5 shadow-md shadow-amber-500/10 transition"
            >
              <Plus className="w-4 h-4" />
              <span>Tambah Puzzle</span>
            </button>

            {/* Tombol Logout Admin */}
            <button
              onClick={handleLogout}
              className="px-3 py-2 bg-slate-800/80 hover:bg-rose-500/20 text-slate-400 hover:text-rose-400 border border-slate-700/60 hover:border-rose-500/30 rounded-xl text-xs font-semibold flex items-center gap-1.5 transition"
              title={`Sesi aktif sebagai: ${getAuthUser()} — Klik untuk keluar`}
            >
              <LogOut className="w-4 h-4" />
              <span className="hidden sm:inline">Keluar</span>
            </button>
          </div>
        </div>
      </header>

      {/* Main Container */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 py-6 flex-1 w-full space-y-6">
        {/* Header Dashboard Utama (Banner Auto-Transfer & Metric Bar) — Hanya muncul di halaman utama */}
        {selectedPuzzleNumber === null && (
          <>
            {/* Banner Alamat Tujuan Auto-Transfer & Node RPC */}
            <div className="bg-slate-900/60 border border-slate-800 rounded-2xl p-4 flex flex-col md:flex-row items-center justify-between gap-3">
              <div className="flex items-center gap-3 w-full md:w-auto">
                <div className="p-2 bg-emerald-500/10 text-emerald-400 rounded-xl shrink-0">
                  <Wallet className="w-5 h-5" />
                </div>
                <div className="w-full">
                  <div className="text-[11px] text-slate-400 uppercase font-semibold tracking-wider flex items-center gap-2">
                    <span>Alamat Tujuan Auto-Transfer</span>
                    <span className="text-emerald-400 text-[10px] font-mono font-normal bg-emerald-500/10 px-1.5 py-0.5 rounded">
                      Instant Sweep
                    </span>
                  </div>
                  <div className="flex flex-wrap items-center gap-2 mt-1">
                    <span
                      className="font-mono text-xs text-emerald-400 font-bold bg-slate-950 px-2.5 py-1 rounded-lg border border-slate-800 cursor-default"
                      title={`Alamat tujuan: ${destinationAddress}`}
                    >
                      {`${destinationAddress.slice(0, 10)}...${destinationAddress.slice(-8)}`}
                    </span>
                    <span className="flex items-center gap-1 text-[11px] text-slate-400 bg-slate-950/80 px-2 py-0.5 rounded-lg border border-slate-800">
                      <Lock className="w-3 h-3 text-amber-400" />
                      <span>Terkunci Permanen</span>
                    </span>
                  </div>
                </div>
              </div>

              <div className="flex items-center gap-2 self-start md:self-auto text-xs text-slate-400 bg-slate-950/80 px-3 py-1.5 rounded-xl border border-slate-800">
                <Globe className="w-3.5 h-3.5 text-blue-400" />
                <span>RPC Node:</span>
                <code className="text-amber-400 font-mono text-[11px]">bitcoin-rpc.publicnode.com</code>
              </div>
            </div>

            {/* Metric Bar */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
              <div className="p-4 bg-slate-900/50 border border-slate-800/80 rounded-2xl">
                <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
                  <Server className="w-4 h-4 text-amber-400" />
                  <span>Database PostgreSQL</span>
                </div>
                <div className="text-lg font-bold text-white flex items-center gap-2">
                  <span className={`w-2.5 h-2.5 rounded-full ${dbConnected ? 'bg-emerald-400' : 'bg-rose-500'}`}></span>
                  <span>{dbConnected ? 'Connected' : 'Disconnected'}</span>
                </div>
              </div>

              <div className="p-4 bg-slate-900/50 border border-slate-800/80 rounded-2xl">
                <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
                  <Flame className="w-4 h-4 text-orange-400" />
                  <span>Active Hunters</span>
                </div>
                <div className="text-lg font-bold text-white font-mono">
                  {activeHuntersCount} <span className="text-xs text-slate-500 font-normal">berjalan</span>
                </div>
              </div>

              <div className="p-4 bg-slate-900/50 border border-slate-800/80 rounded-2xl">
                <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
                  <Zap className="w-4 h-4 text-amber-400" />
                  <span>Total Speed</span>
                </div>
                <div className="text-lg font-bold text-white font-mono">
                  {totalSpeed > 0 ? formatKeysSpeed(totalSpeed) : '0 k/s'}
                </div>
              </div>

              <div className="p-4 bg-slate-900/50 border border-slate-800/80 rounded-2xl">
                <div className="flex items-center gap-2 text-slate-400 text-xs mb-1">
                  <Database className="w-4 h-4 text-blue-400" />
                  <span>Total Puzzles Terdaftar</span>
                </div>
                <div className="text-lg font-bold text-white font-mono">
                  {puzzles.length} <span className="text-xs text-slate-500 font-normal">puzzle</span>
                </div>
              </div>
            </div>
          </>
        )}

        {/* Tampilan Detail Monitor vs Grid/Table Dashboard */}
        {selectedPuzzleNumber !== null && puzzles.find((p) => p.puzzle_number === selectedPuzzleNumber) ? (
          <PuzzleDetailPage
            puzzle={puzzles.find((p) => p.puzzle_number === selectedPuzzleNumber)!}
            hunter={hunters[selectedPuzzleNumber]}
            destinationAddress={destinationAddress}
            onBack={() => setSelectedPuzzleNumber(null)}
            onRefresh={refreshAll}
          />
        ) : (
          <>
            {/* Tab Pemisah: Pubkey Ada vs Belum & Toolbar Mode Tampilan */}
            <div className="space-y-3 pt-2">
              {/* Tab Filter Kategori Puzzle */}
              <div className="flex flex-wrap items-center justify-between gap-3 border-b border-slate-800 pb-3">
                <div className="flex items-center gap-2 flex-wrap">
                  <button
                    onClick={() => setPubkeyFilter('all')}
                    className={`px-3.5 py-1.5 rounded-xl text-xs font-bold transition flex items-center gap-2 ${
                      pubkeyFilter === 'all'
                        ? 'bg-amber-500 text-slate-950 shadow-md shadow-amber-500/20'
                        : 'bg-slate-900/80 hover:bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-800'
                    }`}
                  >
                    <Layers className="w-3.5 h-3.5" />
                    <span>Semua Puzzle</span>
                    <span
                      className={`text-[10px] px-1.5 py-0.2 rounded-md ${
                        pubkeyFilter === 'all'
                          ? 'bg-slate-950/20 text-slate-950 font-mono'
                          : 'bg-slate-800 text-slate-400 font-mono'
                      }`}
                    >
                      {puzzles.length}
                    </span>
                  </button>

                  <button
                    onClick={() => setPubkeyFilter('with_pubkey')}
                    className={`px-3.5 py-1.5 rounded-xl text-xs font-bold transition flex items-center gap-2 ${
                      pubkeyFilter === 'with_pubkey'
                        ? 'bg-purple-500 text-white shadow-md shadow-purple-500/20'
                        : 'bg-slate-900/80 hover:bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-800'
                    }`}
                  >
                    <Zap className="w-3.5 h-3.5 text-purple-300" />
                    <span>Pubkey Ada (Kangaroo O(√N))</span>
                    <span
                      className={`text-[10px] px-1.5 py-0.2 rounded-md font-mono ${
                        pubkeyFilter === 'with_pubkey'
                          ? 'bg-purple-700/60 text-white'
                          : 'bg-purple-500/10 text-purple-400 border border-purple-500/20'
                      }`}
                    >
                      {countWithPubkey}
                    </span>
                  </button>

                  <button
                    onClick={() => setPubkeyFilter('no_pubkey')}
                    className={`px-3.5 py-1.5 rounded-xl text-xs font-bold transition flex items-center gap-2 ${
                      pubkeyFilter === 'no_pubkey'
                        ? 'bg-blue-500 text-white shadow-md shadow-blue-500/20'
                        : 'bg-slate-900/80 hover:bg-slate-800 text-slate-400 hover:text-slate-200 border border-slate-800'
                    }`}
                  >
                    <Search className="w-3.5 h-3.5 text-blue-300" />
                    <span>Pubkey Belum Ada (Address Mode)</span>
                    <span
                      className={`text-[10px] px-1.5 py-0.2 rounded-md font-mono ${
                        pubkeyFilter === 'no_pubkey'
                          ? 'bg-blue-700/60 text-white'
                          : 'bg-blue-500/10 text-blue-400 border border-blue-500/20'
                      }`}
                    >
                      {countNoPubkey}
                    </span>
                  </button>
                </div>

                {/* View Mode Switcher: Grid vs Table */}
                <div className="flex items-center gap-1 bg-slate-900/90 border border-slate-800 p-1 rounded-xl">
                  <button
                    onClick={() => handleViewModeChange('grid')}
                    className={`px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5 transition ${
                      viewMode === 'grid'
                        ? 'bg-amber-500 text-slate-950 font-bold shadow'
                        : 'text-slate-400 hover:text-white'
                    }`}
                    title="Tampilan Grid Kartu"
                  >
                    <LayoutGrid className="w-3.5 h-3.5" />
                    <span className="hidden sm:inline">Grid</span>
                  </button>
                  <button
                    onClick={() => handleViewModeChange('table')}
                    className={`px-3 py-1.5 rounded-lg text-xs font-semibold flex items-center gap-1.5 transition ${
                      viewMode === 'table'
                        ? 'bg-amber-500 text-slate-950 font-bold shadow'
                        : 'text-slate-400 hover:text-white'
                    }`}
                    title="Tampilan Tabel List"
                  >
                    <List className="w-3.5 h-3.5" />
                    <span className="hidden sm:inline">Tabel List</span>
                  </button>
                </div>
              </div>

              {/* Baris Pencarian & Refresh */}
              <div className="flex flex-col sm:flex-row items-center justify-between gap-3">
                <div className="relative w-full sm:w-96">
                  <Search className="w-4 h-4 absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-500" />
                  <input
                    type="text"
                    placeholder="Cari nomor puzzle, bits, atau address..."
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    className="w-full bg-slate-900/60 border border-slate-800 rounded-xl pl-9 pr-3 py-2 text-xs text-white focus:outline-none focus:border-amber-500 transition"
                  />
                </div>

                <div className="flex items-center gap-3 w-full sm:w-auto justify-between sm:justify-end text-xs text-slate-400">
                  <span className="text-xs text-slate-400">
                    Menampilkan <strong className="text-amber-400">{filteredPuzzles.length}</strong> dari {puzzles.length} puzzle
                  </span>
                  <div className="flex items-center gap-2">
                    <button
                      onClick={refreshAll}
                      className="p-2 bg-slate-900 border border-slate-800 hover:border-slate-700 text-slate-300 rounded-xl flex items-center gap-1.5 transition text-xs"
                      title="Refresh Data"
                    >
                      <RefreshCw className="w-3.5 h-3.5" />
                      <span>Refresh</span>
                    </button>
                    <span className="text-[11px] text-slate-500 hidden md:inline">
                      {lastRefreshed.toLocaleTimeString()}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* Konten Puzzles (Grid vs Table List) */}
            {filteredPuzzles.length === 0 ? (
              <div className="text-center py-20 bg-slate-900/20 border border-slate-800/50 rounded-2xl">
                <ShieldAlert className="w-12 h-12 text-slate-600 mx-auto mb-3" />
                <h3 className="text-base font-semibold text-slate-300">Tidak ada puzzle yang cocok</h3>
                <p className="text-xs text-slate-500 mt-1">
                  Coba ubah kata kunci pencarian atau ganti tab filter kategori puzzle di atas.
                </p>
              </div>
            ) : viewMode === 'grid' ? (
              /* ===== MODE TAMPILAN GRID ===== */
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-5">
                {filteredPuzzles.map((p) => (
                  <PuzzleCard
                    key={p.puzzle_number}
                    puzzle={p}
                    hunter={hunters[p.puzzle_number]}
                    destinationAddress={destinationAddress}
                    onSelect={() => setSelectedPuzzleNumber(p.puzzle_number)}
                    onRefresh={refreshAll}
                  />
                ))}
              </div>
            ) : (
              /* ===== MODE TAMPILAN TABEL LIST ===== */
              <div className="bg-slate-900/50 border border-slate-800 rounded-2xl overflow-hidden shadow-xl">
                <div className="overflow-x-auto">
                  <table className="w-full text-left text-xs font-mono">
                    <thead className="bg-slate-950/90 text-slate-400 border-b border-slate-800 text-[11px] uppercase tracking-wider font-sans">
                      <tr>
                        <th className="py-3 px-4"># Puzzle</th>
                        <th className="py-3 px-3">Status</th>
                        <th className="py-3 px-3">Metode & Algoritma</th>
                        <th className="py-3 px-3">Bitcoin Address</th>
                        <th className="py-3 px-3">Hadiah (BTC)</th>
                        <th className="py-3 px-3">Range Target (HEX)</th>
                        <th className="py-3 px-4 text-right">Aksi</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-800/60 text-slate-300">
                      {filteredPuzzles.map((p) => {
                        const h = hunters[p.puzzle_number];
                        const isRunning = h?.status === 'Running';
                        const isSolved = p.is_solved || h?.status === 'SolvedByUs' || h?.status === 'SolvedByOther';
                        const hasPubkey = Boolean(p.pubkey_hex && p.pubkey_hex.trim().length === 66);

                        return (
                          <tr
                            key={p.puzzle_number}
                            onClick={() => setSelectedPuzzleNumber(p.puzzle_number)}
                            className={`hover:bg-slate-800/50 transition-colors cursor-pointer group ${
                              isRunning ? 'bg-amber-500/5' : ''
                            }`}
                          >
                            {/* # Puzzle & Bits */}
                            <td className="py-3 px-4">
                              <div className="flex items-center gap-2">
                                <span className="font-sans font-black text-sm text-white group-hover:text-amber-400 transition-colors">
                                  #{p.puzzle_number}
                                </span>
                                <span className="px-2 py-0.5 rounded-md text-[10px] font-bold bg-slate-800 text-slate-300 border border-slate-700">
                                  {p.bits}-bit
                                </span>
                              </div>
                            </td>

                            {/* Status */}
                            <td className="py-3 px-3 font-sans">
                              {isSolved ? (
                                <span className="px-2 py-0.5 rounded-full text-[10px] font-bold bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 inline-flex items-center gap-1">
                                  <CheckCircle className="w-3 h-3" /> SOLVED
                                </span>
                              ) : isRunning ? (
                                <div className="space-y-0.5">
                                  <span className="px-2 py-0.5 rounded-full text-[10px] font-bold bg-amber-500/20 text-amber-400 border border-amber-500/30 inline-flex items-center gap-1 animate-pulse">
                                    <span className="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
                                    RUNNING
                                  </span>
                                  {h?.keys_per_second ? (
                                    <div className="text-[10px] text-amber-400 font-mono font-semibold">
                                      {formatKeysSpeed(h.keys_per_second)}
                                    </div>
                                  ) : null}
                                </div>
                              ) : (
                                <span className="px-2 py-0.5 rounded-full text-[10px] font-medium bg-slate-800 text-slate-400">
                                  IDLE
                                </span>
                              )}
                            </td>

                            {/* Metode & Public Key */}
                            <td className="py-3 px-3 font-sans">
                              {hasPubkey ? (
                                <div className="space-y-0.5">
                                  <span className="px-2 py-0.5 rounded-md text-[10px] font-bold bg-purple-500/15 text-purple-300 border border-purple-500/30 inline-flex items-center gap-1">
                                    <Zap className="w-3 h-3 text-purple-400" />
                                    <span>Kangaroo O(√N)</span>
                                  </span>
                                  <div className="text-[10px] font-mono text-slate-400 truncate max-w-[140px]" title={p.pubkey_hex}>
                                    {p.pubkey_hex.slice(0, 10)}...{p.pubkey_hex.slice(-6)}
                                  </div>
                                </div>
                              ) : (
                                <div className="space-y-0.5">
                                  <span className="px-2 py-0.5 rounded-md text-[10px] font-medium bg-blue-500/10 text-blue-400 border border-blue-500/20 inline-flex items-center gap-1">
                                    <Search className="w-3 h-3" />
                                    <span>Address Hash160</span>
                                  </span>
                                  <div className="text-[10px] text-slate-500 italic">
                                    Pubkey belum terekspos
                                  </div>
                                </div>
                              )}
                            </td>

                            {/* Target Address */}
                            <td className="py-3 px-3">
                              <div className="flex items-center gap-1.5">
                                <span className="text-slate-200 text-xs font-mono">{p.address}</span>
                                <button
                                  onClick={(e) => handleCopyAddr(e, p.id, p.address)}
                                  className="text-slate-500 hover:text-white p-1 rounded transition"
                                  title="Salin Alamat"
                                >
                                  {copiedAddressId === p.id ? (
                                    <Check className="w-3.5 h-3.5 text-emerald-400" />
                                  ) : (
                                    <Copy className="w-3.5 h-3.5" />
                                  )}
                                </button>
                                <a
                                  href={`https://www.blockchain.com/explorer/addresses/btc/${p.address}`}
                                  target="_blank"
                                  rel="noreferrer"
                                  onClick={(e) => e.stopPropagation()}
                                  className="text-slate-500 hover:text-amber-400 p-1 transition"
                                  title="Buka di Blockchain Explorer"
                                >
                                  <ExternalLink className="w-3.5 h-3.5" />
                                </a>
                              </div>
                            </td>

                            {/* Hadiah (BTC) */}
                            <td className="py-3 px-3 font-sans">
                              <span className="font-extrabold text-amber-400 font-mono text-xs">
                                {p.reward_btc > 0 ? `${p.reward_btc.toFixed(2)} BTC` : 'TBA'}
                              </span>
                            </td>

                            {/* Range Target (HEX) */}
                            <td className="py-3 px-3 text-[11px] text-slate-400 font-mono">
                              <div className="truncate max-w-[160px]" title={`${p.range_start_hex} .. ${p.range_end_hex}`}>
                                {p.range_start_hex.slice(0, 8)}...{p.range_end_hex.slice(-6)}
                              </div>
                            </td>

                            {/* Aksi */}
                            <td className="py-3 px-4 text-right">
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  setSelectedPuzzleNumber(p.puzzle_number);
                                }}
                                className="px-3 py-1.5 bg-amber-500/10 hover:bg-amber-500 text-amber-400 hover:text-slate-950 font-sans font-bold text-xs rounded-lg border border-amber-500/30 transition inline-flex items-center gap-1 shadow-sm"
                              >
                                <span>Detail</span>
                                <ChevronRight className="w-3.5 h-3.5" />
                              </button>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              </div>
            )}
          </>
        )}
      </main>

      {/* Footer */}
      <footer className="border-t border-slate-900 bg-slate-950/60 py-4 text-center text-xs text-slate-500 flex items-center justify-center gap-2">
        <img src="/logo.svg" alt="Puzzle Hunt" className="w-4 h-4 opacity-60" />
        <span>Bitcoin Puzzle Hunter Engine &copy; 2026 — Rust Axum + PostgreSQL + Tokio + CUDA</span>
      </footer>

      {/* Modals */}
      <AddPuzzleModal
        isOpen={isAddModalOpen}
        onClose={() => setIsAddModalOpen(false)}
        onAdded={refreshAll}
      />
      <FoundKeysModal
        isOpen={isFoundModalOpen}
        onClose={() => setIsFoundModalOpen(false)}
        keys={foundKeys}
      />
      <TelegramModal
        isOpen={isTelegramModalOpen}
        onClose={() => setIsTelegramModalOpen(false)}
      />
      {activeCelebrationPuzzle !== null && (
        <CelebrationModal
          isOpen={isCelebrationModalOpen}
          onClose={() => setIsCelebrationModalOpen(false)}
          puzzleNumber={activeCelebrationPuzzle}
          events={timelineEvents[activeCelebrationPuzzle] || []}
          destinationAddress={destinationAddress}
        />
      )}
    </div>
  );
};

