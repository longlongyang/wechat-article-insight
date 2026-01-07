<script setup lang="ts">
import type { ICellRendererParams } from 'ag-grid-community';
import { Loader } from 'lucide-vue-next';

interface Props {
  params: ICellRendererParams & {
    onSync?: (params: ICellRendererParams) => void;
    onStop?: (params: ICellRendererParams) => void;
    onViewArticles?: (params: ICellRendererParams) => void;
    onSyncAndDownload?: (params: ICellRendererParams) => void;
    isDeleting: boolean;
    isSyncing: boolean;
    syncingRowId: string | null;
  };
}
const props = defineProps<Props>();

function sync() {
  props.params.onSync && props.params.onSync(props.params);
}
function syncAndDownload() {
  props.params.onSyncAndDownload && props.params.onSyncAndDownload(props.params);
}
function stop() {
  props.params.onStop && props.params.onStop(props.params);
}
function viewArticles() {
  props.params.onViewArticles && props.params.onViewArticles(props.params);
}
const isDisabled = computed(() => props.params.isDeleting || props.params.isSyncing);
const isLoading = computed(() => props.params.isSyncing && props.params.node.id === props.params.syncingRowId);
</script>

<template>
  <div class="flex items-center justify-center gap-1">
    <UButton
      icon="i-heroicons-list-bullet-20-solid"
      color="green"
      size="xs"
      variant="solid"
      title="查看文章"
      :disabled="isDisabled"
      @click="viewArticles"
    ></UButton>
    <UButton v-if="isLoading" color="green" size="xs" variant="solid" @click="stop">
      <Loader :size="14" class="animate-spin" />
      停止</UButton
    >
    <UButton
      v-else
      icon="i-heroicons:arrow-path-rounded-square-20-solid"
      color="blue"
      size="xs"
      variant="solid"
      :disabled="isDisabled"
      @click="sync"
    ></UButton>
    <UButton
      icon="i-lucide:folder-down"
      color="orange"
      size="xs"
      variant="solid"
      title="同步并下载"
      :disabled="isDisabled"
      @click="syncAndDownload"
    ></UButton>
  </div>
</template>
