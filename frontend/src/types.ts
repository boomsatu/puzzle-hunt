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
  | 'Paused'
  | 'SolvedByUs'
  | 'SolvedByOther'
  | { Error: string };

export interface WorkerWindow {
  worker_id: number;
  start_hex: string;
  end_hex: string;
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

export function formatLargeNumber(n: number): string {
  if (!n || n <= 0) return '0';
  if (n >= 1_000_000_000_000) {
    return `${(n / 1_000_000_000_000).toFixed(2)} Triliun`;
  }
  if (n >= 1_000_000_000) {
    return `${(n / 1_000_000_000).toFixed(2)} Miliar`;
  }
  if (n >= 1_000_000) {
    return `${(n / 1_000_000).toFixed(1)} Juta`;
  }
  return Math.round(n).toLocaleString();
}

export function formatCompactKeys(n: number): string {
  if (!n || n <= 0) return '0';
  if (n >= 1_000_000_000_000) {
    return `${(n / 1_000_000_000_000).toFixed(2)} Triliun`;
  }
  if (n >= 1_000_000_000) {
    return `${(n / 1_000_000_000).toFixed(2)} Miliar`;
  }
  if (n >= 1_000_000) {
    return `${(n / 1_000_000).toFixed(1)} Juta`;
  }
  return Math.round(n).toLocaleString();
}

/**
 * Format BigInt ke satuan terdekat yang ringkas (Juta, Miliar, Triliun, Kuadriliun, dst.)
 */
export function formatBigIntCompact(big: bigint): string {
  if (big <= 0n) return '0';

  const units: [bigint, string][] = [
    [10n ** 33n, 'Desiliun'],
    [10n ** 30n, 'Noniliun'],
    [10n ** 27n, 'Oktiliun'],
    [10n ** 24n, 'Septiliun'],
    [10n ** 21n, 'Sekstiliun'],
    [10n ** 18n, 'Kuintiliun'],
    [10n ** 15n, 'Kuadriliun'],
    [10n ** 12n, 'Triliun'],
    [10n ** 9n,  'Miliar'],
    [10n ** 6n,  'Juta'],
    [10n ** 3n,  'Ribu'],
  ];

  for (const [divisor, label] of units) {
    if (big >= divisor) {
      const scaled = Number((big * 100n) / divisor) / 100;
      return `${scaled.toFixed(2)} ${label}`;
    }
  }

  return big.toString();
}

export function formatKeysSpeed(kps: number): string {
  if (!kps || kps <= 0) return '0 k/s';
  if (kps >= 1_000_000_000) {
    return `${(kps / 1_000_000_000).toFixed(2)} B k/s`;
  }
  if (kps >= 1_000_000) {
    return `${(kps / 1_000_000).toFixed(1)} M k/s`;
  }
  if (kps >= 1_000) {
    return `${(kps / 1_000).toFixed(0)} K k/s`;
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


