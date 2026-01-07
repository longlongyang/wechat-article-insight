/**
 * Gideon 游走引擎
 * DFS 深度优先搜索 + 突变机制
 */

import type { GideonCapture, GideonConfig, LinkScore, SeedSource, WalkState } from '~/types/gideon';
import { rustBackendGet } from '~/utils/rustBackendApi';
import { generateInsight, scoreLinks, selectNextLink } from './scorer';
import { fetchAllSeeds, isWeChatLoggedIn } from './seeds';

/** 游走回调函数 */
export interface WalkCallbacks {
  onStateChange?: (state: Partial<WalkState>) => void;
  onCapture?: (capture: GideonCapture) => void;
  onLog?: (message: string) => void;
  onComplete?: (captures: GideonCapture[]) => void;
  onError?: (error: Error) => void;
}

/** 已访问 URL 集合 */
const visitedUrls = new Set<string>();

/** 已捕获记录 (包含标题和洞察) */
export interface CapturedUrlEntry {
  url: string;
  title: string;
  insight: string;
  capturedAt: number;
}

const CAPTURED_URLS_KEY = 'gideon-captured-urls';
const CAPTURED_URLS_TTL_DAYS = 14;

// 存储所有捕获记录
let capturedEntries: CapturedUrlEntry[] = [];

/**
 * 从 localStorage 加载已捕获记录
 */
function loadCapturedEntries(): CapturedUrlEntry[] {
  if (typeof localStorage === 'undefined') return [];

  try {
    const stored = localStorage.getItem(CAPTURED_URLS_KEY);
    if (!stored) return [];

    const entries: CapturedUrlEntry[] = JSON.parse(stored);
    const now = Date.now();
    const ttlMs = CAPTURED_URLS_TTL_DAYS * 24 * 60 * 60 * 1000;

    // 过滤掉过期的条目
    const validEntries = entries.filter(entry => now - entry.capturedAt < ttlMs);

    console.log(`[Gideon] 加载了 ${validEntries.length} 条捕获记录 (14天内)`);
    return validEntries;
  } catch (e) {
    console.warn('[Gideon] 加载捕获记录失败:', e);
    return [];
  }
}

/**
 * 保存捕获记录到 localStorage
 */
function saveCapturedEntries(): void {
  if (typeof localStorage === 'undefined') return;

  try {
    localStorage.setItem(CAPTURED_URLS_KEY, JSON.stringify(capturedEntries));
  } catch (e) {
    console.warn('[Gideon] 保存捕获记录失败:', e);
  }
}

/**
 * 检查 URL 是否已捕获
 */
function isUrlCaptured(url: string): boolean {
  return capturedEntries.some(e => e.url === url);
}

/**
 * 添加捕获记录
 */
function addCapturedEntry(url: string, title: string, insight: string): void {
  capturedEntries.unshift({
    url,
    title,
    insight,
    capturedAt: Date.now(),
  });
  saveCapturedEntries();
}

// 初始化时加载
capturedEntries = loadCapturedEntries();

/**
 * 获取所有捕获记录（供页面显示）
 */
export function getCapturedUrls(): CapturedUrlEntry[] {
  // 返回副本，按时间倒序
  return [...capturedEntries].sort((a, b) => b.capturedAt - a.capturedAt);
}

/**
 * 删除一条捕获记录
 */
export function removeCapturedUrl(url: string): void {
  capturedEntries = capturedEntries.filter(e => e.url !== url);
  saveCapturedEntries();
}

/**
 * 清除所有捕获记录
 */
export function clearAllCapturedUrls(): void {
  capturedEntries = [];
  if (typeof localStorage !== 'undefined') {
    localStorage.removeItem(CAPTURED_URLS_KEY);
  }
}

/**
 * 获取捕获记录数量
 */
export function getCapturedUrlsCount(): number {
  return capturedEntries.length;
}

/**
 * 从 HTML 中提取链接
 */
function extractLinks(html: string, baseUrl: string): LinkScore[] {
  const links: LinkScore[] = [];
  // 改进的正则表达式，匹配包含嵌套标签的链接
  const linkRegex = /<a[^>]+href=["']([^"']+)["'][^>]*>([\s\S]*?)<\/a>/gi;
  let match;

  while ((match = linkRegex.exec(html)) !== null) {
    try {
      const href = match[1];
      // 移除 HTML 标签获取纯文本
      let text = match[2].replace(/<[^>]+>/g, '').trim();

      if (!href || href.startsWith('#') || href.startsWith('javascript:')) continue;

      // 解析相对 URL
      const fullUrl = new URL(href, baseUrl).toString();

      // 排除已访问和常见无意义链接
      if (visitedUrls.has(fullUrl)) continue;
      if (fullUrl.includes('login') || fullUrl.includes('signup') || fullUrl.includes('privacy')) continue;

      // 排除主页类 URL（只有域名没有有意义的路径）
      try {
        const urlObj = new URL(fullUrl);
        const pathParts = urlObj.pathname.split('/').filter(p => p);
        // 如果路径为空或只有一个很短的部分，可能是主页
        if (pathParts.length === 0) continue; // 纯主页如 github.com/
      } catch {
        continue;
      }

      // 如果没有标题，尝试从 URL 生成一个
      if (!text || text.length < 3) {
        try {
          const urlObj = new URL(fullUrl);
          // 使用路径最后一部分作为标题
          const pathParts = urlObj.pathname.split('/').filter(p => p);
          text = pathParts[pathParts.length - 1] || urlObj.hostname;
          // 清理 URL 编码
          text = decodeURIComponent(text).replace(/[-_]/g, ' ');
        } catch {
          text = 'Link';
        }
      }

      links.push({
        url: fullUrl,
        title: text.slice(0, 100), // 限制标题长度
        context: '',
        score: 0,
        isMutation: false,
      });
    } catch {
      // 忽略无效 URL
    }
  }

  return links.slice(0, 20); // 限制提取数量
}

