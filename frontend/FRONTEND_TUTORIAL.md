# wechat-article-insights 前端学习指南

> 面向有后端开发经验（如 Rust）但前端零基础的开发者

---

## 目录

1. [技术栈概览](#1-技术栈概览)
2. [项目结构](#2-项目结构)
3. [核心概念](#3-核心概念)
4. [Nuxt 3 专题](#4-nuxt-3-专题)
5. [IndexedDB 专题](#5-indexeddb-专题)
6. [实战示例](#6-实战示例)
7. [开发流程](#7-开发流程)

---

## 1. 技术栈概览

### 1.1 核心框架

| 技术 | 类比 Rust 生态 | 作用 |
|------|---------------|------|
| **Vue.js 3** | 类似 Yew/Leptos | 响应式 UI 框架 |
| **Nuxt.js 3** | 类似 Axum + 模板引擎 | Vue 的全栈框架 |
| **TypeScript** | 类似 Rust 的类型系统 | 带类型的 JavaScript |
| **Vite** | 类似 cargo | 构建工具 |

### 1.2 与 Rust 的对比

```
Rust 概念           →  TypeScript/Vue 概念
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
struct              →  interface / type
impl                →  class methods
trait               →  interface (抽象)
enum                →  union types
Option<T>           →  T | undefined
Result<T, E>        →  try-catch / Promise
async/await         →  async/await (相同！)
Cargo.toml          →  package.json
cargo run           →  npm run dev
```

---

## 2. 项目结构

```
wechat-article-insights/
├── pages/                  # 页面组件（自动路由）
│   └── dashboard/
│       ├── account.vue     # 公众号管理页
│       └── article.vue     # 文章下载页
├── components/             # 可复用组件
│   ├── modal/              # 弹窗组件
│   └── grid/               # 表格组件
├── composables/            # 可复用逻辑（类似 Rust 的 utils）
│   ├── useDownloader.ts    # 下载逻辑
│   └── useExporter.ts      # 导出逻辑
├── store/                  # 数据存储
│   └── v2/
│       ├── db.ts           # IndexedDB 定义
│       ├── article.ts      # 文章数据操作
│       └── html.ts         # HTML 缓存操作
├── utils/                  # 工具函数
│   └── download/
│       ├── Downloader.ts   # 下载器类
│       └── Exporter.ts     # 导出器类
├── types/                  # TypeScript 类型定义
├── nuxt.config.ts          # Nuxt 配置（类似 Cargo.toml）
└── package.json            # 依赖管理
```

### 关键目录说明

- **pages/**: 每个 `.vue` 文件自动成为一个路由
  - `pages/index.vue` → `/`
  - `pages/dashboard/account.vue` → `/dashboard/account`

- **composables/**: Vue 3 的 "钩子" 函数，类似 Rust 的 trait 方法
  - 以 `use` 开头命名
  - 提取可复用的状态和逻辑

- **store/**: 本地数据持久化（使用 IndexedDB）

---

## 3. 核心概念

### 3.1 Vue 单文件组件 (.vue)

一个 `.vue` 文件包含三个部分：

```vue
<!-- 1. 模板：HTML 结构 -->
<template>
  <div>
    <h1>{{ title }}</h1>
    <button @click="handleClick">点击</button>
  </div>
</template>

<!-- 2. 脚本：逻辑 -->
<script setup lang="ts">
// 响应式变量
const title = ref('Hello')

// 函数
function handleClick() {
  title.value = 'Clicked!'
}
</script>

<!-- 3. 样式：CSS -->
<style scoped>
h1 { color: blue; }
</style>
```

> **❓ Q: 模板里的 `{{ title }}` 和脚本里的 `const title = ref('Hello')` 是同一个东西吗？**
>
> **A: 是的！它们是同一个变量。** 这是 Vue 的核心机制：
>
> 1. 在 `<script setup>` 中定义的变量会**自动暴露**给 `<template>`
> 2. 模板中的 `{{ title }}` 会自动解包 `ref`（不需要写 `.value`）
> 3. 当脚本中 `title.value` 改变时，模板中的 `{{ title }}` **自动更新**
>
> **类比 Rust：**
> ```rust
> // 想象 Vue 的 <script setup> 像这样工作：
> struct Component {
>     title: Rc<RefCell<String>>,  // ref() 创建的响应式变量
> }
>
> impl Component {
>     // 模板就像一个自动订阅了 title 的 render 函数
>     fn render(&self) -> Html {
>         html! { <h1>{ self.title.borrow() }</h1> }
>     }
> }
> ```
>
> **关键点：** `<script setup>` 是 Vue 3 的语法糖，它让顶层变量和函数自动对模板可用，无需手动 `return`。

### 3.2 响应式系统

Vue 的核心是 **响应式**：当数据变化时，UI 自动更新。

```typescript
// ref - 用于基础类型
const count = ref(0)
count.value++  // 需要 .value

// reactive - 用于对象
const user = reactive({ name: 'Alice', age: 25 })
user.name = 'Bob'  // 不需要 .value

// computed - 计算属性（类似 Rust 的 getter）
const doubled = computed(() => count.value * 2)
```

**类比 Rust:**
```rust
// Rust 中你可能这样写
let count = RefCell::new(0);
*count.borrow_mut() += 1;

// Vue 的 ref 类似于 RefCell + 自动 UI 更新
```

### 3.3 生命周期

```typescript
// 组件挂载完成后执行（类似 Rust 的 impl Default）
onMounted(() => {
  console.log('组件已挂载')
  loadData()
})

// 组件卸载前执行（类似 Drop trait）
onUnmounted(() => {
  cleanup()
})
```

### 3.4 Composables（组合式函数）

类似 Rust 中将逻辑抽取到单独的 `impl` 块：

```typescript
// composables/useCounter.ts
export function useCounter() {
  const count = ref(0)
  
  function increment() {
    count.value++
  }
  
  return { count, increment }
}

// 在组件中使用
const { count, increment } = useCounter()
```

---

## 6. 实战示例

### 6.1 本项目中的 useDownloader

查看 `composables/useDownloader.ts`：

```typescript
export default function useDownloader(options: DownloadArticleOptions) {
  // 状态
  const loading = ref(false)
  const completed_count = ref(0)
  const total_count = ref(0)
  
  // 下载器实例
  let manager: Downloader
  
  // 下载 HTML 内容
  async function downloadArticleHTML(urls: string[]) {
    manager = new Downloader(urls)
    manager.on('download:progress', (url, success, status) => {
      completed_count.value = status.completed.length
    })
    await manager.startDownload('html')
  }
  
  // 返回状态和方法
  return {
    loading,
    completed_count,
    total_count,
    download: downloadArticleHTML,
  }
}
```

### 6.2 本项目中的 IndexedDB 操作

查看 `store/v2/article.ts`：

```typescript
// 定义类型（类似 Rust struct）
export interface ArticleAsset {
  fakeid: string
  url: string
  title: string
  // ...
}

// 存储数据
export async function updateArticleCache(asset: ArticleAsset) {
  await db.article.put(asset)  // IndexedDB 操作
}

// 读取数据
export async function getArticleCache(fakeid: string, time: number) {
  return await db.article.where('fakeid').equals(fakeid).toArray()
}
```

### 6.3 模板语法速查

```html
<!-- 文本插值 -->
<span>{{ message }}</span>

<!-- 属性绑定 -->
<img :src="imageUrl">

<!-- 事件绑定 -->
<button @click="handleClick">

<!-- 条件渲染 -->
<div v-if="isVisible">显示</div>
<div v-else>隐藏</div>

<!-- 列表渲染 -->
<li v-for="item in items" :key="item.id">
  {{ item.name }}
</li>

<!-- 双向绑定（表单） -->
<input v-model="searchText">
```

---

## 7. 开发流程

### 7.1 启动开发服务器

```bash
npm run dev    # 启动开发服务器
# 访问 http://localhost:3000
```

### 7.2 常用命令

```bash
npm install          # 安装依赖（类似 cargo build）
npm run dev          # 开发模式（类似 cargo run）
npm run build        # 生产构建
npm run format       # 代码格式化
```

### 7.3 调试技巧

1. **浏览器开发者工具**: F12 打开
   - Console: 查看日志和错误
   - Network: 查看网络请求
   - Elements: 查看 DOM 结构

2. **Vue DevTools**: Chrome 扩展，查看组件状态

3. **console.log**: 最简单的调试方式

### 7.4 修改代码的基本流程

1. 找到要修改的页面：`pages/` 目录
2. 找到相关组件：`components/` 目录
3. 找到业务逻辑：`composables/` 或 `utils/`
4. 修改后保存，浏览器会自动刷新（热更新）

---

## 快速参考卡片

### TypeScript 类型语法

```typescript
// 基础类型
let name: string = 'hello'
let count: number = 42
let active: boolean = true

// 数组
let items: string[] = ['a', 'b']

// 对象/接口
interface User {
  name: string
  age?: number  // 可选字段（类似 Option）
}

// 函数
function greet(name: string): string {
  return `Hello ${name}`
}

// 异步函数
async function fetchData(): Promise<Data> {
  const response = await fetch(url)
  return response.json()
}
```

### 常用 Vue 3 API

```typescript
import { ref, reactive, computed, watch, onMounted } from 'vue'

// 响应式
const value = ref(0)            // 基础类型
const obj = reactive({})        // 对象

// 计算属性
const doubled = computed(() => value.value * 2)

// 侦听器
watch(value, (newVal, oldVal) => {
  console.log('value changed')
})

// 生命周期
onMounted(() => { /* 初始化 */ })
```

---

## 4. Nuxt 3 专题

### 4.1 什么是 Nuxt？

Nuxt 是 Vue 的**全栈框架**，类比 Rust 生态：

| Nuxt 功能 | 类比 Rust |
|----------|----------|
| 服务端渲染 (SSR) | 类似 Axum + Askama 模板 |
| 文件路由 | 自动根据文件结构生成路由 |
| API 路由 | 类似 Axum 的 handler |
| 自动导入 | 无需手动 import 常用函数 |

### 4.2 文件路由系统

```
pages/
├── index.vue          → /
├── about.vue          → /about
├── dashboard/
│   ├── index.vue      → /dashboard
│   ├── account.vue    → /dashboard/account
│   └── article.vue    → /dashboard/article
└── user/
    └── [id].vue       → /user/:id (动态路由)
```

**动态路由参数：**
```typescript
// pages/user/[id].vue
const route = useRoute()
const userId = route.params.id  // 获取 URL 中的 id
```

### 4.3 Nuxt 自动导入

Nuxt 会自动导入以下内容，无需手动 `import`：

```typescript
// 这些都不需要 import！
const route = useRoute()           // 路由信息
const router = useRouter()         // 路由导航
const { data } = await useFetch()  // 数据获取
const count = ref(0)               // Vue 响应式
const doubled = computed(() => {}) // 计算属性
```

### 4.4 Nuxt composables

```typescript
// composables/useMyHook.ts 文件会自动可用
export function useMyHook() {
  const data = ref(null)
  return { data }
}

// 任何组件中直接使用，无需 import
const { data } = useMyHook()
```

### 4.5 API 路由

```typescript
// server/api/hello.ts
export default defineEventHandler((event) => {
  return { message: 'Hello World' }
})

// 访问: GET /api/hello
```

---

## 5. IndexedDB 专题

### 5.1 什么是 IndexedDB？

IndexedDB 是浏览器内置的 **NoSQL 数据库**，用于客户端持久化存储。

> **❓ Q: IndexedDB 和 MongoDB 差不多吗？**
>
> **A: 是的，非常相似！** 两者都是 NoSQL 文档型数据库：
>
> | 特性 | IndexedDB | MongoDB |
> |------|-----------|---------|
> | 类型 | NoSQL 文档数据库 | NoSQL 文档数据库 |
> | 数据格式 | JavaScript 对象 | BSON (JSON 变体) |
> | 运行位置 | 浏览器客户端 | 服务器端 |
> | 存储限制 | ~50MB-无限(依浏览器) | 无限 |
> | 查询语言 | 游标/索引 API | MongoDB Query Language |
> | 主要用途 | 离线缓存、本地存储 | 后端数据存储 |
>
> **主要区别：** IndexedDB 是客户端存储（类似 SQLite 嵌入式数据库），而 MongoDB 是服务器端数据库。

| IndexedDB 概念 | MongoDB 类比 | Rust/SQLite 类比 |
|---------------|-------------|-----------------|
| Database | Database | SQLite 文件 |
| Object Store | Collection | 表 |
| Document | Document | 行 |
| Index | Index | 索引 |
| Transaction | Transaction | 事务 |

### 5.2 本项目使用 Dexie.js

[Dexie.js](https://dexie.org/) 是 IndexedDB 的封装库，让操作更简单：

```typescript
// store/v2/db.ts - 定义数据库结构
import Dexie from 'dexie'

class ExporterDatabase extends Dexie {
  info!: Table<Info>          // 公众号信息表
  article!: Table<ArticleAsset>  // 文章列表表
  html!: Table<HtmlAsset>     // 文章HTML表
  comment!: Table<CommentAsset>  // 评论表
  
  constructor() {
    super('exporter.wxdown.online')  // 数据库名
    this.version(1).stores({
      info: 'fakeid',              // 主键
      article: 'url, fakeid',      // 主键 + 索引
      html: 'url',
      comment: 'url',
    })
  }
}

export const db = new ExporterDatabase()
```

### 5.3 CRUD 操作

```typescript
// 创建/更新
await db.article.put({ url: '...', fakeid: '...', title: '...' })

// 读取单条
const article = await db.article.get('https://...')

// 查询多条
const articles = await db.article
  .where('fakeid')
  .equals('xxx')
  .toArray()

// 删除
await db.article.delete('https://...')

// 批量操作
await db.article.bulkPut([article1, article2])
```

### 5.4 本项目的数据表结构

| 表名 | 主键 | 用途 |
|------|------|------|
| `info` | fakeid | 公众号基本信息 |
| `article` | url | 文章元数据（标题、发布时间等）|
| `html` | url | 文章 HTML 内容 |
| `metadata` | url | 阅读量、点赞数等 |
| `comment` | url | 评论数据 |
| `resource` | url | 图片等资源文件 |

### 5.5 调试 IndexedDB

1. 打开浏览器开发者工具 (F12)
2. 进入 **Application** 标签
3. 左侧找到 **IndexedDB** 
4. 点击数据库 `exporter.wxdown.online`
5. 查看各个表的数据

---

## 6. 实战示例
