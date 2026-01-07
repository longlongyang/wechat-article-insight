<script setup lang="ts">
import dayjs from 'dayjs';
import { saveAs } from 'file-saver';
import html2pdf from 'html2pdf.js';
import MarkdownIt from 'markdown-it';
import TurndownService from 'turndown';
import type { FormSubmitEvent } from '#ui/types';
import { useLLMConfig } from '~/composables/useLLMConfig';
import { websiteName } from '~/config';

const md = new MarkdownIt();

function renderMarkdown(text: string) {
  return md.render(text || '');
}

const { rustPost, rustGet } = useRustBackend();
const { isActive } = usePageActive();
const { config } = useLLMConfig();
const toast = useToast();
const loginAccount = useLoginAccount();

useHead({
  title: `全网洞察 | ${websiteName}`,
});

// Types
interface InsightTask {
  id: string;
  prompt: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  keywords: string[];
  target_count: number;
  processed_count: number;
  created_at: number;
  updated_at: number;
  completion_reason?: string;
}

interface InsightArticle {
  id: string;
  task_id: string;
  title: string;
  url: string;
  account_name?: string;
  account_fakeid?: string;
  publish_time?: number;
  similarity?: number;
  insight?: string;
  relevance_score?: number;
  created_at: number;
}

interface CreateState {
  prompt: string;
  target_count: number;
  specific_account_fakeid?: string;
  specific_account_name?: string;
  // Advanced LLM Config
  keywordProvider: 'gemini' | 'deepseek';
  reasoningProvider: 'gemini' | 'deepseek';
  embeddingProvider: 'gemini' | 'ollama';
  // Search Speed (risk level)
  searchSpeed: 'high' | 'medium' | 'low';
}

interface PrefetchStats {
  article_success: number;
  article_failed: number;
  image_success: number;
  image_failed: number;
}

// State
const tasks = ref<InsightTask[]>([]);
const activeTask = ref<InsightTask | null>(null);
const activeArticles = ref<InsightArticle[]>([]);
const isLoadingTasks = ref(false);
const isLoadingDetail = ref(false);

// Create Modal
const isCreateModalOpen = ref(false);
const showAdvancedConfig = ref(false);

const createForm = reactive<CreateState>({
  prompt: '',
  target_count: 30,
  specific_account_fakeid: undefined,
  specific_account_name: undefined,
  keywordProvider: 'gemini',
  reasoningProvider: 'gemini',
  embeddingProvider: 'gemini',
  searchSpeed: 'medium',
});

// Load saved provider preferences from localStorage after mount (client-side only)
const prefsLoaded = ref(false);

onMounted(() => {
  // Use setTimeout to ensure all component hydration is complete
  setTimeout(() => {
    try {
      const saved = localStorage.getItem('insight_provider_prefs');
      console.log('[Insights] Loading saved prefs:', saved);
      if (saved) {
        const prefs = JSON.parse(saved);
        if (prefs.keywordProvider) createForm.keywordProvider = prefs.keywordProvider;
        if (prefs.reasoningProvider) createForm.reasoningProvider = prefs.reasoningProvider;
        if (prefs.embeddingProvider) createForm.embeddingProvider = prefs.embeddingProvider;
        if (prefs.searchSpeed) createForm.searchSpeed = prefs.searchSpeed;
        console.log('[Insights] Applied prefs:', prefs);
      }
    } catch (e) {
      console.warn('Failed to load provider preferences:', e);
    }
    // Mark as loaded so watch can start saving (after another delay to be safe)
    setTimeout(() => {
      prefsLoaded.value = true;
      console.log('[Insights] prefsLoaded set to true');
    }, 100);
  }, 100);
});

// Watch provider changes and save to localStorage (only after initial load)
watch(
  () => [createForm.keywordProvider, createForm.reasoningProvider, createForm.embeddingProvider, createForm.searchSpeed],
  ([kw, reasoning, embedding, speed]) => {
    if (!prefsLoaded.value) {
      console.log('[Insights] Skipping save, prefsLoaded is false');
      return;
    }
    const data = {
      keywordProvider: kw,
      reasoningProvider: reasoning,
      embeddingProvider: embedding,
      searchSpeed: speed,
    };
    console.log('[Insights] Saving prefs:', data);
    localStorage.setItem('insight_provider_prefs', JSON.stringify(data));
  }
);

const isCreating = ref(false);

// Preview Drawer
const isPreviewOpen = ref(false);
const activeArticle = ref<InsightArticle | null>(null);

// Login Required Modal
const showLoginRequiredModal = ref(false);

// Methods
async function fetchTasks() {
  isLoadingTasks.value = true;
  try {
    const res = await rustGet<InsightTask[]>('/api/insight/list');
    tasks.value = res || [];
  } catch (e) {
    console.error('Failed to fetch tasks', e);
  } finally {
    isLoadingTasks.value = false;
  }
}

async function selectTask(task: InsightTask) {
  activeTask.value = task;
  isLoadingDetail.value = true;
  try {
    const res = await rustGet<{ task: InsightTask; articles: InsightArticle[] }>(`/api/insight/${task.id}`);
    activeTask.value = res.task;
    activeArticles.value = res.articles;
  } catch (e) {
    console.error('Failed to fetch task detail', e);
  } finally {
    isLoadingDetail.value = false;
  }
}

function openCreateModal(prompt = '', specificAccount?: { fakeid: string; name: string }) {
  // Check if user is logged in
  if (!loginAccount.value) {
    showLoginRequiredModal.value = true;
    return;
  }

  createForm.prompt = prompt;
  createForm.target_count = 30;
  createForm.specific_account_fakeid = specificAccount?.fakeid;
  createForm.specific_account_name = specificAccount?.name;
  // Note: Do NOT reset provider settings - keep the saved preferences from localStorage
  // Only reset embeddingProvider if ollama is disabled but was previously selected
  if (!config.value.ollamaEnabled && createForm.embeddingProvider === 'ollama') {
    createForm.embeddingProvider = 'gemini';
  }
  showAdvancedConfig.value = false;
  isCreateModalOpen.value = true;
}

