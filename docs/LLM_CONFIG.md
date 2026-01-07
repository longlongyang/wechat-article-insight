# 全网洞察 (Network Insight) - LLM 配置指南

## 概述

全网洞察功能支持灵活配置不同的 LLM 服务商，可根据需求选择云端 (Gemini/DeepSeek) 或本地 (Ollama) 模型。

---

## ⚠️ 重要注意事项

### 1. 数据库维度兼容性

> **不同 Embedding 模型输出的向量维度不同，切换后需要重建向量表！**

| 模型 | 维度 | 说明 |
|------|------|------|
| Gemini `gemini-embedding-001` | **768 / 1536 / 3072** | 默认推荐，支持灵活维度 |
| Ollama `qwen3-embedding:8b-q8_0` | **4096** | 本地运行，隐私性更好 |

**切换前必须执行：**
```sql
-- 连接到 PostgreSQL 数据库
DROP TABLE embeddings;
-- 重启后端服务，会自动用新维度重建表
```

### 2. API Key 配置

在 `.env` 文件中配置（至少需要一个）：

```env
# Gemini (推荐 - 默认使用)
GEMINI_API_KEY=AIza...

# DeepSeek (可选 - 用于关键词生成)
DEEPSEEK_API_KEY=sk-...

# Embedding 维度 (切换模型后必须修改)
# Embedding 维度 (切换模型后必须修改)
# Gemini gemini-embedding-001 支持: 768, 1536, 3072 (推荐 768)
# Ollama qwen3-embedding:8b-q8_0 固定: 4096
EMBEDDING_DIMENSION=768   # Gemini 推荐
# EMBEDDING_DIMENSION=4096  # 如使用 Ollama
```

### 3. Ollama 本地模型

如需使用本地 Embedding，请确保：

```bash
# 1. 安装 Ollama (https://ollama.com)
# 2. 拉取模型
ollama pull qwen3-embedding:8b-q8_0

# 3. 确保 Ollama 服务运行
ollama serve
```

---

## 配置说明

### 前端 AI 配置页 (`/dashboard/ai`)

1. **Gemini 配置** - API Key + 模型选择 + 代理开关
   > **Docker 用户注意**：代理地址请填写 `host.docker.internal` 而非 `127.0.0.1`
2. **DeepSeek 配置** - API Key + 模型选择
3. **Ollama 配置** - 启用开关 + 地址 + 模型选择

### 创建洞察任务时的高级配置

展开"高级配置"可选择：

| 环节 | 可选 Provider | 默认值 |
|------|---------------|--------|
| 关键词生成 | Gemini / DeepSeek | Gemini |
| 文章筛选 | Gemini / DeepSeek | Gemini |
| Embedding | Gemini / Ollama | Gemini |

---

## 环境变量汇总

| 变量 | 必填 | 默认值 | 说明 |
|------|------|--------|------|
| `DATABASE_URL` | ✅ | - | PostgreSQL 连接字符串 |
| `GEMINI_API_KEY` | ✅ | - | Google AI Studio API Key |
| `DEEPSEEK_API_KEY` | ❌ | - | DeepSeek Platform API Key |
| `EMBEDDING_DIMENSION` | ❌ | `768` | 向量维度 (768/4096) |
| `RUST_LOG` | ❌ | `info` | 日志级别 |

---

## 常见问题

### Q: 为什么推荐使用 Gemini 而不是 Ollama？

A: Gemini `text-embedding-004` 免费配额充足，且 768 维度向量更小、索引更快。Ollama 适合对隐私有严格要求的场景。

### Q: 切换 Embedding Provider 后报错怎么办？

A: 需要清空 embeddings 表并修改 `EMBEDDING_DIMENSION` 环境变量后重启后端。

### Q: 任务运行时可以切换 Provider 吗？

A: 不可以。每个任务创建时会确定使用的 Provider，运行中不会改变。

---

## 文件结构

```
backend/src/
├── llm/
│   ├── mod.rs           # LLM 抽象层
│   ├── gemini.rs        # Gemini 实现
│   ├── deepseek.rs      # DeepSeek 实现
│   └── ollama.rs        # Ollama 实现
├── api/
│   ├── insight.rs       # 洞察任务逻辑
│   └── embedding.rs     # Embedding API
└── db.rs                # 数据库初始化
```
