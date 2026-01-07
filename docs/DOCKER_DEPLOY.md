# Docker 部署指南

本指南详细说明如何使用 Docker Compose 部署 **WeChat Article Insight**，特别针对中国大陆地区的网络环境提供了加速配置建议。

## 1. 环境准备

确保已安装 [Docker Desktop](https://www.docker.com/products/docker-desktop/) (Windows/Mac) 或 Docker Engine (Linux)。

## 2. 🚀 中国大陆镜像加速 (重要)

由于 Docker Hub 在国内访问受限，强烈建议在启动前配置镜像加速器，否则可能会遇到 `connection refused` 或 `401 Unauthorized` 错误。

### 配置步骤 (Docker Desktop)

1.  打开 Docker Desktop 设置 (Settings)。
2.  进入 **Docker Engine** 选项卡。
3.  在 JSON 配置中添加 `registry-mirrors` 字段：

```json
{
  "builder": {
    "gc": {
      "defaultKeepStorage": "20GB",
      "enabled": true
    }
  },
  "experimental": false,
  "registry-mirrors": [
    "https://docker.m.daocloud.io",
    "https://docker.1panel.live",
    "https://docker.nju.edu.cn"
  ]
}
```

4.  点击 **Apply & restart** 等待重启完成。

## 3. 一键部署

在项目根目录下运行终端：

```powershell
# 1. 启动服务 (自动构建镜像)
docker compose up -d

# 2. 查看容器状态
docker compose ps
```

### 常用命令

- **停止服务**: `docker compose down`
- **查看日志**: `docker compose logs -f`
- **重新构建**: `docker compose up -d --build` (代码更新后使用)

## 4. 常见问题

### Q: 遇到 `lock file version 4` 错误?
这通常是因为之前的构建缓存了旧版 Rust 镜像。我们已升级到 `rust:1.83-alpine`。
**解决**: 运行 `docker compose up -d --build` 强制重新构建。

### Q: 遇到 `401 Unauthorized` 错误?
如果您在 Dockerfile 中硬编码了 `docker.m.daocloud.io` 前缀，可能会导致此错误。
**解决**: 请确保 Dockerfile 使用官方镜像名 (如 `alpine:latest`)，并只通过 Docker Desktop 的 `registry-mirrors` 全局设置来进行加速。

### Q: 容器无法连接网络?
确保没有配置错误的 HTTP_PROXY 环境变量。Docker 容器默认会继承系统的代理设置，但有时需要在 Docker Desktop 设置中的 **Resources > Proxies** 手动配置。

### Q: Gemini/OpenAI 连接超时?
如果您的后端容器无法连接 LLM API，需要在 `docker-compose.yml` 中配置代理。

1.  打开 `docker-compose.yml`
2.  找到 `backend` 服务的 `environment` 部分
3.  取消注释并修改 `HTTPS_PROXY`，指向宿主机的代理端口：
    ```yaml
    environment:
      # host.docker.internal 指向宿主机 IP
      HTTPS_PROXY: http://host.docker.internal:7890 
      # 排除不需要代理的域名 (直连更稳定：微信API、微信图片、DeepSeek、本地)
      NO_PROXY: localhost,127.0.0.1,mp.weixin.qq.com,weixin.qq.com,res.wx.qq.com,api.deepseek.com,mmbiz.qpic.cn
    ```

### Q: 页面上的 "API 测试连接" 失败 (Connection refused)?
如果您在 Docker 中运行，但页面上配置的代理地址是 `127.0.0.1`，会导致连接失败。
**解决**:
1. 进入系统设置 -> AI 模型 -> API 代理服务器
2. 将代理地址修改为 **`host.docker.internal`**
3. 再次点击测试连接

*(原理：`127.0.0.1` 在容器内指向容器自己，而您的代理运行在宿主机上)*

