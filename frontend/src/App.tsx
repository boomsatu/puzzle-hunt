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
} from 'lucide-react';
import { Puzzle, HunterState, FoundKey, formatKeysSpeed } from './types';
import { PuzzleCard } from './components/PuzzleCard';
import { AddPuzzleModal } from './components/AddPuzzleModal';
import { FoundKeysModal } from './components/FoundKeysModal';
import { PuzzleDetailPage } from './components/PuzzleDetailPage';
import { TelegramModal } from './components/TelegramModal';

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

  // Initial load
  useEffect(() => {
    refreshAll();
  }, []);

  // WebSocket Live Stream Connection (Real-time Progress)
  useEffect(() => {
    let ws: WebSocket | null = null;
    let reconnectTimeout: any = null;

    const connectWs = () => {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      const wsUrl = `${protocol}//${window.location.host}/api/ws`;

      ws = new WebSocket(wsUrl);

      ws.onopen = () => {
        setWsConnected(true);
      };

      ws.onmessage = (event) => {
        try {
          const updatedHunter: HunterState = JSON.parse(event.data);
          if (updatedHunter && updatedHunter.puzzle_num) {
            setHunters((prev) => ({
              ...prev,
              [updatedHunter.puzzle_num]: updatedHunter,
            }));
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
  }, []);

  // Fallback Polling setiap 5 detik
  useEffect(() => {
    const timer = setInterval(() => {
      if (!wsConnected) {
        fetchHuntersStatus();
      }
    }, 5000);
    return () => clearInterval(timer);
  }, [wsConnected]);

  // Filter puzzle berdasarkan nomor atau bits
  const filteredPuzzles = puzzles.filter((p) => {
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

  return (
    <div className="min-h-screen bg-[#070b12] text-slate-100 flex flex-col">
      {/* Navbar Header */}
      <header className="border-b border-slate-800/80 bg-slate-900/40 backdrop-blur-md sticky top-0 z-30">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 py-4 flex flex-col sm:flex-row items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="p-2.5 bg-gradient-to-tr from-amber-600 to-amber-400 text-slate-950 rounded-xl shadow-lg shadow-amber-500/20">
              <Flame className="w-6 h-6" />
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
          </div>
        </div>
      </header>

      {/* Main Container */}
      <main className="max-w-7xl mx-auto px-4 sm:px-6 py-6 flex-1 w-full space-y-6">
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

        {/* Tampilan Detail Monitor vs Grid Dashboard */}
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
            {/* Toolbar: Search & Refresh */}
            <div className="flex flex-col sm:flex-row items-center justify-between gap-3 pt-2">
              <div className="relative w-full sm:w-80">
                <Search className="w-4 h-4 absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-500" />
                <input
                  type="text"
                  placeholder="Cari nomor puzzle, bits, atau address..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full bg-slate-900/60 border border-slate-800 rounded-xl pl-9 pr-3 py-2 text-xs text-white focus:outline-none focus:border-amber-500 transition"
                />
              </div>

              <div className="flex items-center gap-2 w-full sm:w-auto justify-end text-xs text-slate-400">
                <button
                  onClick={refreshAll}
                  className="p-2 bg-slate-900 border border-slate-800 hover:border-slate-700 text-slate-300 rounded-xl flex items-center gap-1.5 transition text-xs"
                  title="Refresh Data"
                >
                  <RefreshCw className="w-3.5 h-3.5" />
                  <span>Refresh</span>
                </button>
                <span className="text-[11px] text-slate-500">
                  Refreshed: {lastRefreshed.toLocaleTimeString()}
                </span>
              </div>
            </div>

            {/* Grid Puzzles */}
            {filteredPuzzles.length === 0 ? (
              <div className="text-center py-20 bg-slate-900/20 border border-slate-800/50 rounded-2xl">
                <ShieldAlert className="w-12 h-12 text-slate-600 mx-auto mb-3" />
                <h3 className="text-base font-semibold text-slate-300">Tidak ada puzzle yang cocok</h3>
                <p className="text-xs text-slate-500 mt-1">
                  Klik tombol "+ Tambah Puzzle" di atas untuk mendaftarkan puzzle baru ke database PostgreSQL.
                </p>
              </div>
            ) : (
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
            )}
          </>
        )}
      </main>

      {/* Footer */}
      <footer className="border-t border-slate-900 bg-slate-950/60 py-4 text-center text-xs text-slate-500">
        Bitcoin Puzzle Hunter Engine &copy; 2026 — Rust Axum + PostgreSQL + Tokio
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
    </div>
  );
};

