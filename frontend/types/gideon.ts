/**
 * Gideon 夜行者 - 类型定义
 */

/** 种子源类型 */
export type SeedSource = 'hacker-news' | 'github-trending' | 'wikipedia-random' | 'wechat' | 'continue-last';

/** 种子源配置 */
export interface SeedConfig {
  source: SeedSource;
  weight: number;
  enabled: boolean;
}

/** 链接评分结果 */
export interface LinkScore {
  url: string;
  title: string;
  context: string;
  score: number;
  isMutation: boolean;
}

/** 捕获的内容 */
export interface GideonCapture {
  id?: number;
  logId: number;
  url: string;
  title: string;
  content: string;
  insight: string;
  score: number;
  isMutation: boolean;
  capturedAt: number;
}

/** 游走日志 */
export interface GideonLog {
  id?: number;
  date: string;
  startTime: number;
  endTime?: number;
  seedSource: SeedSource;
  linksVisited: number;
  capturesCount: number;
  maxDepth: number;
  status: 'running' | 'completed' | 'failed' | 'cancelled';
  report?: string;
  tags: string[];
}

/** 游走状态 */
export interface WalkState {
  isWalking: boolean;
  currentUrl: string | null;
  currentDepth: number;
  linksVisited: number;
  captures: GideonCapture[];
  startTime: number | null;
  seedSource: SeedSource | null;
}

/** Gideon 配置 */
export interface GideonConfig {
  provider: 'gemini' | 'deepseek';
  // Gemini Config
  geminiApiKey: string;
  geminiModel:
    | 'gemini-3-pro-preview'
    | 'gemini-3-flash-preview'
    | 'gemini-2.5-flash'
    | 'gemini-2.0-flash-exp'
    | 'gemini-1.5-flash'
    | 'gemini-1.5-pro';
  // DeepSeek Config
  deepseekApiKey: string;
  deepseekModel: 'deepseek-chat' | 'deepseek-reasoner';

  maxDepth: number;
  mutationRate: number; // 0-1, default 0.2
  requestDelayMs: number;
  seeds: SeedConfig[];
  // 连续夜行配置
  autoRepeat: boolean; // 是否自动重复
  walkIntervalMinutes: number; // 两次夜行之间的间隔（分钟）
  // 代理配置
  proxyEnabled: boolean;
  proxyHost: string;
  proxyPort: number;
  proxyUsername?: string;
  proxyPassword?: string;
}

/** 默认配置 */
export const DEFAULT_GIDEON_CONFIG: GideonConfig = {
  provider: 'gemini',
  geminiApiKey: '',
  geminiModel: 'gemini-3-flash-preview',
  deepseekApiKey: '',
  deepseekModel: 'deepseek-chat',
  maxDepth: 3,
  mutationRate: 0.2,
  requestDelayMs: 2000,
  seeds: [
    { source: 'hacker-news', weight: 0.3, enabled: true },
    { source: 'github-trending', weight: 0.3, enabled: true },
    { source: 'wikipedia-random', weight: 0.2, enabled: true },
    { source: 'wechat', weight: 0.1, enabled: true },
    { source: 'continue-last', weight: 0.1, enabled: true },
  ],
  autoRepeat: false,
  walkIntervalMinutes: 5,
  proxyEnabled: false,
  proxyHost: '',
  proxyPort: 7890,
  proxyUsername: '',
  proxyPassword: '',
};
