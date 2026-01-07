# 编译层
FROM node:22-alpine AS build-env

# 安装 Yarn (pin a specific Yarn version)
RUN corepack enable
RUN corepack prepare yarn@1.22.22 --activate

# 设置工作目录
WORKDIR /app

# 复制 package.json 和 lock 文件，安装依赖
COPY package.json yarn.lock ./
RUN yarn install --frozen-lockfile && yarn cache clean

# 复制源代码
COPY . .

# 构建 Nuxt 应用（生成 .output 目录）
ENV NODE_ENV=production \
    NITRO_KV_DRIVER=fs \
    NITRO_KV_BASE=.data/kv

RUN yarn build

# 运行时层
FROM node:22-alpine

ARG VERSION=unknown

# 添加 LABEL 元数据
LABEL maintainer="findsource@proton.me" \
    version="${VERSION}" \
    description="WeChat Article Insight Frontend Docker Image" \
    org.opencontainers.image.source="https://github.com/longlongyang/wechat-article-insight" \
    org.opencontainers.image.description="AI 驱动的微信公众号文章智能发现工具，支持下载阅读量与评论数据，支持私有化部署" \
    org.opencontainers.image.licenses="MIT"

# 设置工作目录
WORKDIR /app

# 复制构建输出
COPY --from=build-env /app/.output ./

# 创建 KV 存储目录并设置权限（以 root 运行，确保 node 用户可写）
RUN mkdir -p .data/kv && chown -R node:node /app

# 创建非 root 用户（使用内置 node 用户）
USER node

# 暴露端口
EXPOSE 3000

# 设置环境变量：生产模式，监听所有接口
ENV NODE_ENV=production HOST=0.0.0.0 PORT=3000

# 启动命令：运行 Nitro 生成的服务器
ENTRYPOINT ["node", "server/index.mjs"]