async function createTask() {
  if (!createForm.prompt) return;
  isCreating.value = true;
  try {
    await rustPost('/api/insight/create', {
      prompt: createForm.prompt,
      target_count: createForm.target_count,
      deepseek_api_key: config.value.deepseekApiKey || undefined,
      gemini_api_key: config.value.geminiApiKey || undefined,
      specific_account_fakeid: createForm.specific_account_fakeid,
      specific_account_name: createForm.specific_account_name,
      // Advanced LLM Config
      keyword_provider: createForm.keywordProvider,
      reasoning_provider: createForm.reasoningProvider,
      embedding_provider: createForm.embeddingProvider,
      ollama_base_url: config.value.ollamaEnabled ? config.value.ollamaBaseUrl : undefined,
      ollama_embedding_model: config.value.ollamaEnabled ? config.value.ollamaEmbeddingModel : undefined,
      // Search Speed
      search_speed: createForm.searchSpeed,
    });
    isCreateModalOpen.value = false;
    // reset form
    createForm.prompt = '';
    createForm.specific_account_fakeid = undefined;
    createForm.specific_account_name = undefined;

    await fetchTasks();
    if (tasks.value.length > 0) {
      selectTask(tasks.value[0]);
    }
    toast.add({ title: '任务已创建', color: 'green' });
  } catch (e: any) {
    const errorMsg = e.message || '未知错误';
    console.error('[CreateTask] Error:', errorMsg);

    // Check if session expired - only for specific session error messages
    // 后端返回格式: "微信登录已过期，请重新登录: Session invalid (200003): xxx"
    if (errorMsg.includes('微信登录已过期') || errorMsg.includes('Session invalid')) {
      isCreateModalOpen.value = false;
      loginAccount.value = null; // Clear login state to show login button
      toast.add({
        title: '微信登录已过期',
        description: '请重新登录后再创建任务',
        color: 'red',
        timeout: 5000,
      });
    } else if (errorMsg.includes('请先登录微信公众平台')) {
      // No session at all
      isCreateModalOpen.value = false;
      showLoginRequiredModal.value = true;
    } else {
      // Other errors - just show the message
      toast.add({ title: '创建任务失败', description: errorMsg, color: 'red' });
    }
  } finally {
    isCreating.value = false;
  }
}

// Account Actions
async function addToMonitored(fakeid: string, name: string) {
  try {
    await rustPost('/api/account/add', { fakeid, nickname: name });
    toast.add({ title: `已添加公众号: ${name}`, color: 'green' });
  } catch (e: any) {
    toast.add({ title: '添加失败', description: e.message, color: 'red' });
  }
}

function appendInsight(fakeid: string, name: string) {
  // Pre-fill prompt for append context, user can modify
  openCreateModal(`针对 [${name}] 的深度洞察`, { fakeid, name });
}

async function cancelTask(task: InsightTask) {
  if (!confirm('确定要终止当前任务吗？')) return;
  try {
    await rustPost('/api/insight/cancel', { id: task.id });
    toast.add({ title: '已发送终止请求', color: 'blue' });

    // Optimistic update to show "cancelling" immediately
    const t = tasks.value.find(t => t.id === task.id);
    if (t) t.status = 'cancelling';

    if (activeTask.value?.id === task.id) {
      activeTask.value.status = 'cancelling';
    }

    // Refresh tasks after a short delay to get the final status from DB
    setTimeout(async () => {
      await fetchTasks();
      // Also refresh active task if still the same
      if (activeTask.value?.id === task.id) {
        const refreshed = tasks.value.find(t => t.id === task.id);
        if (refreshed) activeTask.value = refreshed;
      }
    }, 2000); // 2 second delay to allow backend worker to finalize
  } catch (e: any) {
    toast.add({ title: '取消失败', description: e.message, color: 'red' });
  }
}

async function prefetchTask(task: InsightTask, silent = false) {
  if (!silent && !confirm('确定要预取此任务的文章和图片吗？\n这将后台下载所有文章内容并压缩存储图片。')) return;

  isPrefetching.value = true;
  prefetchingTasks.value.add(task.id); // Add to loading set
  currentPrefetchTask.value = task;

  if (!silent) {
    toast.add({ title: '预取任务已开始', description: '正在下载文章和图片...', color: 'blue' });
  }

  try {
    const prefs = usePreferences();
    const proxies = prefs.value.privateProxyList;
    const authorization = prefs.value.privateProxyAuthorization;

    const res = await rustPost<{ success: boolean; message: string; stats: PrefetchStats }>('/api/insight/prefetch', {
      task_id: task.id,
      proxies: proxies,
      authorization: authorization,
    });

    prefetchStats.value = res.stats;

    if (!silent) {
      isPrefetchResultModalOpen.value = true;
    }
    await fetchTasks();
    return res.stats;
  } catch (e: any) {
    if (!silent) {
      toast.add({ title: '预取失败', description: e.message, color: 'red' });
    }
    throw e;
  } finally {
    isPrefetching.value = false;
    prefetchingTasks.value.delete(task.id); // Remove from loading set
  }
}

async function deleteTask(task: InsightTask) {
  if (!confirm('确定要删除此任务及其所有数据吗？此操作不可恢复。')) return;
  try {
    await rustPost('/api/insight/delete', { id: task.id });
    toast.add({ title: '任务已删除', color: 'green' });

    // Immediately remove from UI
    tasks.value = tasks.value.filter(t => t.id !== task.id);

    // Clear active if matches
    if (activeTask.value?.id === task.id) {
      activeTask.value = null;
      activeArticles.value = [];
    }
  } catch (e: any) {
    toast.add({ title: '删除失败', description: e.message, color: 'red' });
  }
}

