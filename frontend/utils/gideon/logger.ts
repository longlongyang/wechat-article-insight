/**
 * Gideon 报告生成器
 * 生成诗意的夜行报告
 */

import type { GideonCapture, GideonConfig, SeedSource } from '~/types/gideon';

/** 报告数据 */
export interface GideonReport {
  date: string;
  time: string;
  summary: string;
  captures: GideonCapture[];
  mutationDiscovery: GideonCapture | null;
  markdown: string;
}

/**
 * 格式化时间戳
 */
function formatTimestamp(timestamp: number): string {
  const date = new Date(timestamp);
  return date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
}

/**
 * 格式化日期
 */
function formatDate(timestamp: number): string {
  const date = new Date(timestamp);
  return date
    .toLocaleDateString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
    })
    .replace(/\//g, '-');
}

/**
 * 生成报告开头语（诗意化）
 */
function generateOpening(seedSource: SeedSource, captures: GideonCapture[]): string {
  const openings: Record<string, string[]> = {
    'hacker-news': [
      '昨晚我在 Hacker News 的比特洪流中穿行，',
      '今夜，极客们的窃窃私语引导着我的脚步，',
      '橙色的光芒中，我看到了一些有趣的信号，',
    ],
    'github-trending': [
      '代码的星光指引着我，在 GitHub 的星图中漫步，',
      '那些新诞生的仓库，就像宇宙中刚点亮的恒星，',
      '今夜我在开源的海洋里寻找独特的贝壳，',
    ],
    'wikipedia-random': [
      '知识的迷宫里，我随机打开了几扇门，',
      '百科全书的随机性就像记忆深处浮现的碎片，',
      '今晚的突变发现让我想起了一些遥远的事物，',
    ],
    wechat: ['在中文互联网的某个角落，我发现了一些声音，', '微信公众号里，隐藏着一些值得注意的信息，'],
    'continue-last': ['接续昨夜的梦境，我继续深入探索，', '从上次停下的地方，我重新开始游走，'],
  };

  const sourceOpenings = openings[seedSource] || openings['hacker-news'];
  const randomOpening = sourceOpenings[Math.floor(Math.random() * sourceOpenings.length)];

  const mutationCapture = captures.find(c => c.isMutation);
  if (mutationCapture) {
    return `${randomOpening}\n\n不过最让我着迷的是一次意外的发现...`;
  }

  return randomOpening;
}

/**
 * 生成 Markdown 报告
 */
export function generateReport(
  captures: GideonCapture[],
  seedSource: SeedSource,
  startTime: number,
  linksVisited: number
): GideonReport {
  const mutationDiscovery = captures.find(c => c.isMutation) || null;
  const regularCaptures = captures.filter(c => !c.isMutation);

  const opening = generateOpening(seedSource, captures);

  let markdown = `**From: Gideon**
**Time: ${formatTimestamp(Date.now())}**

${opening}

`;

  if (regularCaptures.length > 0) {
    markdown += `**The Capture:**
`;
    for (const capture of regularCaptures) {
      markdown += `- 🔗 [${capture.title.slice(0, 60)}${capture.title.length > 60 ? '...' : ''}](${capture.url})
  💡 ${capture.insight}

`;
    }
  }

  if (mutationDiscovery) {
    markdown += `**Mutation Discovery:**
我本来在找常规的东西，结果被拉进了一个关于 **${mutationDiscovery.title}** 的兔子洞。

${mutationDiscovery.insight}

这让我想到，或许混乱中总有一些必然。

`;
  }

  markdown += `---
*访问了 ${linksVisited} 个链接，捕获了 ${captures.length} 条内容*
`;

  return {
    date: formatDate(startTime),
    time: formatTimestamp(startTime),
    summary: `游走 ${linksVisited} 链接，捕获 ${captures.length} 条`,
    captures,
    mutationDiscovery,
    markdown,
  };
}

/**
 * 下载报告为 Markdown 文件
 */
export function downloadReport(report: GideonReport): void {
  const blob = new Blob([report.markdown], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `Gideon_${report.date}.md`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
