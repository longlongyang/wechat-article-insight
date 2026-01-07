# 贡献指南

感谢您对本项目的兴趣！我们非常欢迎各种形式的贡献，包括但不限于代码、文档、Bug 报告和功能建议。🙌

## 行为准则

请在所有互动中保持友好和尊重。

## 开发环境搭建

### 克隆仓库

```shell
git clone https://github.com/longlongyang/wechat-article-insight.git
```

### 环境要求

| 组件 | 版本要求 |
|------|----------|
| Node.js | >= 22 |
| Rust | >= 1.75 |
| PostgreSQL | >= 15 |
| pgvector | >= 0.5 |

### 安装 PostgreSQL + pgvector

推荐使用 Docker：

```shell
docker run -d --name postgres-pgvector \
  -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 \
  pgvector/pgvector:pg16
```

创建数据库：

```sql
CREATE DATABASE wechat_insights;
\c wechat_insights
CREATE EXTENSION vector;
```

### 安装前端依赖

```shell
cd frontend
corepack enable
corepack prepare yarn@1.22.22 --activate
yarn install
```

### 配置后端

```shell
cd backend
cp .env.example .env
# 编辑 .env 填入 DATABASE_URL 和 GEMINI_API_KEY
```

### 本地运行

```shell
# 终端 1 - 启动后端
cd backend
cargo run

# 终端 2 - 启动前端
cd frontend
yarn dev
```

访问 http://localhost:3000

## 如何贡献

### 1. 报告 Bug 或建议功能
- 先搜索现有 [Issues](https://github.com/longlongyang/wechat-article-insight/issues)，避免重复。
- 如果没有找到，创建一个新 Issue。
    - Bug 报告请包含：复现步骤、预期行为、实际行为、环境信息（操作系统、浏览器、版本等）。
    - 功能建议请详细描述需求和使用场景。

### 2. 提交代码
请遵循以下流程：

1. Fork 本仓库。
2. 创建特性分支：`git checkout -b feature/你的功能描述` 或 `fix/你的修复描述`。
3. 安装开发依赖并运行项目（参考 开发环境搭建）。
4. 编写代码，确保：
    - 遵循项目代码风格（使用 Prettier 工具）。
    - 添加或更新测试用例。
    - 通过所有测试：`yarn test`（或你的测试命令）。
5. 提交时使用清晰的 Commit 消息。
6. Push 到你的 Fork 并打开 Pull Request。
    - PR 标题和描述要清晰，引用相关 Issue（如 `fixes #123`）。
    - 如果是重大更改，请先开 Issue 讨论。

### 3. 文档或翻译贡献
- 文档请修改 [docs](https://github.com/longlongyang/wechat-article-insight/docs) 项目
- 同样通过 Pull Request 提交

### 4. 代码风格指南
- 代码格式化采用 prettier
- 变量命名采用 camelCase
- import 顺序采用`yarn format`命令格式化
