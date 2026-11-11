//! Authentication Utilities & Global API Interceptor for Cloud Security & Vercel Support

const TOKEN_KEY = 'puzzle_auth_token';
const USER_KEY = 'puzzle_auth_user';
const BACKEND_URL_KEY = 'puzzle_backend_url';

/**
 * Mendapatkan basis URL API backend.
 * Prioritas:
 * 1. Environment variable Vite: `import.meta.env.VITE_API_URL` (misal diset di Vercel: http://IP_GPU:3001)
 * 2. Manual setting di localStorage: `puzzle_backend_url` (bisa diubah via UI)
 * 3. Kosong (string kosong "") -> fallback ke relative path (untuk mode lokal/proxy)
 */
export function getApiBaseUrl(): string {
  // 1. Env variable dari Vite/Vercel
  const envUrl = (import.meta.env.VITE_API_URL as string | undefined)?.trim();
  if (envUrl) {
    return envUrl.replace(/\/+$/, '');
  }

  // 2. Manual URL dari localStorage
  const localUrl = localStorage.getItem(BACKEND_URL_KEY)?.trim();
  if (localUrl) {
    return localUrl.replace(/\/+$/, '');
  }

  // 3. Fallback relative
  return '';
}

export function setApiBaseUrl(url: string): void {
  const clean = url.trim().replace(/\/+$/, '');
  if (clean) {
    localStorage.setItem(BACKEND_URL_KEY, clean);
  } else {
    localStorage.removeItem(BACKEND_URL_KEY);
  }
}

export function getAuthToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setAuthToken(token: string): void {
  localStorage.setItem(TOKEN_KEY, token);
}

export function clearAuthToken(): void {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(USER_KEY);
}

export function getAuthUser(): string {
  return localStorage.getItem(USER_KEY) || 'admin';
}

export function setAuthUser(user: string): void {
  localStorage.setItem(USER_KEY, user);
}

/**
 * Memverifikasi apakah token saat ini masih valid di backend
 */
export async function verifyCurrentToken(): Promise<{ valid: boolean; username?: string }> {
  const token = getAuthToken();
  if (!token) return { valid: false };

  try {
    const baseUrl = getApiBaseUrl();
    const endpoint = baseUrl ? `${baseUrl}/api/auth/verify` : '/api/auth/verify';
    
    const res = await fetch(endpoint, {
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });

    if (res.ok) {
      const data = await res.json();
      return { valid: true, username: data.username };
    }
  } catch (e) {
    console.warn('Gagal memverifikasi token:', e);
  }

  return { valid: false };
}

/**
 * Global Interceptor:
 * 1. Otomatis menambahkan Base URL backend jika request diawali dengan '/api' (mendukung deployment Vercel)
 * 2. Menyisipkan header Authorization Bearer secara otomatis ke seluruh pemanggilan fetch()
 */
export function setupAuthInterceptor(): void {
  const originalFetch = window.fetch;

  window.fetch = async function (input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
    const token = getAuthToken();
    const baseUrl = getApiBaseUrl();
    const headers = new Headers(init?.headers || {});

    // Selalu sisipkan token jika tersedia dan belum di-set manual
    if (token && !headers.has('Authorization')) {
      headers.set('Authorization', `Bearer ${token}`);
    }

    // Resolusi URL backend untuk Vercel (jika baseUrl diset dan input adalah relative path /api)
    let finalInput = input;
    if (baseUrl) {
      if (typeof input === 'string' && input.startsWith('/api')) {
        finalInput = `${baseUrl}${input}`;
      } else if (input instanceof Request && input.url.startsWith('/api')) {
        finalInput = new Request(`${baseUrl}${input.url}`, input);
      }
    }

    const res = await originalFetch(finalInput, { ...init, headers });

    // Jika menerima 401 Unauthorized dan bukan di endpoint login, reset sesi
    if (res.status === 401) {
      const urlStr =
        typeof input === 'string'
          ? input
          : input instanceof URL
          ? input.toString()
          : input.url;

      if (!urlStr.includes('/api/auth/login')) {
        clearAuthToken();
        window.dispatchEvent(new Event('puzzle_auth_expired'));
      }
    }

    return res;
  };
}