// Export Logic (Single Article)
const isExporting = ref(false);

// Batch Export Logic
const isExportModalOpen = ref(false);
const exportForm = reactive({
  target_dir: 'C:\\Users\\long\\Desktop', // Default suggestion
  format: 'markdown' as 'markdown' | 'pdf',
  task_id: '',
});
const isExportingBatch = ref(false);
const failedResult = ref('');

// Prefetch Result Modal
const isPrefetchResultModalOpen = ref(false);
const prefetchStats = ref<PrefetchStats | null>(null);
const currentPrefetchTask = ref<InsightTask | null>(null);

const isPrefetching = ref(false);
const prefetchingTasks = ref<Set<string>>(new Set());

function openExportModal(task: InsightTask) {
  exportForm.task_id = task.id;
  isExportModalOpen.value = true;
}

async function submitBatchExport() {
  if (!exportForm.target_dir) {
    toast.add({ title: '请输入导出目录', color: 'red' });
    return;
  }

  isExportingBatch.value = true;
  failedResult.value = ''; // Reset error
  try {
    // Step 1: Prefetch
    if (exportForm.task_id) {
      const task = tasks.value.find(t => t.id === exportForm.task_id);
      if (task) {
        // Determine if we need to show prefetch progress
        // We'll just set isPrefetching and let the UI handle "Prefetching..." text
        // isPrefetching is set inside prefetchTask
        await prefetchTask(task, true);
      }
    }

    // Step 2: Export
    // Retrieve proxy settings
    const prefs = usePreferences();
    const proxies = prefs.value.privateProxyList;
    const authorization = prefs.value.privateProxyAuthorization;

    const res = await rustPost<{ success: boolean; message: string }>(
      '/api/insight/export',
      {
        task_id: exportForm.task_id,
        target_dir: exportForm.target_dir,
        format: exportForm.format,
        proxies: proxies,
        authorization: authorization,
      },
      { timeout: 7200000 }
    ); // 2 hours timeout for large batch exports

    if (res.success) {
      toast.add({ title: '导出成功', description: res.message, color: 'green' });
      isExportModalOpen.value = false;
    } else {
      console.error('Export failed:', res.message);
      failedResult.value = res.message;
    }
  } catch (e: any) {
    failedResult.value = e.message || '网络请求异常';
  } finally {
    isExportingBatch.value = false;
  }
}

async function exportMarkdown() {
  if (!articleContent.value || !activeArticle.value) return;
  isExporting.value = true;
  try {
    const turndownService = new TurndownService();
    // Configure turndown to keep some elements if needed, or default
    const markdown = turndownService.turndown(articleContent.value);

    // Prepend metadata
    const frontmatter = `---
title: ${activeArticle.value.title}
author: ${activeArticle.value.account_name}
date: ${dayjs.unix(activeArticle.value.publish_time || 0).format('YYYY-MM-DD HH:mm')}
url: ${activeArticle.value.url}
---

# ${activeArticle.value.title}

> AI Insight: ${activeArticle.value.insight || 'N/A'}

`;

    const blob = new Blob([frontmatter + markdown], { type: 'text/markdown;charset=utf-8' });
    saveAs(blob, `${activeArticle.value.title || 'article'}.md`);
  } catch (e: any) {
    toast.add({ title: '导出失败', description: e.message, color: 'red' });
  } finally {
    isExporting.value = false;
  }
}

async function exportPDF() {
  if (!articleContent.value || !activeArticle.value) return;
  isExporting.value = true;
  const filename = activeArticle.value.title || 'article';

  try {
    // 1. Try Backend PDF Generation (Prince)
    const response = await fetch('http://localhost:3001/api/pdf', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        html: articleContent.value,
        filename: filename,
      }),
    });

    if (response.ok) {
      const blob = await response.blob();
      saveAs(blob, `${filename}.pdf`);
      return;
    }

    // 2. Fallback to Frontend html2pdf if backend fails
    throw new Error('Backend generation failed, trying frontend fallback');
  } catch (e) {
    // Frontend Fallback
    try {
      const element = document.createElement('div');
      // Wrap content to conform to PDF styling
      element.innerHTML = `
        <div style="padding: 40px; font-family: sans-serif;">
            <h1 style="margin-bottom: 10px;">${activeArticle.value.title}</h1>
            <div style="color: #666; margin-bottom: 20px; border-bottom: 1px solid #eee; padding-bottom: 10px;">
                <span>${activeArticle.value.account_name || '公众号'}</span> · 
                <span>${dayjs.unix(activeArticle.value.publish_time || 0).format('YYYY-MM-DD')}</span>
            </div>
            ${activeArticle.value.insight ? `<div style="background: #fdf4ff; padding: 15px; border-radius: 8px; margin-bottom: 20px; border: 1px solid #f0abfc;"><strong>AI 洞察:</strong> ${activeArticle.value.insight}</div>` : ''}
            <div>${articleContent.value}</div>
        </div>
      `;

      const images = element.querySelectorAll('img');
      images.forEach(img => {
        img.style.maxWidth = '100%';
      });

      const opt = {
        margin: 10,
        filename: `${filename}.pdf`,
        image: { type: 'jpeg', quality: 0.98 },
        html2canvas: { scale: 2, useCORS: true },
        jsPDF: { unit: 'mm', format: 'a4', orientation: 'portrait' },
      };

      await html2pdf().set(opt).from(element).save();
    } catch (feErr: any) {
      toast.add({ title: '导出 PDF 失败', description: feErr.message, color: 'red' });
    }
  } finally {
    isExporting.value = false;
  }
}

// Preview
const articleContent = ref('');
const isArticleLoading = ref(false);
const preferences = usePreferences(); // Use global composable if available, or import

