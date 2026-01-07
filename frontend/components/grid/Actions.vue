<script setup lang="ts">
import type { ICellRendererParams } from 'ag-grid-community';
import toastFactory from '~/composables/toast';

interface Props {
  params: ICellRendererParams & {
    onGotoLink?: (params: ICellRendererParams) => void;
    onPreview?: (params: ICellRendererParams) => void;
  };
}
const props = defineProps<Props>();
const toast = toastFactory();

function gotoLink() {
  props.params.onGotoLink && props.params.onGotoLink(props.params);
}
function preview() {
  props.params.onPreview && props.params.onPreview(props.params);
}
async function copyLink() {
  const link = props.params.data?.link;
  if (link) {
    await navigator.clipboard.writeText(link);
    toast.success('已复制', '链接已复制到剪贴板，可粘贴到 PC 微信中打开');
  }
}
</script>

<template>
  <div class="flex items-center justify-center">
    <UTooltip text="复制链接" :popper="{ placement: 'top' }">
      <UButton icon="i-lucide:copy" color="orange" square variant="ghost" @click="copyLink" />
    </UTooltip>
    <UTooltip text="访问原文" :popper="{ placement: 'top' }">
      <UButton icon="i-heroicons-link-16-solid" color="blue" square variant="ghost" @click="gotoLink" />
    </UTooltip>
    <UTooltip text="预览" :popper="{ placement: 'top' }">
      <UButton
        :disabled="!params.data.contentDownload || params.data.downloading"
        icon="i-heroicons:fire-16-solid"
        color="blue"
        square
        variant="ghost"
        @click="preview"
      />
    </UTooltip>
  </div>
</template>
