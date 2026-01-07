# Ollama 安装指南

## 1. 安装 Ollama

### Windows
1. 访问 [ollama.com/download](https://ollama.com/download)
2. 下载 Windows 版本
3. 运行安装程序

### macOS
```bash
curl -fsSL https://ollama.com/install.sh | sh
```

### Linux
```bash
curl -fsSL https://ollama.com/install.sh | sh
```

## 2. 安装 Embedding 模型

打开终端/命令提示符，运行：

```bash
ollama pull nomic-embed-text
```

这会下载 nomic-embed-text 模型（约 274MB），专门用于文本 embedding，支持中英文。

## 3. 验证安装

```bash
# 测试 API 是否正常
curl http://localhost:11434/api/embeddings -d '{
  "model": "nomic-embed-text",
  "prompt": "测试文本"
}'
```

应该返回一个包含 768 维向量的 JSON 响应。

## 4. 启动服务

Ollama 安装后会自动作为服务运行。如果需要手动启动：

```bash
ollama serve
```

默认监听端口：`11434`

## 常见问题

### Q: 安装后找不到命令？
重启终端或添加到 PATH 环境变量。

### Q: GPU 加速？
Ollama 自动检测 NVIDIA GPU。如果有 GPU，embedding 速度会快 5-10 倍。

### Q: 如何查看已安装模型？
```bash
ollama list
```
