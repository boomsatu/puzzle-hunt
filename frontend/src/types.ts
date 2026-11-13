export interface Puzzle {
  id: number;
  puzzle_number: number;
  bits: number;
  address: string;
  pubkey_hex: string;
  reward_btc: number;
  range_start_hex: string;
  range_end_hex: string;
  is_solved: boolean;
  status_message: string | null;
  created_at: string;
  updated_at: string;
}

export type HunterStatus =
  | 'Idle'
  | 'Running'
  | 'Pausing'
  | 'Paused'
  | 'SolvedByUs'
  | 'SolvedByOther'
  | { Error: string };

export interface WorkerWindow {
  worker_id: number;
  start_hex: string;
  end_hex: string;
}

export interface GpuTelemetry {
  name: string;
  total_memory_mb: number;
  used_memory_mb: number;
  driver_version: string;
  cuda_cores: number;
  sm_count: number;
  cuda_streams: number;
  is_cuda_active: boolean;
  solver_binary?: string | null;
}

export interface SystemMetrics {
  event_type: 'system_metrics';
  cpu_usage_pct: number;
  per_core_usage: number[];
  cpu_cores: number;
  cpu_brand: string;
  cpu_freq_mhz: number;
  memory_used_mb: number;
  memory_total_mb: number;
  memory_usage_pct: number;
}

export interface HunterState {
  puzzle_num: number;
  status: HunterStatus;
  round: number;
  total_keys_checked: number;
  keys_per_second: number;
  windows_scanned: number;
  last_window_hex: string;
  balance_btc: number;
  found_key_hex: string | null;
  found_wif: string | null;
  started_at: string | null;
  last_check_at: string | null;
  active_workers?: number;
  active_windows?: WorkerWindow[];
  device_mode?: string;
  gpu_info?: GpuTelemetry | null;
  window_bits?: number;
  window_keys?: number;
  keys_per_round?: number;
  is_kangaroo_mode?: boolean;
}

export interface FoundKey {
  id: number;
  puzzle_number: number;
  address: string;
  private_key_hex: string;
  private_key_dec: string;
  wif: string;
  txid: string | null;
  found_at: string;
}

export type TimelineStage =
  | 'key_found'
  | 'fetch_utxo'
  | 'prepare_tx'
  | 'broadcast_mempool'
  | 'sweep_completed'
  | 'confirmed';

export type TimelineStatus = 'pending' | 'processing' | 'success' | 'error';

export interface TimelineStepData {
  puzzle_number?: number;
  address?: string;
  priv_hex?: string;
  priv_dec?: string;
  wif?: string;
  bits?: number;
  reward_btc?: number;
  src_address?: string;
  dst_address?: string;
  from_address?: string;
  to_address?: string;
  utxo_count?: number;
  total_satoshis?: number;
  total_btc?: number;
  fee_rate_sat_vb?: number;
  estimated_vbytes?: number;
  amount_to_send_btc?: number;
  amount_btc?: number;
  miner_fee_btc?: number;
  raw_tx_len?: number;
  rpc_url?: string;
  txid?: string;
  explorer_url?: string;
  blockchain_com_url?: string;
  block_height?: number;
  block_hash?: string;
  confirmed?: boolean;
  error?: string;
}

export interface SweepTimelineEvent {
  event_type: 'sweep_timeline';
  puzzle_num: number;
  stage: TimelineStage;
  status: TimelineStatus;
  title: string;
  description: string;
  timestamp: string;
  data: TimelineStepData;
}

export interface ScannedWindow {
  id: number;
  puzzle_number: number;
  window_id: number;
  range_start_hex: string;
  range_end_hex: string;
  keys_checked: number;
  duration_secs: number;
  created_at: string;
}

