/**
 * Gideon Gemini 评分器
 * 使用 Gemini API 评估链接质量
 */

import type { LLMConfig } from '~/composables/useLLMConfig';
import type { GideonConfig, LinkScore } from '~/types/gideon';

/** 从 localStorage 获取共享的 LLM 配置 */
function getSharedLLMConfig(): LLMConfig {
  if (typeof localStorage === 'undefined') {
    return {
      provider: 'gemini',
      geminiApiKey: '',
      geminiModel: 'gemini-2.5-flash',
      deepseekApiKey: '',
      deepseekModel: 'deepseek-chat',
    };
  }

  try {
    const stored = localStorage.getItem('llm-config');
    if (stored) {
      return JSON.parse(stored);
    }
  } catch (e) {
    console.warn('[Gideon] Failed to load LLM config:', e);
  }

  return {
    provider: 'gemini',
    geminiApiKey: '',
    geminiModel: 'gemini-2.5-flash',
    deepseekApiKey: '',
    deepseekModel: 'deepseek-chat',
  };
}

/** 评分 Prompt */
const SCORING_PROMPT = `You are a link quality scorer for a curious explorer of ideas.

Rate this link from 0 to 1 based on its potential for insight and inspiration:

HIGH VALUE (0.8-1.0):
- Unique perspectives on any topic (AI, history, philosophy, science)
- Deep analysis or unconventional thinking
- Stories that reveal hidden patterns or connections
- Thought-provoking ideas that challenge assumptions

MEDIUM VALUE (0.4-0.7):
- Educational content with substance
- Industry insights and trends
- Scientific discoveries or research
- Cultural observations or historical analysis

LOW VALUE (0.0-0.3):
- Celebrity gossip, entertainment fluff
- Generic marketing or advertisements
- Clickbait without substance
- Shallow listicles or summaries

URL: {url}
Title: {title}
Context: {context}

Respond with ONLY a single number between 0.0 and 1.0, nothing else.`;

/** 最后一次 API 错误 */
let lastApiError: string | null = null;

/**
 * 获取最后一次 API 错误
 */
export function getLastApiError(): string | null {
  return lastApiError;
}

/**
 * 调用 Gemini API
 * 注意: Gemini 3 的 thinkingTokens 也计入 maxOutputTokens
 * 所以即使只需要输出一个数字，也需要足够的 tokens 用于思考
 */