/**
 * 抓取页面内容
 */
async function fetchPage(url: string, config: GideonConfig): Promise<{ html: string; text: string } | null> {
  try {
    const response = await rustBackendGet<{ success: boolean; data: string }>('/api/gideon/fetch', {
      url,
      proxyEnabled: config.proxyEnabled ? 'true' : 'false',
      proxyHost: config.proxyHost || '',
      proxyPort: String(config.proxyPort || 7890),
      proxyUsername: config.proxyUsername || '',
      proxyPassword: config.proxyPassword || '',
    });

    if (!response.success) return null;

    const html = response.data;

    // 简单提取纯文本（移除 HTML 标签）
    const text = html
      .replace(/<script[^>]*>[\s\S]*?<\/script>/gi, '')
      .replace(/<style[^>]*>[\s\S]*?<\/style>/gi, '')
      .replace(/<[^>]+>/g, ' ')
      .replace(/\s+/g, ' ')
      .trim()
      .slice(0, 5000); // 限制长度

    return { html, text };
  } catch (error) {
    console.error(`Failed to fetch ${url}:`, error);
    return null;
  }
}

/**
 * 深度优先游走
 */
async function dfsWalk(
  config: GideonConfig,
  startLinks: LinkScore[],
  depth: number,
  callbacks: WalkCallbacks,
  state: WalkState
): Promise<void> {
  if (depth >= config.maxDepth || !state.isWalking) {
    return;
  }

  // 评分所有链接
  callbacks.onLog?.(`🔍 正在评估 ${startLinks.length} 个链接...`);
  const scoredLinks = await scoreLinks(config, startLinks);

  // 检测 API 是否正常工作
  const allHalfScores = scoredLinks.slice(0, 3).every(l => l.score === 0.5);
  if (allHalfScores) {
    callbacks.onLog?.('⚠️ Gemini API 可能未正常工作 (所有评分都是 0.5)');
    callbacks.onLog?.('   请检查 API Key 和模型名称是否正确');
  }

  // 选择下一个链接（包含突变机制）
  const nextLink = selectNextLink(scoredLinks, config.mutationRate);
  if (!nextLink) {
    callbacks.onLog?.('📭 没有可用的链接');
    return;
  }

  // 更新状态
  visitedUrls.add(nextLink.url);
  state.linksVisited++;
  state.currentUrl = nextLink.url;
  state.currentDepth = depth + 1;
  callbacks.onStateChange?.(state);

  const mutationLabel = nextLink.isMutation ? '🧬 [突变]' : '📍';
  callbacks.onLog?.(`${mutationLabel} 深入: ${nextLink.title.slice(0, 50)}...`);
  callbacks.onLog?.(`   📊 评分: ${nextLink.score.toFixed(2)}`);

  // 抓取页面内容
  const page = await fetchPage(nextLink.url, config);
  if (!page) {
    callbacks.onLog?.('⚠️ 无法获取页面内容');
    return;
  }

  // 捕获条件：评分 > 0.45 或者是突变链接
  // 如果评分全是 0.5 (API失败的默认值)，也会捕获一些内容
  const shouldCapture = nextLink.score > 0.45 || nextLink.isMutation;

  // 检查是否已经捕获过这个 URL（使用 localStorage 持久化，14天有效）
  if (isUrlCaptured(nextLink.url)) {
    callbacks.onLog?.(`⏭️ 跳过 (14天内已捕获): ${nextLink.title.slice(0, 30)}...`);
  } else if (shouldCapture) {
    callbacks.onLog?.('💡 生成洞察...');
    const insight = await generateInsight(config, page.text, nextLink.isMutation);

    const capture: GideonCapture = {
      logId: Date.now(),
      url: nextLink.url,
      title: nextLink.title,
      content: page.text.slice(0, 1000),
      insight,
      score: nextLink.score,
      isMutation: nextLink.isMutation,
      capturedAt: Date.now(),
    };

    // 添加到捕获记录（持久化到 localStorage，包含标题和洞察）
    addCapturedEntry(nextLink.url, nextLink.title, insight);

    state.captures.push(capture);
    callbacks.onCapture?.(capture);
    callbacks.onLog?.(`✅ 捕获: ${nextLink.title.slice(0, 40)}...`);
  } else {
    callbacks.onLog?.(`⏭️ 跳过 (评分 ${nextLink.score.toFixed(2)} < 0.45)`);
  }

  // 等待
  await new Promise(r => setTimeout(r, config.requestDelayMs));

  // 提取新链接并继续深入
  const newLinks = extractLinks(page.html, nextLink.url);
  if (newLinks.length > 0 && state.isWalking) {
    await dfsWalk(config, newLinks, depth + 1, callbacks, state);
  }
}

