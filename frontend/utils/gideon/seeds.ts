/**
 * Gideon 种子源模块
 * 提供游走的起始点
 */

import { getAccountList } from '~/apis';
import type { LinkScore, SeedConfig, SeedSource } from '~/types/gideon';
import { rustBackendGet } from '~/utils/rustBackendApi';

/** 种子源结果 */
export interface SeedResult {
  source: SeedSource;
  links: LinkScore[];
}

/**
 * 检查微信登录是否有效
 */
export function isWeChatLoggedIn(): boolean {
  try {
    const loginData = localStorage.getItem('login');
    if (!loginData) return false;
    const account = JSON.parse(loginData);
    return account && account.nickname;
  } catch {
    return false;
  }
}

/**
 * 获取动态种子配置
 * 根据微信登录状态调整权重
 */
export function getDynamicSeedConfig(): SeedConfig[] {
  const isLoggedIn = isWeChatLoggedIn();

  if (isLoggedIn) {
    // 微信已登录: HN 3 : GitHub 2 : Wikipedia 1 : WeChat 4
    // 总和 = 10
    return [
      { source: 'hacker-news', weight: 0.3, enabled: true },
      { source: 'github-trending', weight: 0.2, enabled: true },
      { source: 'wikipedia-random', weight: 0.1, enabled: true },
      { source: 'wechat', weight: 0.4, enabled: true },
    ];
  } else {
    // 微信未登录: HN 5 : GitHub 3 : Wikipedia 2
    // 总和 = 10
    return [
      { source: 'hacker-news', weight: 0.5, enabled: true },
      { source: 'github-trending', weight: 0.3, enabled: true },
      { source: 'wikipedia-random', weight: 0.2, enabled: true },
      { source: 'wechat', weight: 0, enabled: false },
    ];
  }
}

/**
 * 获取 Hacker News 最新帖子
 */
export async function fetchHackerNewsSeeds(): Promise<LinkScore[]> {
  try {
    // 获取最新故事 ID 列表
    const response = await rustBackendGet<{ success: boolean; data: number[] }>('/api/gideon/fetch', {
      url: 'https://hacker-news.firebaseio.com/v0/newstories.json',
    });

    if (!response.success || !Array.isArray(response.data)) {
      console.warn('HN API response invalid');
      return [];
    }

    // 只取前 30 个
    const storyIds = response.data.slice(0, 30);

    // 获取每个故事的详情
    const stories = await Promise.all(
      storyIds.slice(0, 10).map(async id => {
        try {
          const storyResp = await rustBackendGet<{ success: boolean; data: any }>('/api/gideon/fetch', {
            url: `https://hacker-news.firebaseio.com/v0/item/${id}.json`,
          });
          return storyResp.success ? storyResp.data : null;
        } catch {
          return null;
        }
      })
    );

    return stories
      .filter((s): s is NonNullable<typeof s> => s !== null && s.url)
      .map(story => ({
        url: story.url,
        title: story.title || 'Untitled',
        context: `HN Score: ${story.score}, Comments: ${story.descendants || 0}`,
        score: 0, // 待评分
        isMutation: false,
      }));
  } catch (error) {
    console.error('Failed to fetch HN seeds:', error);
    return [];
  }
}

/**
 * 获取 GitHub Trending 仓库
 */
export async function fetchGitHubTrendingSeeds(): Promise<LinkScore[]> {
  try {
    // 使用 GitHub API 搜索最近创建的热门仓库
    const response = await rustBackendGet<{ success: boolean; data: any }>('/api/gideon/fetch', {
      url: 'https://api.github.com/search/repositories?q=created:>2024-12-01&sort=stars&order=desc&per_page=20',
    });

    if (!response.success || !response.data?.items) {
      console.warn('GitHub API response invalid');
      return [];
    }

    return response.data.items.map((repo: any) => ({
      url: repo.html_url,
      title: `${repo.full_name} - ${repo.description || 'No description'}`,
      context: `Stars: ${repo.stargazers_count}, Language: ${repo.language || 'Unknown'}`,
      score: 0,
      isMutation: false,
    }));
  } catch (error) {
    console.error('Failed to fetch GitHub seeds:', error);
    return [];
  }
}

/**
 * 获取 Wikipedia 随机文章
 */
export async function fetchWikipediaRandomSeeds(): Promise<LinkScore[]> {
  const results: LinkScore[] = [];

  try {
    // 获取多篇随机文章
    for (let i = 0; i < 5; i++) {
      const response = await rustBackendGet<{ success: boolean; data: any }>('/api/gideon/fetch', {
        url: 'https://en.wikipedia.org/api/rest_v1/page/random/summary',
      });

      if (response.success && response.data) {
        results.push({
          url: response.data.content_urls?.desktop?.page || `https://en.wikipedia.org/wiki/${response.data.title}`,
          title: response.data.title,
          context: response.data.extract?.slice(0, 200) || 'No extract available',
          score: 0,
          isMutation: true, // Wikipedia 总是作为突变源
        });
      }

      // 添加小延迟避免过快请求
      await new Promise(r => setTimeout(r, 500));
    }
  } catch (error) {
    console.error('Failed to fetch Wikipedia seeds:', error);
  }

  return results;
}

/**
 * 获取微信公众号文章作为种子源
 * 先搜索公众号，再获取其文章列表
 */
