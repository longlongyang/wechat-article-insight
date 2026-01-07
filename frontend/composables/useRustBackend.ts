/**
 * Composable for making API calls to the Rust backend
 */
export function useRustBackend() {
  const config = useRuntimeConfig();
  const baseUrl = config.public.rustBackendUrl as string;

  /**
   * Make a fetch request to the Rust backend
   */
  /**
   * Make a fetch request to the Rust backend
   */
  async function rustFetch<T>(path: string, options: RequestInit & { timeout?: number } = {}): Promise<T> {
    const url = `${baseUrl}${path}`;

    const headers: Record<string, string> = {
      ...(options.headers as Record<string, string>),
    };

    // If body is FormData, let browser set Content-Type (multipart/form-data with boundary)
    // Otherwise default to application/json
    if (!headers['Content-Type'] && !(options.body instanceof FormData)) {
      headers['Content-Type'] = 'application/json';
    }

    // Default timeout 300s (5 minutes) for long running tasks like embedding
    const timeout = options.timeout ?? 300000;
    const controller = new AbortController();
    const id = setTimeout(() => controller.abort(), timeout);

    try {
      const response = await fetch(url, {
        ...options,
        headers,
        credentials: 'include',
        signal: controller.signal,
      });

      if (!response.ok) {
        const error = await response.json().catch(() => ({ error: response.statusText }));
        throw new Error(error.error || `HTTP ${response.status}`);
      }

      return response.json();
    } finally {
      clearTimeout(id);
    }
  }

  /**
   * GET request to Rust backend
   */
  async function rustGet<T>(path: string, params?: Record<string, string>): Promise<T> {
    let url = path;
    if (params) {
      const searchParams = new URLSearchParams(params);
      url = `${path}?${searchParams.toString()}`;
    }
    return rustFetch<T>(url, { method: 'GET' });
  }

  /**
   * POST request to Rust backend
   */
  async function rustPost<T>(path: string, body?: unknown, options?: RequestInit & { timeout?: number }): Promise<T> {
    const isFormData = body instanceof FormData;
    return rustFetch<T>(path, {
      method: 'POST',
      body: isFormData ? (body as FormData) : body ? JSON.stringify(body) : undefined,
      ...options,
    });
  }

  return {
    baseUrl,
    rustFetch,
    rustGet,
    rustPost,
  };
}
