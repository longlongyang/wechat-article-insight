/**
 * 向量搜索工具
 * 用于语义搜索的向量相似度计算
 */

// import { db } from '~/store/v2/db';
// import type { ArticleEmbedding, EmbeddingSource } from '~/store/v2/embedding';

export type EmbeddingSource = 'title' | 'content' | 'comment';

export interface ArticleEmbedding {
  id: string;
  fakeid: string;
  aid?: string;
  title: string;
  source: EmbeddingSource;
  textHash: string;
  vector: number[];
  indexedAt: number;
}

/**
 * 简单哈希函数（用于检测内容变化）
 */
export function simpleHash(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash; // Convert to 32bit integer
  }
  return hash.toString(16);
}

/**
 * 计算余弦相似度
 */
export function cosineSimilarity(a: number[], b: number[]): number {
  if (a.length !== b.length) return 0;

  let dotProduct = 0;
  let normA = 0;
  let normB = 0;

  for (let i = 0; i < a.length; i++) {
    dotProduct += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }

  const denominator = Math.sqrt(normA) * Math.sqrt(normB);
  return denominator === 0 ? 0 : dotProduct / denominator;
}

/**
 * 搜索结果
 */
export interface SearchResult {
  id: string;
  title: string;
  fakeid: string;
  source: EmbeddingSource;
  score: number;
}

/**
 * 向量搜索 - 本地已废弃，请使用后端API
 */
export async function vectorSearch(
  queryVector: number[],
  topK: number = 20,
  minScore: number = 0.5
): Promise<SearchResult[]> {
  console.warn('Local vector search is deprecated. Use backend API.');
  return [];
}

/**
 * 获取未索引的文章数量 - stub
 */
export async function getUnindexedCount(): Promise<number> {
  return 0;
}

/**
 * 获取索引统计 - stub
 */
export async function getIndexStats(): Promise<{
  total: number;
  indexed: number;
  unindexed: number;
  percentage: number;
}> {
  return { total: 0, indexed: 0, unindexed: 0, percentage: 0 };
}

/**
 * 检查文章是否已索引 - stub
 */
export async function isArticleIndexed(id: string): Promise<boolean> {
  return false;
}

/**
 * 保存文章 embedding - stub
 */
export async function saveEmbedding(embedding: ArticleEmbedding): Promise<void> {
  // no-op
}

/**
 * 批量保存 embedding - stub
 */
export async function saveEmbeddings(embeddings: ArticleEmbedding[]): Promise<void> {
  // no-op
}

/**
 * 删除文章 embedding - stub
 */
export async function deleteEmbedding(id: string): Promise<void> {
  // no-op
}

/**
 * 清空所有 embedding（重建索引时使用） - stub
 */
export async function clearAllEmbeddings(): Promise<void> {
  // no-op
}