export async function fetchWeChatSeeds(): Promise<LinkScore[]> {
  const results: LinkScore[] = [];

  // 多样化关键词列表 - 降低 AI 频率，增加其他有趣主题
  const keywords = [
    // AI/科技 (20%)
    'AI',
    '大模型',
    // 创业/产品 (15%)
    '创业',
    '独立开发',
    '产品',
    // 金融/投资 (15%)
    '投资',
    '金融',
    '商业',
    // 认知/思维 (15%)
    '认知',
    '思维模型',
    '心理学',
    // 历史/文化 (15%)
    '历史',
    '文化',
    '艺术',
    // 科学/自然 (10%)
    '科学',
    '物理',
    '生物',
    // 哲学/人文 (10%)
    '哲学',
    '社会',
    '人生',
  ];

  try {
    // 先检查登录状态
    if (!isWeChatLoggedIn()) {
      console.log('[Gideon] WeChat 未登录，跳过微信种子源');
      return results;
    }

    // 随机选择一个关键词搜索公众号
    const keyword = keywords[Math.floor(Math.random() * keywords.length)];
    console.log(`[Gideon] WeChat 搜索关键词: ${keyword}`);

    const [accounts, _] = await getAccountList(0, keyword);

    if (!accounts || accounts.length === 0) {
      console.log('[Gideon] WeChat 搜索结果为空，可能是关键词问题或 session 问题');
      return results;
    }

    console.log(`[Gideon] 找到 ${accounts.length} 个公众号`);

    // 获取前 2 个公众号的文章
    for (const account of accounts.slice(0, 2)) {
      try {
        // 构造一个简单的 Info 对象用于获取文章
        const infoLike = {
          fakeid: account.fakeid,
          nickname: account.nickname,
        } as any;

        // 获取该公众号的最新文章
        const { getArticleList } = await import('~/apis');
        const [articles, _, total] = await getArticleList(infoLike, 0, '');

        console.log(`[Gideon] 公众号 "${account.nickname}" 有 ${articles.length} 篇文章`);

        // 取前 3 篇文章
        for (const article of articles.slice(0, 3)) {
          if (article.link) {
            results.push({
              url: article.link,
              title: article.title || 'Untitled Article',
              context: `微信公众号: ${account.nickname} | ${keyword}`,
              score: 0,
              isMutation: false,
            });
          }
        }

        // 添加延迟避免请求过快
        await new Promise(r => setTimeout(r, 1000));
      } catch (innerError: any) {
        console.warn(`[Gideon] 获取公众号 "${account.nickname}" 文章失败:`, innerError.message);
      }
    }
  } catch (error: any) {
    if (error.message === 'session expired') {
      console.warn('[Gideon] WeChat session expired, skipping WeChat seeds');
    } else {
      console.error('[Gideon] Failed to fetch WeChat seeds:', error);
    }
  }

  console.log(`[Gideon] WeChat 种子源返回 ${results.length} 篇文章`);
  return results;
}

/**
 * 根据权重选择种子源
 */
export function selectSeedSource(seeds: SeedConfig[]): SeedSource {
  const enabledSeeds = seeds.filter(s => s.enabled && s.weight > 0);
  if (enabledSeeds.length === 0) {
    return 'hacker-news'; // 默认回退
  }

  const totalWeight = enabledSeeds.reduce((sum, s) => sum + s.weight, 0);
  let random = Math.random() * totalWeight;

  for (const seed of enabledSeeds) {
    random -= seed.weight;
    if (random <= 0) {
      return seed.source;
    }
  }

  return enabledSeeds[0].source;
}

/**
 * 获取种子链接
 */
export async function fetchSeeds(source: SeedSource): Promise<SeedResult> {
  let links: LinkScore[] = [];

  switch (source) {
    case 'hacker-news':
      links = await fetchHackerNewsSeeds();
      break;
    case 'github-trending':
      links = await fetchGitHubTrendingSeeds();
      break;
    case 'wechat':
      links = await fetchWeChatSeeds();
      break;
    case 'continue-last':
      // TODO: 从上次游走终点继续
      console.log('Continue-last seed source not yet implemented');
      break;
  }

  return { source, links };
}

/**
 * 从所有源获取种子（每个源取一条）
 *
 * 不再随机选源，而是从 HN、GitHub、WeChat 各取一条种子
 */
export async function fetchAllSeeds(): Promise<LinkScore[]> {
  const allSeeds: LinkScore[] = [];
  const isLoggedIn = isWeChatLoggedIn();

  console.log('[Gideon] 开始从多个源获取种子...');

  // 1. 从 Hacker News 获取一条
  try {
    const hnSeeds = await fetchHackerNewsSeeds();
    if (hnSeeds.length > 0) {
      // 随机取一条
      const randomHn = hnSeeds[Math.floor(Math.random() * hnSeeds.length)];
      allSeeds.push(randomHn);
      console.log(`[Gideon] HN: ${randomHn.title.slice(0, 40)}...`);
    }
  } catch (e) {
    console.warn('[Gideon] HN 获取失败');
  }

  // 2. 从 GitHub 获取一条
  try {
    const ghSeeds = await fetchGitHubTrendingSeeds();
    if (ghSeeds.length > 0) {
      const randomGh = ghSeeds[Math.floor(Math.random() * ghSeeds.length)];
      allSeeds.push(randomGh);
      console.log(`[Gideon] GitHub: ${randomGh.title.slice(0, 40)}...`);
    }
  } catch (e) {
    console.warn('[Gideon] GitHub 获取失败');
  }

  // 3. 从微信获取一条（如果已登录）
  if (isLoggedIn) {
    try {
      const wxSeeds = await fetchWeChatSeeds();
      if (wxSeeds.length > 0) {
        const randomWx = wxSeeds[Math.floor(Math.random() * wxSeeds.length)];
        allSeeds.push(randomWx);
        console.log(`[Gideon] WeChat: ${randomWx.title.slice(0, 40)}...`);
      }
    } catch (e) {
      console.warn('[Gideon] WeChat 获取失败');
    }
  }

  console.log(`[Gideon] 共获取 ${allSeeds.length} 个种子入口`);
  return allSeeds;
}
