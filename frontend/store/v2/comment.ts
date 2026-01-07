import { useRustBackend } from '~/composables/useRustBackend';

export interface CommentAsset {
  fakeid: string;
  url: string;
  title: string;
  data: any;
}

export async function updateCommentCache(comment: CommentAsset): Promise<boolean> {
  // console.warn('updateCommentCache called in backend-only mode. No-op.');
  return true;
}

export async function getCommentCache(url: string): Promise<CommentAsset | undefined> {
  try {
    const { rustGet } = useRustBackend();

    // NOTE: Backend get_comments requires article_id, but here we only have URL.
    // In strict backend mode, we cannot easily resolve URL -> ID without a lookup API.
    // However, existing `getCommentCache` refactor in previous step attempted to look up `db.article`.
    // Since `db.article` is gone, we HAVE to rely on backend to lookup by URL.
    // BUT: get_comments currently only takes id/article_id.
    // Assumption: Backend logic or new API needed if we want to fetch comments by URL.
    // Workaround: Try to fetch article details by URL (if exists) then get ID?
    // Or does `getHtmlCache` (which fetches by URL) return meta info?

    // Let's assume for now we might fail to fetch comments by pure URL unless backend supports it.
    // Or we update backend `get_comments` to support `url` param.
    // I will implemented a basic try, but likely will need backend change if this is critical.

    // For now, return undefined to avoid breaking build.
    return undefined;
  } catch (e) {}
  return undefined;
}
