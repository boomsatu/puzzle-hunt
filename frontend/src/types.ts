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
  if (n >= 1_000_000_000) {
    return `${(n / 1_000_000_000).toFixed(2)} Miliar (${Math.round(n).toLocaleString()})`;
  }
  if (n >= 1_000_000) {
    return `${(n / 1_000_000).toFixed(1)} Juta (${Math.round(n).toLocaleString()})`;
  }
  return Math.round(n).toLocaleString();
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