async function callGemini(prompt: string, maxTokens: number = 200): Promise<string> {
  const llmConfig = getSharedLLMConfig();
  const modelName = llmConfig.geminiModel;
  const url = `https://generativelanguage.googleapis.com/v1beta/models/${modelName}:generateContent?key=${llmConfig.geminiApiKey}`;

  console.log(`[Gideon] Calling Gemini API: ${modelName}`);
  console.log(`[Gideon] API URL: ${url.replace(llmConfig.geminiApiKey, 'API_KEY_HIDDEN')}`);

  // 检测是否是 Gemini 3 模型
  const isGemini3 = modelName.includes('gemini-3');

  // 构建请求体
  const requestBody: any = {
    contents: [
      {
        parts: [{ text: prompt }],
      },
    ],
    generationConfig: {
      temperature: 0.1,
      maxOutputTokens: maxTokens,
    },
  };

  // Gemini 3 需要 thinkingConfig
  // Pro 只支持 low/high, Flash 支持 minimal/low/medium/high
  // minimal 几乎不用思考 token，对于简单评分任务更合适
  if (isGemini3) {
    const isFlash = modelName.includes('flash');
    const thinkingLevel = isFlash ? 'minimal' : 'low';
    requestBody.generationConfig.thinkingConfig = {
      thinkingLevel: thinkingLevel,
    };
    console.log(`[Gideon] Using Gemini 3 with thinkingLevel: ${thinkingLevel}`);

    // Pro 模型即使 low 也用 ~100 tokens 思考，所以需要更多 token
    if (!isFlash && maxTokens < 300) {
      requestBody.generationConfig.maxOutputTokens = 300;
      console.log('[Gideon] Increased maxOutputTokens to 300 for Pro model');
    }
  }

  console.log('[Gideon] Request body:', JSON.stringify(requestBody, null, 2).slice(0, 500));

  const startTime = Date.now();

  try {
    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(requestBody),
    });

    const elapsed = Date.now() - startTime;
    console.log(`[Gideon] API response received in ${elapsed}ms, status: ${response.status}`);

    if (!response.ok) {
      const errorText = await response.text();
      lastApiError = `HTTP ${response.status}: ${errorText.slice(0, 500)}`;
      console.error(`[Gideon] Gemini API error:`, lastApiError);
      throw new Error(lastApiError);
    }

    const data = await response.json();
    console.log('[Gideon] Response data:', JSON.stringify(data).slice(0, 500));

    // 检查 API 响应结构
    if (!data.candidates || data.candidates.length === 0) {
      lastApiError = 'No candidates in response: ' + JSON.stringify(data).slice(0, 300);
      console.error('[Gideon] Gemini response error:', lastApiError);
      throw new Error(lastApiError);
    }

    const candidate = data.candidates[0];
    const text = candidate?.content?.parts?.[0]?.text;

    if (!text) {
      // 检查是否被过滤
      if (candidate?.finishReason === 'SAFETY') {
        lastApiError = 'Response filtered by safety settings';
      } else {
        lastApiError = 'No text in response: ' + JSON.stringify(candidate).slice(0, 300);
      }
      console.error('[Gideon] Gemini response structure:', lastApiError);
      throw new Error(lastApiError);
    }

    console.log(`[Gideon] Gemini response text: "${text.trim().slice(0, 100)}"`);
    lastApiError = null;
    return text;
  } catch (fetchError: any) {
    const elapsed = Date.now() - startTime;
    console.error(`[Gideon] Fetch error after ${elapsed}ms:`, fetchError.message);

    // 如果是网络错误，可能需要代理
    if (fetchError.message.includes('fetch') || fetchError.message.includes('network')) {
      lastApiError = `Network error: ${fetchError.message}. 可能需要配置代理来访问 Google API`;
    } else {
      lastApiError = fetchError.message;
    }
    throw new Error(lastApiError);
  }
}

/**
 * Call DeepSeek API (OpenAI Compatible)
 */
