# Cloudflare Worker Batch Deploy Tool

此工具帮助你快速部署多个 Cloudflare Worker 节点，用于搭建高可用代理池。

## 准备工作

1.  **Node.js**: 确保已安装 Node.js。
2.  **Cloudflare 账号**: 拥有一个 Cloudflare 账号。

## 使用步骤

### 1. 安装依赖

在当前目录下打开终端：

```bash
npm install wrangler --save-dev
```

### 2. 登录 Cloudflare

执行以下命令并按提示在浏览器中登录授权：

```bash
npx wrangler login
```

### 3. 开始批量部署

执行 `deploy.js` 脚本。

**语法**: `node deploy.js [名称前缀] [数量] [起始编号]`

**示例**：部署 10 个名为 `my-proxy-1` 到 `my-proxy-10` 的 Worker：

```bash
node deploy.js my-proxy 10 1
```

### 4. 获取结果

脚本运行结束后，会将所有部署成功的 Worker URL 打印在屏幕上，并保存在 `deployed_proxies.txt` 文件中。
你可以直接复制这些 URL 粘贴到 `系统设置` 的 `网络服务` 设置界面中。

## 注意事项

*   Cloudflare 免费版账号通常限制每天请求数（10万/天）和 Worker 数量（通常限制脚本数为 100 个）。
*   请勿滥用资源。
