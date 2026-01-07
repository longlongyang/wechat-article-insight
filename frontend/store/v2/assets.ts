import { useRustBackend } from '~/composables/useRustBackend';

export interface Asset {
  fakeid: string;
  url: string;
  file: Blob; // or Base64 string?
  size: number;
}

export async function updateAssetCache(asset: Asset): Promise<boolean> {
  return true;
}

export async function getAssetCache(url: string): Promise<Asset | undefined> {
  try {
    const { rustGet } = useRustBackend();
    // Use Blob response type
    const blob = await rustGet<Blob>(`/api/public/v1/asset?url=${encodeURIComponent(url)}`, { responseType: 'blob' });
    if (blob && blob instanceof Blob) {
      return {
        fakeid: '',
        url: url,
        file: blob,
        size: blob.size,
      };
    }
  } catch (e) {}
  return undefined;
}
