<template>
  <UButton
    :icon="isDark ? 'i-lucide:sun' : 'i-lucide:moon'"
    color="gray"
    variant="ghost"
    size="sm"
    @click="toggleColorMode"
  />
</template>

<script setup lang="ts">
const colorMode = useColorMode();

const isDark = computed(() => colorMode.preference === 'dark');

function toggleColorMode() {
  colorMode.preference = isDark.value ? 'light' : 'dark';
}
</script>
