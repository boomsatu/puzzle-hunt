//! Authentication Utilities & Global API Interceptor for Cloud Security

const TOKEN_KEY = 'puzzle_auth_token';
const USER_KEY = 'puzzle_auth_user';

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
    const res = await fetch('/api/auth/verify', {
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
 * Global Interceptor: Menyisipkan header Authorization Bearer secara otomatis
 * ke seluruh pemanggilan fetch() di aplikasi.
 */
export function setupAuthInterceptor(): void {
  const originalFetch = window.fetch;

  window.fetch = async function (input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
    const token = getAuthToken();
    const headers = new Headers(init?.headers || {});

    // Selalu sisipkan token jika tersedia dan belum di-set manual
    if (token && !headers.has('Authorization')) {
      headers.set('Authorization', `Bearer ${token}`);
    }

    const res = await originalFetch(input, { ...init, headers });

    // Jika menerima 401 Unauthorized dan bukan di endpoint login, reset sesi
    if (res.status === 401) {
      const url =
        typeof input === 'string'
          ? input
          : input instanceof URL
          ? input.toString()
          : input.url;

      if (!url.includes('/api/auth/login')) {
        clearAuthToken();
        window.dispatchEvent(new Event('puzzle_auth_expired'));
      }
    }

    return res;
  };
}

