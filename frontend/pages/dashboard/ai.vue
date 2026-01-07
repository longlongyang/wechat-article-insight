<script setup lang="ts">
import {
  DEEPSEEK_MODELS,
  GEMINI_MODELS,
  OLLAMA_CHAT_MODELS,
  OLLAMA_EMBEDDING_MODELS,
  useLLMConfig,
} from '~/composables/useLLMConfig';
import { websiteName } from '~/config';

const { rustPost } = useRustBackend();

const { isActive } = usePageActive();

useHead({
  title: `系统设置 | ${websiteName}`,
});

const { config, hasApiKey } = useLLMConfig();

// Use computed for reliable template reactivity
const currentProvider = computed(() => config.value.provider);
const preferences = usePreferences();

// Web Gateway Proxy List Handling
const webGatewayList = computed({
  get: () => (preferences.value.privateProxyList || []).join('\n'),
  set: val => {
    preferences.value.privateProxyList = val
      .split('\n')
      .map(x => x.trim())
      .filter(x => x.length > 0);
  },
});

function selectProvider(provider: 'gemini' | 'deepseek' | 'openai_compatible') {
  config.value.provider = provider;
}

// API Key 显示控制
const showGeminiKey = ref(false);
const showDeepseekKey = ref(false);
const showOpenaiCompatibleKey = ref(false);

// 测试连接状态
const testStatus = ref<'idle' | 'testing' | 'success' | 'failed'>('idle');
const testMessage = ref('');

// Ollama 测试状态
const ollamaTestStatus = ref<'idle' | 'testing' | 'success' | 'failed'>('idle');
const ollamaTestMessage = ref('');

// 测试 API 连接（通过后端代理）
async function testConnection() {
  testStatus.value = 'testing';
  const useProxy =
    config.value.provider === 'gemini' ? config.value.geminiProxyEnabled : config.value.deepseekProxyEnabled;
  testMessage.value = useProxy ? '正在通过代理测试连接...' : '正在测试连接...';

  try {
    // 使用后端 API 进行测试（支持代理）
    const result = await rustPost<{ success: boolean; message: string }>('/api/llm/test', {
      provider: config.value.provider,
      geminiApiKey: config.value.geminiApiKey,
      geminiModel: config.value.geminiModel,
      geminiProxyEnabled: config.value.geminiProxyEnabled,
      deepseekApiKey: config.value.deepseekApiKey,
      deepseekModel: config.value.deepseekModel,
      deepseekProxyEnabled: config.value.deepseekProxyEnabled,
      openaiCompatibleBaseUrl: config.value.openaiCompatibleBaseUrl,
      openaiCompatibleApiKey: config.value.openaiCompatibleApiKey,
      openaiCompatibleModel: config.value.openaiCompatibleModel,
      openaiCompatibleProxyEnabled: config.value.openaiCompatibleProxyEnabled,
      proxyHost: config.value.proxyHost,
      proxyPort: config.value.proxyPort,
      proxyUsername: config.value.proxyUsername,
      proxyPassword: config.value.proxyPassword,
    });

    if (result.success) {
      testStatus.value = 'success';
      testMessage.value = result.message;
    } else {
      testStatus.value = 'failed';
      testMessage.value = result.message;
    }
  } catch (error: any) {
    testStatus.value = 'failed';
    testMessage.value = `✗ 连接失败: ${error.message || error.data?.message || '未知错误'}`;
  }
}

// 测试 Ollama 连接
async function testOllamaConnection() {
  ollamaTestStatus.value = 'testing';
  ollamaTestMessage.value = '正在连接 Ollama...';

  try {
    const result = await rustPost<{ success: boolean; message: string; models?: string[] }>('/api/llm/test-ollama', {
      baseUrl: config.value.ollamaBaseUrl,
      embeddingModel: config.value.ollamaEmbeddingModel,
    });

    if (result.success) {
      ollamaTestStatus.value = 'success';
      ollamaTestMessage.value = result.message;
    } else {
      ollamaTestStatus.value = 'failed';
      ollamaTestMessage.value = result.message;
    }
  } catch (error: any) {
    ollamaTestStatus.value = 'failed';
    ollamaTestMessage.value = `✗ 连接失败: ${error.message || error.data?.message || '无法连接 Ollama'}`;
  }
}

