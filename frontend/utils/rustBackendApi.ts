/**
 * Rust Backend API client for use in non-component files (utils)
 *
 * This is a standalone version of useRustBackend for use outside Vue components
 */

// Get the backend URL from window config or fallback
function getRustBackendUrl(): string {
  if (typeof window !== 'undefined' && (window as any).__NUXT__?.config?.public?.rustBackendUrl) {
    return (window as any).__NUXT__.config.public.rustBackendUrl;
  }
  return 'http://localhost:3001';
}

/**
 * Make a GET request to the Rust backend
 */
export async function rustBackendGet<T>(path: string, params?: Record<string, any>): Promise<T> {
  const baseUrl = getRustBackendUrl();
  let url = `${baseUrl}${path}`;

  if (params) {
    // Filter out undefined/null and convert to string
    const safeParams: Record<string, string> = {};
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        safeParams[key] = String(value);
      }
    });
    const searchParams = new URLSearchParams(safeParams);
    url = `${url}?${searchParams.toString()}`;
  }

  const response = await fetch(url, {
    method: 'GET',
    headers: {
      'Content-Type': 'application/json',
    },
    credentials: 'include',
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: response.statusText }));
    throw new Error(error.error || `HTTP ${response.status}`);
  }

  return response.json();
}

/**
 * Make a POST request to the Rust backend
 */
export async function rustBackendPost<T>(path: string, body?: unknown): Promise<T> {
  const baseUrl = getRustBackendUrl();
  const url = `${baseUrl}${path}`;

  const response = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    credentials: 'include',
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!response.ok) {
    const error = await response.json().catch(() => ({ error: response.statusText }));
    throw new Error(error.error || `HTTP ${response.status}`);
  }

  return response.json();
}

/**
 * Make a raw fetch request to the Rust backend
 * Useful for handling Blobs or other non-JSON responses
 */
export async function rustBackendFetch(path: string, options: RequestInit = {}): Promise<Response> {
  const baseUrl = getRustBackendUrl();
  const url = `${baseUrl}${path}`;

  return fetch(url, options);
}