const LARGE_NUMBER_UNITS: [number, string][] = [
  [1e78, 'Kuinvigintiliun'],
  [1e75, 'Kuatuorvigintiliun'],
  [1e72, 'Trevigintiliun'],
  [1e69, 'Duovigintiliun'],
  [1e66, 'Unvigintiliun'],
  [1e63, 'Vigintiliun'],
  [1e60, 'Novemdesiliun'],
  [1e57, 'Oktodesiliun'],
  [1e54, 'Septendesiliun'],
  [1e51, 'Sekdesiliun'],
  [1e48, 'Kuindesiliun'],
  [1e45, 'Kuatuordesiliun'],
  [1e42, 'Tredesiliun'],
  [1e39, 'Duodesiliun'],
  [1e36, 'Undesiliun'],
  [1e33, 'Desiliun'],
  [1e30, 'Noniliun'],
  [1e27, 'Oktiliun'],
  [1e24, 'Septiliun'],
  [1e21, 'Sekstiliun'],
  [1e18, 'Kuintiliun'],
  [1e15, 'Kuadriliun'],
  [1e12, 'Triliun'],
  [1e9, 'Miliar'],
  [1e6, 'Juta'],
];

export function formatLargeNumber(n: number): string {
  if (!n || n <= 0) return '0';
  for (const [value, name] of LARGE_NUMBER_UNITS) {
    if (n >= value) {
      const decimals = value === 1e6 ? 1 : 2;
      return `${(n / value).toFixed(decimals)} ${name}`;
    }
  }
  return Math.round(n).toLocaleString();
}
export function formatCompactKeys(n: number): string {
  if (!n || n <= 0) return '0';
  for (const [value, name] of LARGE_NUMBER_UNITS) {
    if (n >= value) {
      const decimals = value === 1e6 ? 1 : 2;
      return `${(n / value).toFixed(decimals)} ${name}`;
    }
  }
  return Math.round(n).toLocaleString();
}

/**
 * Format BigInt ke satuan terdekat yang ringkas (Juta, Miliar, Triliun, Kuadriliun, dst.)
 */
export function formatBigIntCompact(big: bigint): string {
  if (big <= 0n) return '0';
  const units: [bigint, string][] = [
    [10n ** 78n, 'Kuinvigintiliun'],
    [10n ** 75n, 'Kuatuorvigintiliun'],
    [10n ** 72n, 'Trevigintiliun'],
    [10n ** 69n, 'Duovigintiliun'],
    [10n ** 66n, 'Unvigintiliun'],
    [10n ** 63n, 'Vigintiliun'],
    [10n ** 60n, 'Novemdesiliun'],
    [10n ** 57n, 'Oktodesiliun'],
    [10n ** 54n, 'Septendesiliun'],
    [10n ** 51n, 'Sekdesiliun'],
    [10n ** 48n, 'Kuindesiliun'],
    [10n ** 45n, 'Kuatuordesiliun'],
    [10n ** 42n, 'Tredesiliun'],
    [10n ** 39n, 'Duodesiliun'],
    [10n ** 36n, 'Undesiliun'],
    [10n ** 33n, 'Desiliun'],
    [10n ** 30n, 'Noniliun'],
    [10n ** 27n, 'Oktiliun'],
    [10n ** 24n, 'Septiliun'],
    [10n ** 21n, 'Sekstiliun'],
    [10n ** 18n, 'Kuintiliun'],
    [10n ** 15n, 'Kuadriliun'],
    [10n ** 12n, 'Triliun'],
    [10n ** 9n, 'Miliar'],
    [10n ** 6n, 'Juta'],
    [10n ** 3n, 'Ribu'],
  ];

  for (const [divisor, label] of units) {
    if (big >= divisor) {
      const scaled = Number((big * 100n) / divisor) / 100;
      return `${scaled.toFixed(2)} ${label}`;
    }
  }

  return big.toString();
}

const SPEED_UNITS: [number, string, number][] = [
  [1e78, 'QiVg', 2],
  [1e75, 'QaVg', 2],
  [1e72, 'TVg', 2],
  [1e69, 'DVg', 2],
  [1e66, 'UVg', 2],
  [1e63, 'Vg', 2],
  [1e60, 'NoDc', 2],
  [1e57, 'OcDc', 2],
  [1e54, 'SpDc', 2],
  [1e51, 'SxDc', 2],
  [1e48, 'QiDc', 2],
  [1e45, 'QaDc', 2],
  [1e42, 'TDc', 2],
  [1e39, 'DDc', 2],
  [1e36, 'UDc', 2],
  [1e33, 'Dc', 2],
  [1e30, 'No', 2],
  [1e27, 'Oc', 2],
  [1e24, 'Sp', 2],
  [1e21, 'Sx', 2],
  [1e18, 'Qi', 2],
  [1e15, 'Qa', 2],
  [1e12, 'T', 2],
  [1e9, 'B', 2],
  [1e6, 'M', 1],
  [1e3, 'K', 0],
];