/**
 * 开始游走
 */
export async function startWalk(config: GideonConfig, callbacks: WalkCallbacks = {}): Promise<GideonCapture[]> {
  // 重置状态
  visitedUrls.clear();

  const state: WalkState = {
    isWalking: true,
    currentUrl: null,
    currentDepth: 0,
    linksVisited: 0,
    captures: [],
    startTime: Date.now(),
    seedSource: null,
  };

  callbacks.onStateChange?.(state);

  try {
    // 获取微信登录状态
    const weChatStatus = isWeChatLoggedIn() ? '已登录' : '未登录';

    callbacks.onLog?.(`🌙 Gideon 开始夜行`);
    callbacks.onLog?.(`📱 微信状态: ${weChatStatus}`);

    // 从 HN、GitHub、WeChat 各获取一条种子
    callbacks.onLog?.(`🌱 正在从多个源获取种子...`);
    const seedLinks = await fetchAllSeeds();

    if (seedLinks.length === 0) {
      throw new Error('所有种子源都无法获取链接，请检查网络或代理设置');
    }

    // 记录种子源信息
    state.seedSource = 'hacker-news'; // 主源标记
    callbacks.onLog?.(`📦 获取到 ${seedLinks.length} 个种子入口`);

    // 策略：先访问并捕获所有种子入口（宽度优先），然后再深度探索
    // 这确保了每个源（HN、GitHub、WeChat）都有代表性内容

    const allDiscoveredLinks: LinkScore[] = [];

    // 第一阶段：处理所有种子入口
    callbacks.onLog?.(`📍 第一阶段：访问所有种子入口...`);
    for (const seed of seedLinks) {
      if (!state.isWalking) break;

      // 直接访问种子页面（不评分，直接捕获）
      visitedUrls.add(seed.url);
      state.linksVisited++;
      state.currentUrl = seed.url;
      callbacks.onStateChange?.(state);

      callbacks.onLog?.(`📍 访问种子: ${seed.title.slice(0, 50)}...`);

      const page = await fetchPage(seed.url, config);
      if (!page) {
        callbacks.onLog?.('⚠️ 无法获取页面内容');
        continue;
      }

      // 直接捕获种子入口（如果未捕获过）
      if (!isUrlCaptured(seed.url)) {
        callbacks.onLog?.('💡 生成洞察...');
        const insight = await generateInsight(config, page.text, false);

        const capture: GideonCapture = {
          logId: Date.now(),
          url: seed.url,
          title: seed.title,
          content: page.text.slice(0, 1000),
          insight,
          score: 1.0, // 种子入口默认高分
          isMutation: false,
          capturedAt: Date.now(),
        };

        addCapturedEntry(seed.url, seed.title, insight);
        state.captures.push(capture);
        callbacks.onCapture?.(capture);
        callbacks.onLog?.(`✅ 捕获种子: ${seed.title.slice(0, 40)}...`);
      } else {
        callbacks.onLog?.(`⏭️ 跳过种子 (14天内已捕获): ${seed.title.slice(0, 30)}...`);
      }

      // 收集该页面的链接用于后续深度探索
      const newLinks = extractLinks(page.html, seed.url);
      allDiscoveredLinks.push(...newLinks);

      // 等待
      await new Promise(r => setTimeout(r, config.requestDelayMs));
    }

    // 第二阶段：深度探索（如果还有深度余量）
    if (config.maxDepth > 1 && allDiscoveredLinks.length > 0 && state.isWalking) {
      callbacks.onLog?.(`🔍 第二阶段：深度探索发现的链接...`);
      await dfsWalk(config, allDiscoveredLinks.slice(0, 10), 1, callbacks, state);
    }

    callbacks.onLog?.(`🌅 夜行结束`);
    callbacks.onLog?.(`📊 统计: 访问 ${state.linksVisited} 个链接, 捕获 ${state.captures.length} 条内容`);

    callbacks.onComplete?.(state.captures);
    return state.captures;
  } catch (error: any) {
    callbacks.onError?.(error);
    callbacks.onLog?.(`❌ 错误: ${error.message}`);
    return state.captures;
  }
}

/**
 * 停止游走
 */
export function stopWalk(state: WalkState): void {
  state.isWalking = false;
}