const tabs = [
  {
    slot: 'ai',
    label: 'AI 模型',
    icon: 'i-lucide:brain-circuit',
  },
  {
    slot: 'network',
    label: '网络服务',
    icon: 'i-lucide:network',
  },
];
</script>

<template>
  <div class="h-full">
    <Teleport v-if="isActive" defer to="#title">
      <h1 class="text-[28px] leading-[34px] text-slate-12 dark:text-slate-50 font-bold flex items-center gap-3">
        <div class="size-8 rounded-lg bg-gradient-to-tr from-indigo-500 to-violet-500 flex items-center justify-center text-white shadow-lg shadow-indigo-500/20">
          <UIcon name="i-lucide:settings-2" class="size-5" />
        </div>
        系统设置
      </h1>
    </Teleport>

    <div class="h-full overflow-auto p-6">
      <div class="max-w-3xl mx-auto">
        <UTabs :items="tabs" class="w-full">
          <template #ai>
            <div class="space-y-6 mt-4">
              <!-- Provider 选择 -->
              <UCard>
                <template #header>
                  <h3 class="font-semibold">选择 AI 服务商</h3>
                </template>
                
                <div class="grid grid-cols-3 gap-4">
                  <button 
                    type="button"
                    class="p-4 rounded-lg border-2 cursor-pointer transition-all text-left w-full relative group"
                    :class="currentProvider === 'gemini' 
                      ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20' 
                      : 'border-gray-200 dark:border-gray-700 hover:border-blue-300'"
                    @click="selectProvider('gemini')"
                  >
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-gradient-to-br from-blue-500 to-cyan-500 flex items-center justify-center shrink-0">
                        <span class="text-white font-bold">G</span>
                      </div>
                      <div>
                        <h4 class="font-semibold">Google Gemini</h4>
                        <p class="text-xs text-gray-500">免费配额</p>
                      </div>
                      <UIcon v-if="currentProvider === 'gemini'" name="i-lucide:check-circle" class="text-blue-500 ml-auto" />
                    </div>
                  </button>
                  
                  <button 
                    type="button"
                    class="p-4 rounded-lg border-2 cursor-pointer transition-all text-left w-full relative group"
                    :class="currentProvider === 'deepseek' 
                      ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20' 
                      : 'border-gray-200 dark:border-gray-700 hover:border-blue-300'"
                    @click="selectProvider('deepseek')"
                  >
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-gradient-to-br from-indigo-500 to-purple-500 flex items-center justify-center shrink-0">
                        <span class="text-white font-bold">D</span>
                      </div>
                      <div>
                        <h4 class="font-semibold">DeepSeek</h4>
                        <p class="text-xs text-gray-500">国产高性价比</p>
                      </div>
                      <UIcon v-if="currentProvider === 'deepseek'" name="i-lucide:check-circle" class="text-blue-500 ml-auto" />
                    </div>
                  </button>

                  <button 
                    type="button"
                    class="p-4 rounded-lg border-2 cursor-pointer transition-all text-left w-full relative group"
                    :class="currentProvider === 'openai_compatible' 
                      ? 'border-blue-500 bg-blue-50 dark:bg-blue-900/20' 
                      : 'border-gray-200 dark:border-gray-700 hover:border-blue-300'"
                    @click="selectProvider('openai_compatible')"
                  >
                    <div class="flex items-center gap-3">
                      <div class="w-10 h-10 rounded-lg bg-gradient-to-br from-green-500 to-teal-500 flex items-center justify-center shrink-0">
                        <span class="text-white font-bold">🔗</span>
                      </div>
                      <div>
                        <h4 class="font-semibold">OpenAI 兼容</h4>
                        <p class="text-xs text-gray-500">POE/OpenRouter</p>
                      </div>
                      <UIcon v-if="currentProvider === 'openai_compatible'" name="i-lucide:check-circle" class="text-blue-500 ml-auto" />
                    </div>
                  </button>
                </div>
              </UCard>

              <!-- Gemini 配置 -->
              <UCard v-if="currentProvider === 'gemini'">
                <template #header>
                  <h3 class="font-semibold flex items-center gap-2">
                    <span class="w-6 h-6 rounded bg-gradient-to-br from-blue-500 to-cyan-500 flex items-center justify-center text-white text-xs font-bold">G</span>
                    Gemini 配置
                  </h3>
                </template>
                
                <div class="space-y-4">
                  <div>
                    <label class="block text-sm font-medium mb-2">API Key</label>
                    <div class="flex gap-2">
                      <UInput 
                        v-model="config.geminiApiKey" 
                        :type="showGeminiKey ? 'text' : 'password'"
                        placeholder="AIza..."
                        class="flex-1 font-mono"
                      />
                      <UButton 
                        color="gray" 
                        variant="ghost"
                        :icon="showGeminiKey ? 'i-lucide:eye-off' : 'i-lucide:eye'"
                        @click="showGeminiKey = !showGeminiKey"
                      />
                    </div>
                    <p class="text-xs text-gray-500 mt-2">
                      从 <a href="https://aistudio.google.com/apikey" target="_blank" class="text-blue-500 hover:underline">Google AI Studio</a> 获取 API Key
                    </p>
                  </div>
                  
                  <div>
                    <label class="block text-sm font-medium mb-2">模型</label>
                    <USelectMenu
                      v-model="config.geminiModel"
                      :options="GEMINI_MODELS"
                      value-attribute="value"
                      option-attribute="label"
                      class="w-full"
                    />
                  </div>
                  
                  <!-- Gemini 代理开关 -->
                  <div class="flex items-center justify-between p-3 bg-orange-50 dark:bg-orange-900/20 rounded-lg mt-4">
                    <div class="flex items-center gap-2">
                      <UIcon name="i-lucide:globe" class="size-4 text-orange-500" />
                      <span class="text-sm font-medium">使用代理</span>
                      <span class="text-xs text-gray-500">(访问 Google API 通常需要代理)</span>
                    </div>
                    <UToggle v-model="config.geminiProxyEnabled" />
                  </div>
                  
                  <!-- Gemini 测试连接 -->
                  <div class="pt-2 flex items-center gap-3">
                    <UButton 
                      color="blue" 
                      variant="soft"
                      size="sm"
                      :loading="testStatus === 'testing' && currentProvider === 'gemini'"
                      :disabled="!config.geminiApiKey"
                      @click="testConnection"
                    >
                      <UIcon name="i-lucide:wifi" class="mr-1" />
                      测试连接
                    </UButton>
                    <span v-if="testMessage && currentProvider === 'gemini'" 
                      class="text-sm"
                      :class="{
                        'text-green-600': testStatus === 'success',
                        'text-red-600': testStatus === 'failed',
                        'text-gray-500': testStatus === 'testing',
                      }"
                    >
                      {{ testMessage }}
                    </span>
                  </div>
                </div>
              </UCard>

              <!-- DeepSeek 配置 -->
              <UCard v-if="currentProvider === 'deepseek'">
                <template #header>
                  <h3 class="font-semibold flex items-center gap-2">
                    <span class="w-6 h-6 rounded bg-gradient-to-br from-indigo-500 to-purple-500 flex items-center justify-center text-white text-xs font-bold">D</span>
                    DeepSeek 配置
                  </h3>
                </template>
                
                <div class="space-y-4">
                  <div>
                    <label class="block text-sm font-medium mb-2">API Key</label>
                    <div class="flex gap-2">
                      <UInput 
                        v-model="config.deepseekApiKey" 
                        :type="showDeepseekKey ? 'text' : 'password'"
                        placeholder="sk-..."
                        class="flex-1 font-mono"
                      />
                      <UButton 
                        color="gray" 
                        variant="ghost"
                        :icon="showDeepseekKey ? 'i-lucide:eye-off' : 'i-lucide:eye'"
                        @click="showDeepseekKey = !showDeepseekKey"
                      />
                    </div>
                    <p class="text-xs text-gray-500 mt-2">
                      从 <a href="https://platform.deepseek.com/api_keys" target="_blank" class="text-blue-500 hover:underline">DeepSeek Platform</a> 获取 API Key
                    </p>
                  </div>
                  
                  <div>
                    <label class="block text-sm font-medium mb-2">模型</label>
                    <USelectMenu
                      v-model="config.deepseekModel"
                      :options="DEEPSEEK_MODELS"
                      value-attribute="value"
                      option-attribute="label"
                      class="w-full"
                    />
                  </div>
                  
                  <!-- DeepSeek 代理开关 -->
                  <div class="flex items-center justify-between p-3 bg-gray-50 dark:bg-gray-800 rounded-lg mt-4">
                    <div class="flex items-center gap-2">
                      <UIcon name="i-lucide:globe" class="size-4 text-gray-500" />
                      <span class="text-sm font-medium">使用代理</span>
                      <span class="text-xs text-gray-500">(通常不需要)</span>
                    </div>
                    <UToggle v-model="config.deepseekProxyEnabled" />
                  </div>
                  
                  <!-- DeepSeek 测试连接 -->
                  <div class="pt-2 flex items-center gap-3">
                    <UButton 
                      color="indigo" 
                      variant="soft"
                      size="sm"
                      :loading="testStatus === 'testing' && currentProvider === 'deepseek'"
                      :disabled="!config.deepseekApiKey"
                      @click="testConnection"
                    >
                      <UIcon name="i-lucide:wifi" class="mr-1" />
                      测试连接
                    </UButton>
                    <span v-if="testMessage && currentProvider === 'deepseek'" 
                      class="text-sm"
                      :class="{
                        'text-green-600': testStatus === 'success',
                        'text-red-600': testStatus === 'failed',
                        'text-gray-500': testStatus === 'testing',
                      }"
                    >
                      {{ testMessage }}
                    </span>
                  </div>
                </div>
              </UCard>

              <!-- OpenAI-Compatible 配置 -->
              <UCard v-if="currentProvider === 'openai_compatible'">
                <template #header>
                  <h3 class="font-semibold flex items-center gap-2">
                    <span class="w-6 h-6 rounded bg-gradient-to-br from-green-500 to-teal-500 flex items-center justify-center text-white text-xs font-bold">🔗</span>
                    OpenAI 兼容 API 配置
                  </h3>
                </template>
                
                <div class="space-y-4">
                  <div>
                    <label class="block text-sm font-medium mb-2">Base URL</label>
                    <UInput 
                      v-model="config.openaiCompatibleBaseUrl" 
                      placeholder="https://api.poe.com/v1"
                      class="font-mono"
                    />
                    <p class="text-xs text-gray-500 mt-2">
                      API 端点地址，如 POE: <code class="bg-gray-100 dark:bg-gray-800 px-1 rounded">https://api.poe.com/v1</code>
                    </p>
                  </div>
                  
                  <div>
                    <label class="block text-sm font-medium mb-2">API Key</label>
                    <div class="flex gap-2">
                      <UInput 
                        v-model="config.openaiCompatibleApiKey" 
                        :type="showOpenaiCompatibleKey ? 'text' : 'password'"
                        placeholder="your_api_key"
                        class="flex-1 font-mono"
                      />
                      <UButton 
                        color="gray" 
                        variant="ghost"
                        :icon="showOpenaiCompatibleKey ? 'i-lucide:eye-off' : 'i-lucide:eye'"
                        @click="showOpenaiCompatibleKey = !showOpenaiCompatibleKey"
                      />
                    </div>
                  </div>
                  
                  <div>
                    <label class="block text-sm font-medium mb-2">模型名称</label>
                    <UInput 
                      v-model="config.openaiCompatibleModel" 
                      placeholder="Claude-Sonnet-4"
                      class="font-mono"
                    />
                    <p class="text-xs text-gray-500 mt-2">
                      模型名称取决于服务商，如 POE: <code class="bg-gray-100 dark:bg-gray-800 px-1 rounded">Claude-Sonnet-4</code>、<code class="bg-gray-100 dark:bg-gray-800 px-1 rounded">Gemini-2.5-Pro</code>
                    </p>
                  </div>
                  
                  <!-- OpenAI-compatible 代理开关 -->
                  <div class="flex items-center justify-between p-3 bg-orange-50 dark:bg-orange-900/20 rounded-lg mt-4">
                    <div class="flex items-center gap-2">
                      <UIcon name="i-lucide:globe" class="size-4 text-orange-500" />
                      <span class="text-sm font-medium">使用代理</span>
                      <span class="text-xs text-gray-500">(访问海外服务通常需要代理)</span>
                    </div>
                    <UToggle v-model="config.openaiCompatibleProxyEnabled" />
                  </div>
                  
                  <!-- OpenAI-compatible 测试连接 -->
                  <div class="pt-2 flex items-center gap-3">
                    <UButton 
                      color="green" 
                      variant="soft"
                      size="sm"
                      :loading="testStatus === 'testing' && currentProvider === 'openai_compatible'"
                      :disabled="!config.openaiCompatibleApiKey || !config.openaiCompatibleBaseUrl || !config.openaiCompatibleModel"
                      @click="testConnection"
                    >
                      <UIcon name="i-lucide:wifi" class="mr-1" />
                      测试连接
                    </UButton>
                    <span v-if="testMessage && currentProvider === 'openai_compatible'" 
                      class="text-sm"
                      :class="{
                        'text-green-600': testStatus === 'success',
                        'text-red-600': testStatus === 'failed',
                        'text-gray-500': testStatus === 'testing',
                      }"
                    >
                      {{ testMessage }}
                    </span>
                  </div>
                </div>
              </UCard>

              <!-- Ollama 本地模型配置 -->
              <UCard>
                <template #header>
                  <div class="flex items-center justify-between">
                    <h3 class="font-semibold flex items-center gap-2">
                      <span class="w-6 h-6 rounded bg-gradient-to-br from-emerald-500 to-teal-500 flex items-center justify-center text-white text-xs font-bold">O</span>
                      本地模型 (Ollama)
                    </h3>
                    <UToggle v-model="config.ollamaEnabled" />
                  </div>
                </template>
                
                <div v-if="config.ollamaEnabled" class="space-y-4">
                  <div>
                    <label class="block text-sm font-medium mb-2">Ollama 地址</label>
                    <UInput 
                      v-model="config.ollamaBaseUrl" 
                      placeholder="http://127.0.0.1:11434"
                      class="font-mono"
                    />
                    <p class="text-xs text-gray-500 mt-2">
                      确保 <a href="https://ollama.com" target="_blank" class="text-blue-500 hover:underline">Ollama</a> 已安装并运行
                    </p>
                  </div>
                  
                  <div>
                    <label class="block text-sm font-medium mb-2">Embedding 模型</label>
                    <USelectMenu
                      v-model="config.ollamaEmbeddingModel"
                      :options="OLLAMA_EMBEDDING_MODELS"
                      value-attribute="value"
                      option-attribute="label"
                      class="w-full"
                    />
                    <p class="text-xs text-gray-500 mt-2">
                      运行 <code class="bg-gray-100 dark:bg-gray-800 px-1 rounded">ollama pull {{ config.ollamaEmbeddingModel }}</code> 下载模型
                    </p>
                  </div>
                  
                  <div>
                    <label class="block text-sm font-medium mb-2">Chat 模型 (可选)</label>
                    <USelectMenu
                      v-model="config.ollamaChatModel"
                      :options="OLLAMA_CHAT_MODELS"
                      value-attribute="value"
                      option-attribute="label"
                      class="w-full"
                    />
                  </div>
                  
                  <!-- Ollama 测试连接按钮 -->
                  <div class="pt-2">
                    <UButton 
                      color="emerald" 
                      variant="soft"
                      size="sm"
                      :loading="ollamaTestStatus === 'testing'"
                      @click="testOllamaConnection"
                    >
                      <UIcon name="i-lucide:wifi" class="mr-1" />
                      测试连接
                    </UButton>
                  </div>
                  
                  <!-- Ollama 测试结果 -->
                  <div 
                    v-if="ollamaTestMessage" 
                    class="p-3 rounded-lg text-sm"
                    :class="{
                      'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400': ollamaTestStatus === 'success',
                      'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400': ollamaTestStatus === 'failed',
                      'bg-gray-50 dark:bg-gray-800 text-gray-600': ollamaTestStatus === 'testing',
                    }"
                  >
                    {{ ollamaTestMessage }}
                  </div>
                </div>
                
                <div v-else class="text-center py-4 text-gray-500 text-sm">
                  <p>启用后可使用本地模型进行 Embedding</p>
                  <p class="text-xs mt-1">不启用时，Embedding 将使用 Gemini API</p>
                </div>
              </UCard>

              <!-- API 代理配置 -->
              <UCard v-if="(currentProvider === 'gemini' && config.geminiProxyEnabled) || (currentProvider === 'deepseek' && config.deepseekProxyEnabled)">
                <template #header>
                  <h3 class="font-semibold flex items-center gap-2">
                    <UIcon name="i-lucide:globe" class="size-5 text-orange-500" />
                    API 代理服务器
                  </h3>
                </template>
                
                <p class="text-sm text-gray-500 mb-4">
                  配置用于连接 LLM API 的代理服务器
                </p>
                
                <div class="space-y-4">
                  <div class="grid grid-cols-3 gap-4">
                    <div class="col-span-2">
                      <label class="block text-sm font-medium mb-2">代理地址</label>
                      <UInput 
                        v-model="config.proxyHost" 
                        placeholder="127.0.0.1"
                        class="font-mono"
                      />
                    </div>
                    <div>
                      <label class="block text-sm font-medium mb-2">端口</label>
                      <UInput 
                        v-model.number="config.proxyPort" 
                        type="number"
                        placeholder="7890"
                        class="font-mono"
                      />
                    </div>
                  </div>
                  
                  <div class="grid grid-cols-2 gap-4">
                    <div>
                      <label class="block text-sm font-medium mb-2">用户名 (可选)</label>
                      <UInput 
                        v-model="config.proxyUsername" 
                        placeholder="可选"
                        class="font-mono"
                      />
                    </div>
                    <div>
                      <label class="block text-sm font-medium mb-2">密码 (可选)</label>
                      <UInput 
                        v-model="config.proxyPassword" 
                        type="password"
                        placeholder="可选"
                        class="font-mono"
                      />
                    </div>
                  </div>
                </div>
              </UCard>

              <!-- 注意事项 -->
              <UAlert
                color="blue"
                icon="i-lucide:info"
                title="使用提示"
              >
                <template #description>
                  <ul class="list-disc list-inside text-sm space-y-1 mt-2">
                    <li>API Key 仅保存在本地浏览器中，不会上传到服务器</li>
                    <li>Gemini 免费版有配额限制，适合轻度使用</li>
                    <li>DeepSeek 按量计费，中文场景性价比高</li>
                    <li>建议在使用前先点击「测试连接」验证配置</li>
                  </ul>
                </template>
              </UAlert>
            </div>
          </template>

          <template #network>
            <div class="space-y-6 mt-4">
              <!-- 文章下载代理 (Web Gateway) -->
              <UCard>
                <template #header>
                  <h3 class="font-semibold flex items-center gap-2">
                    <span class="w-6 h-6 rounded bg-gradient-to-br from-green-500 to-emerald-500 flex items-center justify-center text-white text-xs font-bold">W</span>
                    文章下载代理 (Web Gateway)
                  </h3>
                </template>

                <div class="space-y-4">
                  <div>
                    <div class="flex items-center justify-between mb-2">
                      <label class="block text-sm font-medium">代理节点列表</label>
                      <div class="text-xs text-gray-500">一行一个 URL</div>
                    </div>
                    
                    <UTextarea 
                      v-model="webGatewayList" 
                      :rows="3"
                      placeholder="https://my-worker.username.workers.dev/&#10;https://another-gateway.vercel.app/api/proxy"
                      class="font-mono text-sm leading-6"
                    />
                    <p class="text-xs text-gray-500 mt-2">
                      用于下载微信文章内容和图片。这些节点必须支持透明代理格式：<code>https://node.com/?url=target_url</code>
                    </p>
                  </div>

                  <div>
                    <label class="block text-sm font-medium mb-2">认证密钥 (可选)</label>
                    <UInput 
                      v-model="preferences.privateProxyAuthorization" 
                      type="password"
                      placeholder="Authorization Header Value"
                      class="font-mono"
                    />
                    <p class="text-xs text-gray-500 mt-2">
                      如果您的代理节点需要鉴权（例如 Cloudflare Workers 验证），请在此输入 Authorization 头的值。
                    </p>
                  </div>
                </div>
              </UCard>
            </div>
          </template>
        </UTabs>
      </div>
    </div>
  </div>
</template>