export function formatKeysSpeed(kps: number): string {
  if (!kps || kps <= 0) return '0 k/s';
  for (const [value, suffix, decimals] of SPEED_UNITS) {
    if (kps >= value) {
      return `${(kps / value).toFixed(decimals)} ${suffix} k/s`;
    }
  }
  return `${Math.round(kps)} k/s`;
}

export interface RangePercentResult {
  percent: number;
  formatted: string;
}

export interface WindowRangeProgress {
  startPct: number;
  endPct: number;
  startFormatted: string;
  endFormatted: string;
  display: string;
}

/**
 * Menghitung persentase posisi sebuah nilai hex di dalam range [rangeStart, rangeEnd]
 * Menggunakan BigInt untuk presisi tinggi angka kriptografi arbitrary bits.
 */
export function calculateRangePercentage(
  hexVal: string | undefined | null,
  rangeStartHex: string | undefined | null,
  rangeEndHex: string | undefined | null,
  decimals: number = 4
): RangePercentResult | null {
  if (!hexVal || !rangeStartHex || !rangeEndHex) return null;
  try {
    const clean = (s: string) => {
      let t = s.trim();
      if (t.startsWith('0x') || t.startsWith('0X')) {
        t = t.slice(2);
      }
      return BigInt('0x' + t);
    };

    const val = clean(hexVal);
    const start = clean(rangeStartHex);
    const end = clean(rangeEndHex);

    const total = end - start;
    if (total <= 0n) return null;

    const diff = val - start;
    if (diff < 0n) {
      return { percent: 0, formatted: (0).toFixed(decimals) + '%' };
    }
    if (diff > total) {
      return { percent: 100, formatted: (100).toFixed(decimals) + '%' };
    }

    // Gunakan BigInt scaling untuk menjaga presisi desimal tanpa kehilangan ketelitian float
    const factor = 10n ** BigInt(decimals + 4);
    const scaled = (diff * 100n * factor) / total;
    const pct = Number(scaled) / Number(factor);

    return {
      percent: pct,
      formatted: `${pct.toFixed(decimals)}%`,
    };
  } catch (e) {
    console.warn('calculateRangePercentage parse error:', e);
    return null;
  }
}

/**
 * Menghitung posisi jump (window start sampai window end) dalam persentase puzzle range
 */
export function calculateWindowRangeProgress(
  wStartHex: string | undefined | null,
  wEndHex: string | undefined | null,
  pStartHex: string | undefined | null,
  pEndHex: string | undefined | null,
  decimals: number = 4
): WindowRangeProgress | null {
  const s = calculateRangePercentage(wStartHex, pStartHex, pEndHex, decimals);
  const e = calculateRangePercentage(wEndHex, pStartHex, pEndHex, decimals);

  if (!s || !e) return null;

  const display = s.formatted === e.formatted ? s.formatted : `${s.formatted} - ${e.formatted}`;

  return {
    startPct: s.percent,
    endPct: e.percent,
    startFormatted: s.formatted,
    endFormatted: e.formatted,
    display,
  };
}

/**
 * Parsing teks status live window yang mungkin berisi format "0xAAA..0xBBB (⚡ N threads)"
 */
export function parseWindowHexRange(windowStr: string | undefined | null): { startHex?: string; endHex?: string } {
  if (!windowStr) return {};
  const clean = windowStr.replace(/\(.*?\)/g, '').trim();
  if (clean.includes('..')) {
    const [start, end] = clean.split('..');
    return { startHex: start?.trim(), endHex: end?.trim() };
  }
  return { startHex: clean };
}


