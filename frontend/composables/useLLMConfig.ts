/**
 * 统一的 LLM 配置管理
 * 用于 Gideon、Doppelganger 等需要 LLM API 的功能
 */

export interface LLMConfig {
  provider: 'gemini' | 'deepseek' | 'openai_compatible';
  // Gemini
  geminiApiKey: string;
  geminiModel: string;
  geminiProxyEnabled: boolean;
  // DeepSeek
  deepseekApiKey: string;
  deepseekModel: string;
  deepseekProxyEnabled: boolean;
  // OpenAI-Compatible
  openaiCompatibleBaseUrl: string;
  openaiCompatibleApiKey: string;
  openaiCompatibleModel: string;
  openaiCompatibleProxyEnabled: boolean;
  // Ollama (Local)
  ollamaEnabled: boolean;
  ollamaBaseUrl: string;
  ollamaEmbeddingModel: string;
  ollamaChatModel: string;
  // 共享代理配置
  proxyHost: string;
  proxyPort: number;
  proxyUsername?: string;
  proxyPassword?: string;
}

export const DEFAULT_LLM_CONFIG: LLMConfig = {
  provider: 'gemini',
  geminiApiKey: '',
  geminiModel: 'gemini-3-flash-preview',
  geminiProxyEnabled: true, // Gemini 默认需要代理
  deepseekApiKey: '',
  deepseekModel: 'deepseek-chat',
  deepseekProxyEnabled: false, // DeepSeek 默认不需要代理
  openaiCompatibleBaseUrl: '',
  openaiCompatibleApiKey: '',
  openaiCompatibleModel: '',
  openaiCompatibleProxyEnabled: true, // 默认使用代理
  ollamaEnabled: false,
  ollamaBaseUrl: 'http://127.0.0.1:11434',
  ollamaEmbeddingModel: 'qwen3-embedding:8b-q8_0',
  ollamaChatModel: 'qwen3:8b',
  proxyHost: 'host.docker.internal',
  proxyPort: 7890,
  proxyUsername: '',
  proxyPassword: '',
};

export const GEMINI_MODELS = [
  { value: 'gemini-3-pro-preview', label: 'Gemini 3 Pro (最强)' },
  { value: 'gemini-3-flash-preview', label: 'Gemini 3 Flash (推荐)' },
  { value: 'gemini-2.5-flash', label: 'Gemini 2.5 Flash' },
  { value: 'gemini-2.5-pro', label: 'Gemini 2.5 Pro (高质量)' },
  { value: 'gemini-2.0-flash', label: 'Gemini 2.0 Flash' },
  { value: 'gemini-1.5-flash', label: 'Gemini 1.5 Flash (快速)' },
  { value: 'gemini-1.5-pro', label: 'Gemini 1.5 Pro' },
];

export const DEEPSEEK_MODELS = [
  { value: 'deepseek-chat', label: 'DeepSeek V3 (Chat)' },
  { value: 'deepseek-reasoner', label: 'DeepSeek R1 (Reasoner)' },
];

export const OLLAMA_EMBEDDING_MODELS = [
  { value: 'qwen3-embedding:8b-q8_0', label: 'Qwen3 Embedding 8B (推荐)' },
  { value: 'nomic-embed-text', label: 'Nomic Embed Text' },
  { value: 'mxbai-embed-large', label: 'MXBai Embed Large' },
  { value: 'bge-m3', label: 'BGE-M3 (多语言)' },
];

export const OLLAMA_CHAT_MODELS = [
  { value: 'qwen3:8b', label: 'Qwen3 8B (推荐)' },
  { value: 'qwen3:14b', label: 'Qwen3 14B' },
  { value: 'llama3.3:70b', label: 'Llama 3.3 70B' },
  { value: 'deepseek-r1:14b', label: 'DeepSeek R1 14B' },
];

/**
 * 获取 LLM 配置
 */
export function useLLMConfig() {
  const config = useLocalStorage<LLMConfig>('llm-config', DEFAULT_LLM_CONFIG, { mergeDefaults: true, deep: true });

  // 是否已配置有效的 API Key
  const hasApiKey = computed(() => {
    if (config.value.provider === 'gemini') {
      return !!config.value.geminiApiKey;
    } else if (config.value.provider === 'deepseek') {
      return !!config.value.deepseekApiKey;
    } else if (config.value.provider === 'openai_compatible') {
      return !!config.value.openaiCompatibleApiKey && !!config.value.openaiCompatibleBaseUrl;
    }
    return false;
  });

  // 当前选中的 API Key
  const currentApiKey = computed(() => {
    if (config.value.provider === 'gemini') {
      return config.value.geminiApiKey;
    } else if (config.value.provider === 'deepseek') {
      return config.value.deepseekApiKey;
    } else {
      return config.value.openaiCompatibleApiKey;
    }
  });

  // 当前选中的模型
  const currentModel = computed(() => {
    if (config.value.provider === 'gemini') {
      return config.value.geminiModel;
    } else if (config.value.provider === 'deepseek') {
      return config.value.deepseekModel;
    } else {
      return config.value.openaiCompatibleModel;
    }
  });

  // 获取 API 调用配置
  function getApiConfig() {
    const isGemini = config.value.provider === 'gemini';
    return {
      provider: config.value.provider,
      apiKey: isGemini ? config.value.geminiApiKey : config.value.deepseekApiKey,
      model: isGemini ? config.value.geminiModel : config.value.deepseekModel,
    };
  }

  return {
    config,
    hasApiKey,
    currentApiKey,
    currentModel,
    getApiConfig,
    GEMINI_MODELS,
    DEEPSEEK_MODELS,
  };
}
