import { useRustBackend } from '~/composables/useRustBackend';
import type { AppMsgExWithFakeID, PublishInfo, PublishPage } from '~/types/types';
import { type Info } from './info';

export type ArticleAsset = AppMsgExWithFakeID;

export async function updateArticleCache(account: Info, publish_page: PublishPage) {
  const { rustPost } = useRustBackend();

  // Extract articles from publish_page
  const articles: any[] = [];
  for (const item of publish_page.publish_list) {
    if (!item.publish_info) continue;

    try {
      const publishInfo: PublishInfo = JSON.parse(item.publish_info);
      for (const article of publishInfo.appmsgex) {
        articles.push({
          id: `${account.fakeid}:${article.aid}`,
          fakeid: account.fakeid,
          aid: article.aid,
          title: article.title || '',
          link: article.link,
          create_time: article.create_time,
          update_time: article.update_time,
          digest: article.digest || '',
          cover: article.cover || article.pic_cdn_url_1_1 || '',
          is_deleted: article.is_deleted || false,
          item_show_type: article.item_show_type,
          itemidx: article.itemidx,
          raw_json: article,
        });
      }
    } catch (e) {
      console.error('Failed to parse publish_info:', e);
    }
  }

  if (articles.length === 0) return;

  try {
    // Send articles to backend
    const result = await rustPost<{ success: boolean; stored: number; failed: number }>('/api/sync/articles', {
      articles,
    });

    // Also update account info with new counts (backend accumulates these)
    const newMsgCount = articles.filter(a => a.itemidx === 1).length;
    const newArticleCount = articles.length;
    const accountUpdate = {
      fakeid: account.fakeid,
      nickname: account.nickname,
      round_head_img: account.round_head_img,
      total_count: publish_page.total_count,
      // Send delta counts - backend will add these to existing values
      count: newMsgCount,
      articles: newArticleCount,
      update_time: Math.floor(Date.now() / 1000),
      last_update_time: articles.length > 0 ? Math.max(...articles.map((a: any) => a.create_time)) : 0,
    };

    await rustPost('/api/sync/accounts', { accounts: [accountUpdate] });

    console.log(`[Sync] Stored ${result.synced} articles for ${account.nickname}`);
  } catch (e) {
    console.error('Failed to sync articles to backend:', e);
  }
}

export async function hitCache(fakeid: string, create_time: number): Promise<boolean> {
  // In backend mode, we don't need local cache hit detection
  // Always return false to allow normal sync flow
  return false;
}

export async function getArticleCache(fakeid: string, create_time: number): Promise<AppMsgExWithFakeID[]> {
  // Fetch all articles for this account from backend
  try {
    const { rustGet } = useRustBackend();
    const res = await rustGet<any>(`/api/public/v1/articles/db?fakeid=${fakeid}&limit=10000`);
    if (res.success && Array.isArray(res.data)) {
      return res.data;
    }
  } catch (e) {
    console.warn('Failed to fetch articles from backend', e);
  }
  return [];
}

export async function getArticleByLink(url: string): Promise<AppMsgExWithFakeID> {
  // Search articles by link - not implemented yet
  // For now, throw error
  throw new Error(`getArticleByLink not implemented yet (${url})`);
}

export async function articleDeleted(url: string): Promise<void> {
  // Mark article as deleted - no-op for now
  console.warn('articleDeleted not implemented in backend-only mode');
}

export async function getRecentArticles(
  days: number,
  limit: number = 500,
  offset: number = 0
): Promise<AppMsgExWithFakeID[]> {
  try {
    const { rustGet } = useRustBackend();
    const res = await rustGet<any>(`/api/public/v1/articles/db?days=${days}&limit=${limit}&offset=${offset}`);
    if (res.success && Array.isArray(res.data)) {
      return res.data;
    }
  } catch (e) {
    console.warn('Failed to fetch recent articles from backend', e);
  }
  return [];
}

export async function getRecentArticlesPaginated(
  days: number,
  offset: number,
  limit: number
): Promise<{ articles: AppMsgExWithFakeID[]; hasMore: boolean }> {
  try {
    const { rustGet } = useRustBackend();
    const res = await rustGet<any>(`/api/public/v1/articles/db?days=${days}&offset=${offset}&limit=${limit}`);
    if (res.success && Array.isArray(res.data)) {
      return {
        articles: res.data,
        hasMore: res.data.length === limit,
      };
    }
  } catch (e) {
    console.warn('Failed to fetch recent articles from backend', e);
  }
  return { articles: [], hasMore: false };
}

export async function getArticleCachePaginated(
  fakeid: string,
  offset: number,
  limit: number
): Promise<{ articles: AppMsgExWithFakeID[]; hasMore: boolean }> {
  try {
    const { rustGet } = useRustBackend();
    const res = await rustGet<any>(`/api/public/v1/articles/db?fakeid=${fakeid}&offset=${offset}&limit=${limit}`);
    if (res.success && Array.isArray(res.data)) {
      return {
        articles: res.data,
        hasMore: res.data.length === limit,
      };
    }
  } catch (e) {
    console.warn('Failed to fetch paginated articles from backend', e);
  }

  return { articles: [], hasMore: false };
}

export async function getArticleCount(fakeid: string): Promise<number> {
  // Need backend count API
  return 0;
}
