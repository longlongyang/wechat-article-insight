# PostgreSQL + pgvector 安装指南

## Windows 安装步骤

### 1. 下载 PostgreSQL

```powershell
# 使用 winget 安装
winget install PostgreSQL.PostgreSQL.16

# 或从官网下载: https://www.postgresql.org/download/windows/
```

### 2. 安装 pgvector 扩展

```powershell
# 克隆 pgvector
git clone https://github.com/pgvector/pgvector.git
cd pgvector

# 编译安装 (需要 Visual Studio Build Tools)
# 设置 PostgreSQL 路径
set "PGROOT=C:\Program Files\PostgreSQL\16"
nmake /F Makefile.win
nmake /F Makefile.win install
```

> **简化方式**: 直接使用 Docker:
> ```bash
> docker run -d --name postgres-pgvector \
>   -e POSTGRES_PASSWORD=postgres \
>   -p 5432:5432 \
>   pgvector/pgvector:pg16
> ```

### 3. 创建数据库

```sql
-- 连接 PostgreSQL
psql -U postgres

-- 创建数据库
CREATE DATABASE wechat_insights;

-- 连接到新数据库
\c wechat_insights

-- 启用 pgvector 扩展
CREATE EXTENSION vector;
```

### 4. 配置环境变量

创建 `.env` 文件:
```env
DATABASE_URL=postgres://postgres:postgres@localhost:5432/wechat_insights
```

### 5. 运行后端

```bash
cd backend
cargo run --release
```

---

## 验证安装

```sql
-- 检查 pgvector 版本
SELECT extversion FROM pg_extension WHERE extname = 'vector';

-- 检查表是否创建
\dt

-- 查看 embedding 数量
SELECT COUNT(*) FROM embeddings;
```

## 性能对比

| 操作 | SQLite (内存搜索) | PostgreSQL + pgvector |
|------|------------------|----------------------|
| 10万条搜索 | 2-5 秒 | 10-50 毫秒 |
| 索引类型 | 无 | IVFFlat |
| 复杂度 | O(N) | O(log N) |
