import { useRustBackend } from '~/composables/useRustBackend';

export interface HtmlAsset {
  fakeid: string;
  url: string;
  file: Blob;
  title: string;
  commentID: string | null;
}

export async function updateHtmlCache(html: HtmlAsset): Promise<boolean> {
  // console.warn('updateHtmlCache called in backend-only mode. No-op.');
  return true;
}

export async function getHtmlCache(url: string): Promise<HtmlAsset | undefined> {
  try {
    const { rustGet } = useRustBackend();
    // Fetch HTML string from backend
    const htmlContent = await rustGet<string>(`/api/public/v1/html?url=${encodeURIComponent(url)}`, {
      responseType: 'text',
    });

    if (htmlContent) {
      return {
        fakeid: '',
        url: url,
        file: new Blob([htmlContent], { type: 'text/html' }),
        title: '',
        commentID: null,
      };
    }
  } catch (e) {
    // console.warn('Failed to fetch html', e);
  }
  return undefined;
}

export async function getAllHtmlCache(): Promise<HtmlAsset[]> {
  // Need backend API if we want to list all cached HTML.
  return [];
}

export async function deleteHtmlCache(url: string): Promise<void> {
  // Call backend delete?
}

export async function deleteMultipleHtmlCache(urls: string[]): Promise<void> {
  // Call backend delete?
}