async function openArticlePreview(article: InsightArticle) {
  activeArticle.value = article;
  isPreviewOpen.value = true;
  articleContent.value = '';
  isArticleLoading.value = true;

  try {
    const url = article.url;
    if (!url) throw new Error('URL 缺失');

    // Retrieve proxy settings from preferences if possible, or default to backend handling
    // In this project context, preferences might be available via usePreferences or local storage
    // For simplicity and to match search.vue, we'll try to get them
    const prefs = preferences.value as any;
    const proxies = prefs?.privateProxyList || [];
    const authorization = prefs?.privateProxyAuthorization || undefined;

    // Call the same fetch endpoint as search.vue
    const response = await fetch('http://localhost:3001/api/public/v1/article/fetch', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        url: url,
        id: article.id,
        proxies: proxies.length > 0 ? proxies : undefined,
        authorization: authorization,
      }),
    });

    if (!response.ok) {
      const errText = await response.text();
      throw new Error(`Status: ${response.status} - ${errText}`);
    }

    articleContent.value = await response.text();
  } catch (e: any) {
    articleContent.value = `<div class="p-8 text-center text-red-500">
        <p class="font-bold text-lg mb-2">加载失败</p>
        <p>${e.message}</p>
        <p class="mt-4 text-sm text-gray-500">请检查网络或配置代理节点</p>
      </div>`;
  } finally {
    isArticleLoading.value = false;
  }
}

function formatDate(ts: number) {
  return dayjs.unix(ts).format('YYYY-MM-DD HH:mm');
}

function getStatusColor(status: string) {
  switch (status) {
    case 'completed':
      return 'green';
    case 'processing':
      return 'blue';
    case 'failed':
      return 'red';
    case 'cancelling':
      return 'orange';
    case 'cancelled':
      return 'gray';
    default:
      return 'gray';
  }
}

function getStatusLabel(status: string) {
  switch (status) {
    case 'completed':
      return '完成';
    case 'processing':
      return '进行中';
    case 'failed':
      return '失败';
    case 'pending':
      return '等待中';
    case 'cancelling':
      return '取消中';
    case 'cancelled':
      return '已取消';
    default:
      return status;
  }
}

// Auto refresh active task if processing or cancelling
const { pause, resume } = useIntervalFn(() => {
  if (activeTask.value && (activeTask.value.status === 'processing' || activeTask.value.status === 'cancelling')) {
    rustGet<{ task: InsightTask; articles: InsightArticle[] }>(`/api/insight/${activeTask.value.id}`)
      .then(res => {
        activeTask.value = res.task;
        activeArticles.value = res.articles;
        if (res.task.status === 'completed' || res.task.status === 'failed' || res.task.status === 'cancelled') {
          fetchTasks();
        }
      })
      .catch(console.error);
  }
}, 3000);

onMounted(() => {
  fetchTasks();
});
</script>