async function callDeepSeek(prompt: string, maxTokens: number = 200): Promise<string> {
  const llmConfig = getSharedLLMConfig();
  const apiKey = llmConfig.deepseekApiKey;
  const model = llmConfig.deepseekModel;
  const url = 'https://api.deepseek.com/chat/completions';

  console.log(`[Gideon] Calling DeepSeek API: ${model}`);

  const requestBody = {
    model: model,
    messages: [{ role: 'user', content: prompt }],
    max_tokens: maxTokens,
    temperature: 0.1,
    stream: false,
  };

  console.log('[Gideon] DeepSeek Request:', JSON.stringify(requestBody).slice(0, 300));

  const startTime = Date.now();

  try {
    const response = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`,
      },
      body: JSON.stringify(requestBody),
    });

    const elapsed = Date.now() - startTime;
    console.log(`[Gideon] API response received in ${elapsed}ms, status: ${response.status}`);

    if (!response.ok) {
      const errorText = await response.text();
      lastApiError = `DeepSeek HTTP ${response.status}: ${errorText.slice(0, 500)}`;
      console.error(`[Gideon] DeepSeek API error:`, lastApiError);
      throw new Error(lastApiError);
    }

    const data = await response.json();
    const content = data.choices?.[0]?.message?.content;

    if (!content) {
      lastApiError = 'No content in DeepSeek response: ' + JSON.stringify(data).slice(0, 300);
      throw new Error(lastApiError);
    }

    console.log(`[Gideon] DeepSeek response: "${content.trim().slice(0, 100)}"`);
    lastApiError = null;
    return content;
  } catch (fetchError: any) {
    lastApiError = fetchError.message;
    throw new Error(lastApiError);
  }
}

/**
 * 为单个链接评分
 */
export async function scoreLink(config: GideonConfig, link: LinkScore): Promise<number> {
  const prompt = SCORING_PROMPT.replace('{url}', link.url)
    .replace('{title}', link.title)
    .replace('{context}', link.context);

  try {
    let result: string;
    const llmConfig = getSharedLLMConfig();

    if (llmConfig.provider === 'deepseek') {
      // DeepSeek V3 is cheap, we can use reasoner or chat
      result = await callDeepSeek(prompt, 100);
    } else {
      // Gemini 3 needs ~50-100 tokens for thinking, plus a few for the score
      result = await callGemini(prompt, 100);
    }

    const cleanResult = result.trim().replace(/[^0-9.]/g, '');
    const score = parseFloat(cleanResult);

    if (isNaN(score)) {
      console.warn(`[Gideon] Could not parse score from: "${result}"`);
      return 0.5;
    }

    console.log(`[Gideon] Scored "${link.title.slice(0, 30)}..." = ${score}`);
    return Math.min(1, Math.max(0, score));
  } catch (error: any) {
    console.error('[Gideon] Scoring failed:', error.message);
    return 0.5; // 失败时返回中等分数
  }
}

/**
 * 批量评分链接（只评分前3个以节省 API 调用）
 */
export async function scoreLinks(config: GideonConfig, links: LinkScore[]): Promise<LinkScore[]> {
  const scoredLinks: LinkScore[] = [];

  // 只评分前 3 个链接以节省 API 调用
  const linksToScore = links.slice(0, 3);
  const remainingLinks = links.slice(3);

  for (const link of linksToScore) {
    const score = await scoreLink(config, link);
    scoredLinks.push({ ...link, score });

    // 添加延迟避免 API 限流
    await new Promise(r => setTimeout(r, 500));
  }

  // 剩余链接给默认分数
  for (const link of remainingLinks) {
    scoredLinks.push({ ...link, score: 0.4 });
  }

  return scoredLinks;
}

/**
 * 根据评分和突变率选择下一个链接
 */
export function selectNextLink(links: LinkScore[], mutationRate: number): LinkScore | null {
  if (links.length === 0) return null;

  const random = Math.random();

  if (random < mutationRate) {
    // 突变路径：选择评分最低的链接
    const lowScoreLinks = links.filter(l => l.score < 0.4);
    if (lowScoreLinks.length > 0) {
      const selected = lowScoreLinks[Math.floor(Math.random() * lowScoreLinks.length)];
      return { ...selected, isMutation: true };
    }
  }

  // 理性路径：选择评分最高的链接
  const sortedLinks = [...links].sort((a, b) => b.score - a.score);
  return sortedLinks[0];
}

/**
 * 生成内容洞察
 */
export async function generateInsight(config: GideonConfig, content: string, isMutation: boolean): Promise<string> {
  const mutationNote = isMutation ? '(这是一个偶然发现的内容)' : '';

  const prompt = `分析以下内容，用中文写一个简洁的洞察（2-3句话）。

要求：
1. 直接说明这个内容的核心价值是什么
2. 指出它解决了什么问题，或者有什么实际应用场景
3. 如有独特之处，一针见血地指出

风格要求：
- 通俗易懂，不要华丽辞藻
- 直接揭示本质，像在和朋友解释一样
- 不要用"这很有趣因为..."、"这标志着..."这种套话开头
- 不要用比喻和隐喻，直接说事实

${mutationNote}

内容：
${content.slice(0, 2000)}

洞察：`;

  try {
    const llmConfig = getSharedLLMConfig();

    if (llmConfig.provider === 'deepseek') {
      const insight = await callDeepSeek(prompt, 500);
      return insight.trim();
    }

    // Gemini 3 Pro 需要更多 token 用于思考，增加到 500
    const insight = await callGemini(prompt, 500);
    return insight.trim();
  } catch (error: any) {
    console.error('[Gideon] Failed to generate insight:', error.message);
    return '无法生成洞察 - ' + (lastApiError || error.message);
  }
}
