import { useRustBackend } from '~/composables/useRustBackend';

export interface Info {
  fakeid: string;
  completed: boolean;
  count: number;
  articles: number;
  nickname?: string;
  round_head_img?: string;
  total_count: number;
  create_time?: number;
  update_time?: number;
  last_update_time?: number;
  syncAll?: boolean;
}

export async function updateInfoCache(info: Info): Promise<boolean> {
  const { rustPost } = useRustBackend();
  try {
    await rustPost('/api/sync/accounts', { accounts: [info] });
    return true;
  } catch (e) {
    console.error('Failed to update account info:', e);
    return false;
  }
}

export async function updateLastUpdateTime(fakeid: string): Promise<boolean> {
  const { rustPost } = useRustBackend();
  try {
    // Get current info and update the update_time
    const info = await getInfoCache(fakeid);
    if (info) {
      info.update_time = Math.floor(Date.now() / 1000);
      await rustPost('/api/sync/accounts', { accounts: [info] });
    }
    return true;
  } catch (e) {
    console.error('Failed to update last update time:', e);
    return false;
  }
}

export async function updateSyncAll(fakeid: string, syncAll: boolean): Promise<boolean> {
  const { rustPost } = useRustBackend();
  try {
    const info = await getInfoCache(fakeid);
    if (info) {
      await rustPost('/api/sync/accounts', { accounts: [{ ...info, syncAll }] });
    }
    return true;
  } catch (e) {
    console.error('Failed to update syncAll:', e);
    return false;
  }
}

export async function getInfoCache(fakeid: string): Promise<Info | undefined> {
  const all = await getAllInfo();
  return all.find(i => i.fakeid === fakeid);
}

export async function getAllInfo(): Promise<Info[]> {
  const { rustGet } = useRustBackend();
  try {
    const res = await rustGet<any>('/api/public/v1/accounts/db?limit=1000');
    if (res.success && Array.isArray(res.data)) {
      return res.data;
    }
  } catch (e) {
    console.error('Failed to fetch accounts from backend', e);
  }
  return [];
}

export async function getAccountNameByFakeid(fakeid: string): Promise<string | null> {
  const account = await getInfoCache(fakeid);
  return account?.nickname || null;
}

export async function importInfos(infos: Info[]): Promise<void> {
  const { rustPost } = useRustBackend();
  try {
    // Ensure all accounts have required fields
    const accounts = infos.map(info => ({
      fakeid: info.fakeid,
      nickname: info.nickname || '',
      round_head_img: info.round_head_img || '',
      count: info.count || 0,
      articles: info.articles || 0,
      total_count: info.total_count || 0,
      create_time: info.create_time || Math.floor(Date.now() / 1000),
      update_time: info.update_time || Math.floor(Date.now() / 1000),
      syncAll: info.syncAll || false,
    }));

    await rustPost('/api/sync/accounts', { accounts });
    console.log(`[Import] Imported ${accounts.length} accounts`);
  } catch (e) {
    console.error('Failed to import accounts:', e);
    throw e;
  }
}