<template>
  <div class="h-full flex flex-col">
    <Teleport v-if="isActive" defer to="#title">
      <h1 class="text-[28px] leading-[34px] text-slate-12 dark:text-slate-50 font-bold flex items-center gap-3">
        <div class="size-8 rounded-lg bg-gradient-to-tr from-blue-500 to-cyan-400 flex items-center justify-center text-white shadow-lg shadow-blue-500/20">
          <UIcon name="i-lucide:globe" class="size-5" />
        </div>
        全网洞察
      </h1>
    </Teleport>

    <div class="flex-1 flex overflow-hidden">
      <!-- Sidebar / Task List -->
      <div class="w-80 border-r dark:border-gray-800 flex flex-col bg-gray-50 dark:bg-gray-900/50">
        <div class="p-4 border-b dark:border-gray-800">
          <UButton block color="black" icon="i-lucide:plus" @click="openCreateModal()">
            新建洞察任务
          </UButton>
        </div>
        
        <div class="flex-1 overflow-y-auto p-2 space-y-2">
            <div 
              v-for="task in tasks" 
              :key="task.id"
              class="p-3 border rounded-lg cursor-pointer hover:border-black dark:hover:border-white transition-colors group relative"
              :class="{ 'bg-gray-50 dark:bg-gray-800 border-gray-300 dark:border-gray-700': activeTask?.id === task.id }"
              @click="selectTask(task)"
            >
              <div class="flex justify-between items-start mb-2">
                <div class="line-clamp-2 font-medium text-sm pr-6">{{ task.prompt }}</div>
                <UBadge :color="getStatusColor(task.status)" size="xs" variant="subtle">{{ getStatusLabel(task.status) }}</UBadge>
              </div>
              <div class="flex items-center justify-between text-xs text-gray-400 mt-2">
                <span>{{ formatDate(task.created_at) }}</span>
                <span>{{ task.processed_count }} / {{ task.target_count }}</span>
              </div>
              
              <!-- Completion Reason -->
              <div v-if="task.completion_reason" class="mt-2 text-xs border-t dark:border-gray-700 pt-2 break-all" :class="task.status === 'failed' ? 'text-red-500' : 'text-gray-500'">
                 <span v-if="task.status === 'failed'" class="i-lucide-alert-circle inline-block mr-1 align-sub"></span>
                 {{ task.completion_reason }}
              </div>
              
              <!-- Actions (Visible on Hover, bottom right) -->
              <div class="absolute bottom-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity flex gap-1 bg-gray-50/90 dark:bg-gray-800/90 pl-2 rounded-l-md backdrop-blur-sm">

                 <UButton 
                   icon="i-lucide:download" 
                   color="blue" 
                   variant="ghost" 
                   size="2xs" 
                   title="批量导出 (Markdown/PDF)"
                   @click.stop="openExportModal(task)"
                 />
                 <UButton 
                   icon="i-lucide:zap" 
                   color="amber" 
                   variant="ghost" 
                   size="2xs" 
                   title="预取文章与图片 (Prefetch)"
                   :loading="prefetchingTasks.has(task.id)"
                   @click.stop="prefetchTask(task)"
                 />
                 <UButton 
                   icon="i-lucide:trash-2" 
                   color="red" 
                   variant="ghost" 
                   size="2xs" 
                   title="删除任务"
                   @click.stop="deleteTask(task)"
                 />
              </div>
            </div>
          <div v-if="tasks.length === 0 && !isLoadingTasks" class="text-center py-10 text-gray-500 text-sm">
            暂无任务
          </div>
        </div>
      </div>

      <!-- Main Content -->
      <div class="flex-1 flex flex-col overflow-hidden bg-white dark:bg-gray-900">
        <template v-if="activeTask">
           <!-- Header (Same as before) -->
           <div class="p-6 border-b dark:border-gray-800">
             <div class="flex justify-between items-start">
               <div>
                 <h2 class="text-xl font-bold mb-2 flex items-center gap-2">
                   {{ activeTask.prompt }}
                   <UButton 
                     v-if="['processing', 'pending'].includes(activeTask.status)"
                     size="2xs" 
                     color="red" 
                     variant="soft" 
                     icon="i-lucide:square" 
                     @click="cancelTask(activeTask)"
                   >
                     终止
                   </UButton>
                 </h2>
                 <div class="flex items-center gap-4 text-sm text-gray-500">
                   <span class="flex items-center gap-1">
                     <UIcon name="i-lucide:calendar" />
                     {{ formatDate(activeTask.created_at) }}
                   </span>
                   <span class="flex items-center gap-1">
                     <UIcon name="i-lucide:key" />
                     关键词: {{ activeTask.keywords.join(', ') || '生成中...' }}
                   </span>
                 </div>
               </div>
               <div class="text-right">
                 <div class="text-2xl font-bold font-mono">
                   {{ activeTask.processed_count }} <span class="text-sm text-gray-400 font-normal">/ {{ activeTask.target_count }}</span>
                 </div>
                 <div class="text-xs text-gray-500">已分析文章</div>
               </div>
             </div>
             <div class="mt-4 h-2 bg-gray-100 dark:bg-gray-800 rounded-full overflow-hidden">
               <div 
                 class="h-full bg-blue-500 transition-all duration-500"
                 :style="{ width: `${Math.min(100, (activeTask.processed_count / activeTask.target_count) * 100)}%` }"
                 :class="activeTask.status === 'processing' ? 'animate-pulse' : ''"
               ></div>
             </div>
             
             <!-- Completion Reason Alert -->
             <div v-if="activeTask.completion_reason" class="mt-4 rounded-md p-3 text-sm flex items-start gap-2" :class="activeTask.status === 'failed' ? 'bg-red-50 text-red-600 dark:bg-red-900/20' : 'bg-gray-50 text-gray-600 dark:bg-gray-800/50'">
                <UIcon :name="activeTask.status === 'failed' ? 'i-lucide:alert-circle' : 'i-lucide:info'" class="mt-0.5 shrink-0" />
                <div class="break-all font-mono text-xs">{{ activeTask.completion_reason }}</div>
             </div>
           </div>

           <!-- Articles List -->
           <div class="flex-1 overflow-y-auto p-6">
             <div class="space-y-4 max-w-4xl mx-auto">
               <div 
                 v-for="article in activeArticles" 
                 :key="article.id"
                 class="border dark:border-gray-800 rounded-lg p-4 hover:shadow-md transition-shadow group"
               >
                 <div class="flex justify-between items-start gap-4">
                   <div>
                     <!-- Valid clickable title for preview -->
                     <button @click="openArticlePreview(article)" class="text-lg font-semibold hover:text-blue-500 hover:underline text-left">
                       {{ article.title }}
                     </button>
                     
                     <div class="flex items-center gap-3 text-sm text-gray-500 mt-1">
                       <!-- Account Name with Popover -->
                       <UPopover v-if="article.account_name" mode="hover">
                         <span class="bg-gray-100 dark:bg-gray-800 px-2 py-0.5 rounded text-xs cursor-pointer hover:bg-gray-200 dark:hover:bg-gray-700">
                           {{ article.account_name }}
                         </span>
                         <template #panel>
                           <div class="p-2 flex flex-col gap-1 w-32">
                             <!-- <UButton 
                               size="2xs" 
                               color="gray" 
                               variant="ghost" 
                               icon="i-lucide:plus" 
                               @click="addToMonitored(article.account_fakeid!, article.account_name!)"
                               :disabled="!article.account_fakeid"
                             >
                               添加到监控
                             </UButton> -->
                             <UButton 
                               size="2xs" 
                               color="gray" 
                               variant="ghost" 
                               icon="i-lucide:search"
                               @click="appendInsight(article.account_fakeid!, article.account_name!)"
                               :disabled="!article.account_fakeid"
                             >
                               追加洞察
                             </UButton>
                           </div>
                         </template>
                       </UPopover>
                       <span v-else class="bg-gray-100 dark:bg-gray-800 px-2 py-0.5 rounded text-xs text-gray-400">
                         未知公众号
                       </span>

                       <span>{{ article.publish_time ? dayjs.unix(article.publish_time).format('YYYY-MM-DD') : '' }}</span>
                       <span class="text-orange-500">相似度: {{ (article.similarity || 0).toFixed(2) }}</span>
                     </div>
                   </div>
                 </div>
                 
                 <div class="mt-3 p-3 bg-fuchsia-50 dark:bg-fuchsia-900/10 rounded-lg text-sm text-slate-700 dark:text-slate-300">
                   <div class="font-semibold text-fuchsia-600 dark:text-fuchsia-400 flex items-center gap-1 mb-1">
                     <UIcon name="i-lucide:sparkles" class="size-4" />
                     AI 洞察
                   </div>
                   {{ article.insight || '分析中...' }}
                 </div>
               </div>
               
               <!-- Empty States (Same as before) -->
               <div v-if="activeArticles.length === 0" class="text-center py-20 text-gray-500">
                 <!-- ... copied logic ... -->
                 <template v-if="activeTask.status === 'processing'">
                   <UIcon name="i-lucide:loader-2" class="size-12 mx-auto mb-4 animate-spin text-blue-500" />
                   <p>AI 正在全网搜寻中... ({{ activeTask.processed_count }} / {{ activeTask.target_count }})</p>
                 </template>
                 <template v-else-if="activeTask.status === 'completed'">
                   <UIcon name="i-lucide:inbox" class="size-12 mx-auto mb-4 opacity-20" />
                   <p>任务已完成，未找到符合条件的文章。</p>
                 </template>
                  <template v-else>
                    <UIcon name="i-lucide:clock" class="size-12 mx-auto mb-4 opacity-20" />
                    <p>等待开始...</p>
                 </template>
               </div>
             </div>
           </div>
        </template>
        <div v-else class="flex-1 flex items-center justify-center text-gray-400">
          <div class="text-center">
            <UIcon name="i-lucide:globe" class="size-16 mx-auto mb-4 opacity-20" />
            <p>请选择或创建一个洞察任务</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Create Modal -->
    <UModal v-model="isCreateModalOpen">
      <UCard>
        <template #header>
          <div class="font-bold text-lg">
            {{ createForm.specific_account_name ? `追加洞察: ${createForm.specific_account_name}` : '新建全网洞察任务' }}
          </div>
        </template>

        <form @submit.prevent="createTask" class="space-y-4">
          <UFormGroup label="洞察意图 (Prompt)" required>
            <UTextarea 
              v-model="createForm.prompt" 
              placeholder="例如：我最近对不良资产感兴趣，想了解一下不良资产的赚钱原理" 
              :rows="4"
              autofocus
            />
          </UFormGroup>
          
          <UFormGroup label="目标文章数量">
            <UInput v-model="createForm.target_count" type="number" />
          </UFormGroup>
          
          <div v-if="createForm.specific_account_name" class="text-xs text-gray-500 bg-gray-50 p-2 rounded">
             <UIcon name="i-lucide:info" class="inline-block mr-1" />
             将只分析公众号 <b>{{ createForm.specific_account_name }}</b> 的历史文章。
          </div>
          
          <!-- Advanced LLM Config (Collapsible) -->
          <div class="border rounded-lg overflow-hidden">
            <button 
              type="button"
              class="w-full p-3 flex items-center justify-between bg-gray-50 dark:bg-gray-800 hover:bg-gray-100 dark:hover:bg-gray-700 transition-colors"
              @click="showAdvancedConfig = !showAdvancedConfig"
            >
              <span class="text-sm font-medium flex items-center gap-2">
                <UIcon name="i-lucide:settings-2" class="size-4" />
                高级配置
              </span>
              <UIcon 
                :name="showAdvancedConfig ? 'i-lucide:chevron-up' : 'i-lucide:chevron-down'" 
                class="size-4 text-gray-400"
              />
            </button>
            
            <div v-show="showAdvancedConfig" class="p-4 space-y-4 border-t dark:border-gray-700">
              <p class="text-xs text-gray-500 mb-3">
                选择每个环节使用的 AI 模型。默认全部使用 Gemini。
              </p>
              
              <div class="grid grid-cols-3 gap-4">
                <div>
                  <label class="block text-xs font-medium mb-1.5 text-gray-600">关键词生成</label>
                  <USelectMenu
                    v-model="createForm.keywordProvider"
                    :options="[
                      { value: 'gemini', label: 'Gemini' },
                      { value: 'deepseek', label: 'DeepSeek' }
                    ]"
                    value-attribute="value"
                    option-attribute="label"
                    size="sm"
                  />
                </div>
                
                <div>
                  <label class="block text-xs font-medium mb-1.5 text-gray-600">文章筛选</label>
                  <USelectMenu
                    v-model="createForm.reasoningProvider"
                    :options="[
                      { value: 'gemini', label: 'Gemini' },
                      { value: 'deepseek', label: 'DeepSeek' }
                    ]"
                    value-attribute="value"
                    option-attribute="label"
                    size="sm"
                  />
                </div>
                
                <div>
                  <label class="block text-xs font-medium mb-1.5 text-gray-600">Embedding</label>
                  <USelectMenu
                    v-model="createForm.embeddingProvider"
                    :options="[
                      { value: 'gemini', label: 'Gemini' },
                      { value: 'ollama', label: 'Ollama (本地)', disabled: !config.ollamaEnabled }
                    ]"
                    value-attribute="value"
                    option-attribute="label"
                    size="sm"
                  />
                </div>
              </div>
              
              <!-- Search Speed -->
              <div class="border-t dark:border-gray-700 pt-4 mt-4">
                <label class="block text-xs font-medium mb-2 text-gray-600">搜索速度 / 风险等级</label>
                <div class="grid grid-cols-3 gap-2">
                  <button
                    type="button"
                    :class="[
                      'p-2 rounded-lg border text-center text-sm transition-all',
                      createForm.searchSpeed === 'high' 
                        ? 'bg-red-50 border-red-300 text-red-700 dark:bg-red-900/20 dark:border-red-700 dark:text-red-400' 
                        : 'bg-gray-50 border-gray-200 text-gray-600 hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700'
                    ]"
                    @click="createForm.searchSpeed = 'high'"
                  >
                    <div class="font-medium">高速</div>
                    <div class="text-xs opacity-70">0.5秒/次</div>
                    <div class="text-xs text-red-500 mt-0.5">高风险</div>
                  </button>
                  <button
                    type="button"
                    :class="[
                      'p-2 rounded-lg border text-center text-sm transition-all',
                      createForm.searchSpeed === 'medium' 
                        ? 'bg-amber-50 border-amber-300 text-amber-700 dark:bg-amber-900/20 dark:border-amber-700 dark:text-amber-400' 
                        : 'bg-gray-50 border-gray-200 text-gray-600 hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700'
                    ]"
                    @click="createForm.searchSpeed = 'medium'"
                  >
                    <div class="font-medium">中速</div>
                    <div class="text-xs opacity-70">1-2秒/次</div>
                    <div class="text-xs text-amber-500 mt-0.5">中风险</div>
                  </button>
                  <button
                    type="button"
                    :class="[
                      'p-2 rounded-lg border text-center text-sm transition-all',
                      createForm.searchSpeed === 'low' 
                        ? 'bg-green-50 border-green-300 text-green-700 dark:bg-green-900/20 dark:border-green-700 dark:text-green-400' 
                        : 'bg-gray-50 border-gray-200 text-gray-600 hover:bg-gray-100 dark:bg-gray-800 dark:border-gray-700'
                    ]"
                    @click="createForm.searchSpeed = 'low'"
                  >
                    <div class="font-medium">慢速</div>
                    <div class="text-xs opacity-70">2-3秒/次</div>
                    <div class="text-xs text-green-500 mt-0.5">低风险</div>
                  </button>
                </div>
                <p class="text-xs text-gray-400 mt-2">
                  速度越快，被微信限制的风险越高。建议首次使用选择「慢速」。
                </p>
              </div>
              
              <p v-if="!config.ollamaEnabled" class="text-xs text-amber-600 bg-amber-50 dark:bg-amber-900/20 p-2 rounded">
                <UIcon name="i-lucide:info" class="inline-block mr-1" />
                Ollama 未启用。前往 <NuxtLink to="/dashboard/ai" class="underline">AI 配置</NuxtLink> 启用本地模型。
              </p>
            </div>
          </div>

          <div class="pt-2 flex justify-end gap-2">
            <UButton color="gray" variant="ghost" @click="isCreateModalOpen = false">取消</UButton>
            <UButton type="submit" color="black" :loading="isCreating">开始任务</UButton>
          </div>
        </form>
      </UCard>
    </UModal>
    
    <!-- Article Preview Drawer -->
    <USlideover v-model="isPreviewOpen" :ui="{ width: 'max-w-4xl' }">
       <div class="flex flex-col h-full bg-white dark:bg-gray-900">
          <div class="p-4 border-b flex justify-between items-center bg-gray-50 dark:bg-gray-800">
             <h3 class="font-bold text-lg truncate flex-1 pr-4">{{ activeArticle?.title }}</h3>
             <UButton icon="i-lucide:x" color="gray" variant="ghost" @click="isPreviewOpen = false" />
          </div>
          
          <div class="flex-1 overflow-y-auto p-6 relative">
            <!-- Loading State -->
            <div v-if="isArticleLoading" class="absolute inset-0 flex items-center justify-center bg-white/80 dark:bg-gray-900/80 z-10 transition-opacity">
               <div class="flex flex-col items-center gap-3">
                 <UIcon name="i-lucide:loader-2" class="animate-spin size-8 text-primary-500" />
                 <p class="text-sm font-medium text-gray-700 dark:text-gray-200">正在拉取文章内容...</p>
                 <p class="text-xs text-gray-400">系统将通过代理节点尝试获取全文</p>
               </div>
            </div>

            <!-- Insight Summary Block -->
             <div v-if="activeArticle?.insight" class="mb-6 p-4 bg-primary-50/50 dark:bg-primary-950/30 rounded-lg border border-primary-100 dark:border-primary-900">
               <h4 class="font-semibold text-primary-700 dark:text-primary-400 mb-2 flex items-center gap-2">
                 <UIcon name="i-lucide:sparkles" class="size-4" />
                 AI 洞察
               </h4>
               <p class="text-sm text-gray-700 dark:text-gray-300 leading-relaxed">
                 {{ activeArticle.insight }}
               </p>
             </div>
             
             <!-- Main Content -->
             <div class="prose dark:prose-invert max-w-none prose-img:rounded-lg prose-a:text-primary-600" v-html="articleContent"></div> 
             <!-- Meta Info if content fails or is empty, show original meta -->
             <div v-if="!articleContent && !isArticleLoading" class="mt-8 pt-8 border-t text-sm text-gray-500 grid grid-cols-2 gap-4">
                 <div>
                   <span class="block text-xs text-gray-400 mb-1">公众号</span>
                   {{ activeArticle?.account_name || 'Unknown' }}
                 </div>
                 <div>
                   <span class="block text-xs text-gray-400 mb-1">发布时间</span>
                   {{ formatDate(activeArticle?.publish_time || 0) }}
                 </div>
             </div>
          </div>
          
          <div class="p-4 border-t flex justify-end gap-2 bg-gray-50 dark:bg-gray-800">
              <UButton v-if="activeArticle?.url" :to="activeArticle.url" target="_blank" variant="ghost" icon="i-lucide:external-link">跳转原文</UButton>
              
              <UDropdown :items="[[
                { label: '导出 Markdown', icon: 'i-lucide:file-text', click: () => exportMarkdown() },
                { label: '导出 PDF', icon: 'i-lucide:file-code', click: () => exportPDF() }
              ]]">
                <UButton variant="soft" icon="i-lucide:download" :loading="isExporting">导出</UButton>
              </UDropdown>

              <UButton @click="isPreviewOpen = false">关闭</UButton>
          </div>
       </div>
    </USlideover>


    <!-- Export Modal -->
    <UModal v-model="isExportModalOpen">
      <div class="p-6">
        <h3 class="text-lg font-bold mb-4">批量导出任务数据</h3>
        
        <div v-if="failedResult" class="mb-4 p-3 bg-red-50 text-red-600 rounded-md text-sm border border-red-200">
           <div class="font-bold flex items-center gap-1 mb-1">
             <UIcon name="i-lucide:alert-circle" class="size-4" />
             导出失败
           </div>
           <div class="break-all whitespace-pre-wrap">{{ failedResult }}</div>
        </div>

        <form @submit.prevent="submitBatchExport" class="space-y-4">
          <UFormGroup label="导出目录 (绝对路径)" required>
             <UInput v-model="exportForm.target_dir" placeholder="例如: C:\Users\name\Desktop" />
             <p class="text-xs text-gray-400 mt-1">
               <span class="block">后端服务必须有权限写入该目录。</span>
               <span class="block mt-1 text-orange-500 font-medium">🐳 Docker 用户提示：请填写 "exports" 或 "/app/exports"，文件将导出到项目根目录的 exports 文件夹中。</span>
             </p>
          </UFormGroup>
          
          <UFormGroup label="导出格式">
            <div class="flex gap-4">
              <URadio v-model="exportForm.format" value="markdown" label="Markdown + 图片" />
              <URadio v-model="exportForm.format" value="pdf" label="PDF 文档" />
            </div>
             <p class="text-xs text-gray-400 mt-1">所有模式均会自动下载图片到本地 images 目录，并生成包含图片的文档。</p>
          </UFormGroup>
          
          <div v-if="isExportingBatch" class="mt-4 p-3 bg-blue-50 text-blue-600 rounded-md text-sm border border-blue-100 flex items-center gap-2">
             <UIcon name="i-lucide:loader-2" class="size-4 animate-spin" />
             <span>{{ isPrefetching ? 'Step 1/2: 正在预取文章与图片资源（这也将用于导出）...' : 'Step 2/2: 正在后台并行处理导出任务，请勿关闭页面...' }}</span>
          </div>

          <div class="pt-2 flex justify-end gap-2">
            <UButton color="gray" variant="ghost" @click="isExportModalOpen = false" :disabled="isExportingBatch">取消</UButton>
            <UButton type="submit" color="black" :loading="isExportingBatch" :disabled="isExportingBatch">
                {{ isExportingBatch ? (isPrefetching ? '正在预取...' : '正在导出...') : '开始导出' }}
            </UButton>
          </div>
        </form>
      </div>
    </UModal>
    
    <!-- Prefetch Result Modal -->
    <UModal v-model="isPrefetchResultModalOpen">
      <UCard>
        <template #header>
          <div class="font-bold text-lg flex items-center gap-2">
            <UIcon name="i-lucide:check-circle" class="text-green-500" />
            预取任务完成
          </div>
        </template>
        <div class="p-4 space-y-4" v-if="prefetchStats">
           <div class="grid grid-cols-2 gap-4">
              <div class="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg text-center">
                 <div class="text-sm text-gray-500 mb-1">文章处理</div>
                 <div class="font-bold text-xl flex justify-center items-center gap-2">
                    <span class="text-green-600">{{ prefetchStats.article_success }}</span>
                    <span class="text-gray-300">/</span>
                    <span class="text-red-500" v-if="prefetchStats.article_failed > 0">{{ prefetchStats.article_failed }}</span>
                    <span class="text-gray-500" v-else>0</span>
                 </div>
                 <div class="text-xs text-gray-400 mt-1">成功 / 失败</div>
              </div>
              <div class="bg-gray-50 dark:bg-gray-800 p-3 rounded-lg text-center">
                 <div class="text-sm text-gray-500 mb-1">图片压缩</div>
                 <div class="font-bold text-xl flex justify-center items-center gap-2">
                    <span class="text-green-600">{{ prefetchStats.image_success }}</span>
                    <span class="text-gray-300">/</span>
                    <span class="text-red-500" v-if="prefetchStats.image_failed > 0">{{ prefetchStats.image_failed }}</span>
                    <span class="text-gray-500" v-else>0</span>
                 </div>
                 <div class="text-xs text-gray-400 mt-1">成功 / 失败</div>
              </div>
           </div>
           
           <div v-if="prefetchStats.article_failed > 0 || prefetchStats.image_failed > 0" class="text-sm text-amber-600 p-2 bg-amber-50 rounded">
              <UIcon name="i-lucide:alert-triangle" class="inline mb-0.5 mr-1"/>
              部分资源下载失败，您可以点击重试按钮再次尝试。
           </div>
        </div>
        <template #footer>
           <div class="flex justify-end gap-2">
             <UButton color="gray" @click="isPrefetchResultModalOpen = false">关闭</UButton>
             <UButton 
                v-if="prefetchStats && (prefetchStats.article_failed > 0 || prefetchStats.image_failed > 0)"
                color="amber" 
                :loading="isPrefetching"
                @click="currentPrefetchTask && prefetchTask(currentPrefetchTask, false)" 
             >
                重试预取 (Full Scan)
             </UButton>
           </div>
        </template>
      </UCard>
    </UModal>



    <!-- Login Required Modal -->
    <UModal v-model="showLoginRequiredModal">
      <UCard>
        <template #header>
          <div class="font-bold flex items-center gap-2 text-yellow-600 dark:text-yellow-400">
            <UIcon name="i-lucide:log-in" class="size-5" />
            需要登录
          </div>
        </template>
        
        <div class="text-center py-4">
          <UIcon name="i-lucide:user-circle" class="size-16 text-gray-300 dark:text-gray-600 mb-4" />
          <p class="text-gray-700 dark:text-gray-300 mb-2">
            您需要先登录微信公众平台才能创建洞察任务
          </p>
          <p class="text-sm text-gray-500">
            请点击页面右上角的「登录」按钮进行扫码登录
          </p>
        </div>
        
        <template #footer>
          <div class="flex justify-center">
            <UButton color="black" @click="showLoginRequiredModal = false">
              我知道了
            </UButton>
          </div>
        </template>
      </UCard>
    </UModal>
  </div>
</template>
