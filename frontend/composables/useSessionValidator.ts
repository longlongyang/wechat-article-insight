import type { GetAuthKeyResult } from '~/types/types';
import { rustBackendGet } from '~/utils/rustBackendApi';

// Extended type with new fields
interface ExtendedAuthKeyResult extends GetAuthKeyResult {
  expires_at?: number;
  expires_soon?: boolean;
}

/**
 * 验证微信登录 session 是否仍然有效
 *
 * 在页面加载时调用，如果 session 失效则清除本地登录信息
 *
 * Response codes from backend:
 * - 0: Valid session
 * - -1: No auth key found
 * - -2: Session expired
 * - -3: Session expiring soon (within 1 hour)
 */
export async function validateWeChatSession(): Promise<{
  valid: boolean;
  expiringSoon: boolean;
  expiresAt: number | null;
}> {
  const loginAccount = useLoginAccount();
  const toast = useToast();

  // 如果没有本地登录信息，直接返回
  if (!loginAccount.value) {
    return { valid: false, expiringSoon: false, expiresAt: null };
  }

  try {
    // 向服务端验证 auth-key 是否有效
    const resp = await rustBackendGet<ExtendedAuthKeyResult>('/api/public/v1/authkey');

    if (resp.code === 0) {
      console.log('[Session] WeChat session is valid, expires at:', resp.expires_at);
      return {
        valid: true,
        expiringSoon: resp.expires_soon || false,
        expiresAt: resp.expires_at || null,
      };
    } else if (resp.code === -3) {
      // Session expiring soon - warn user but don't logout
      console.warn('[Session] WeChat session expiring soon');
      toast.add({
        title: '登录即将过期',
        description: '您的微信登录即将在1小时内过期，请及时重新登录',
        color: 'yellow',
        timeout: 10000,
      });
      return {
        valid: true, // Still valid for now
        expiringSoon: true,
        expiresAt: resp.expires_at || null,
      };
    } else {
      // Session 失效 (code -1 or -2)，清除本地登录信息
      console.warn('[Session] WeChat session expired or not found, clearing local login data');
      loginAccount.value = null;

      // 弹出通知
      toast.add({
        title: '微信登录已过期',
        description: '请重新扫码登录',
        color: 'red',
        timeout: 5000,
      });

      return { valid: false, expiringSoon: false, expiresAt: null };
    }
  } catch (error: any) {
    console.error('[Session] Failed to validate session:', error.message);
    // 验证失败不清除登录信息（可能是网络问题）
    return { valid: false, expiringSoon: false, expiresAt: null };
  }
}

/**
 * 手动登出
 */
export function logoutWeChat(): void {
  const loginAccount = useLoginAccount();
  loginAccount.value = null;

  // 清除 auth-key cookie
  document.cookie = 'auth-key=; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT';

  console.log('[Session] Logged out');
}

export default function useSessionValidator() {
  const loginAccount = useLoginAccount();
  const modal = useModal();
  const isValidating = ref(false);
  const isLoggedIn = computed(() => loginAccount.value !== null);
  const sessionExpiringSoon = ref(false);
  const sessionExpiresAt = ref<number | null>(null);

  // 在挂载时验证 session
  onMounted(async () => {
    if (loginAccount.value) {
      isValidating.value = true;
      const result = await validateWeChatSession();
      sessionExpiringSoon.value = result.expiringSoon;
      sessionExpiresAt.value = result.expiresAt;

      // 如果 session 无效且之前有登录信息，弹出登录框
      if (!result.valid && !loginAccount.value) {
        // 已在 validateWeChatSession 中处理登出和通知
      }

      isValidating.value = false;
    }
  });

  return {
    isLoggedIn,
    isValidating,
    sessionExpiringSoon,
    sessionExpiresAt,
    logout: logoutWeChat,
    validate: validateWeChatSession,
  };
}
