<template>
  <div class="flex flex-col h-screen bg-slate-50 dark:bg-slate-950">
    <!-- Top Navigation Bar -->
    <header class="h-16 flex items-center justify-between px-6 border-b border-gray-200 dark:border-gray-800 bg-white/80 dark:bg-slate-900/80 backdrop-blur-md z-50 sticky top-0">
      <!-- Left: Logo & Title -->
      <div class="flex items-center gap-3">
        <!-- Teleport target for child page titles -->
        <div id="title"></div>
      </div>

      <!-- Center: Navigation Tabs -->
      <div class="absolute left-1/2 -translate-x-1/2 hidden md:flex p-1 bg-slate-100 dark:bg-slate-800/50 rounded-full border border-slate-200 dark:border-slate-700/50">
        <NuxtLink 
          to="/dashboard/insights" 
          class="px-6 py-1.5 rounded-full text-sm font-medium transition-all duration-200 flex items-center gap-2"
          active-class="bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-sm"
          class-active="text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200"
        >
          <UIcon name="i-lucide:sparkles" class="size-4" />
          洞察
        </NuxtLink>
        <NuxtLink 
          to="/dashboard/ai" 
          class="px-6 py-1.5 rounded-full text-sm font-medium transition-all duration-200 flex items-center gap-2 text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200"
          active-class="bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-sm !text-blue-600"
        >
          <UIcon name="i-lucide:settings-2" class="size-4" />
          系统设置
        </NuxtLink>
      </div>

      <!-- Right: User & Actions -->
      <div class="flex items-center gap-3">
        <!-- Theme Toggle -->
        <ColorModeButton />

        <UDivider orientation="vertical" class="h-6 mx-1" />

        <!-- User Menu -->
        <UDropdown 
          v-if="loginAccount"
          :items="userMenuItems" 
          :poppers="{ placement: 'bottom-end' }"
        >
          <UButton color="gray" variant="ghost" class="px-2">
            <template #leading>
              <UAvatar 
                :src="loginAccount.avatar ? IMAGE_PROXY + loginAccount.avatar : undefined" 
                :alt="loginAccount.nickname"
                size="xs"
              />
            </template>
            <span class="hidden md:inline text-sm font-medium max-w-[100px] truncate">{{ loginAccount.nickname }}</span>
            <template #trailing>
              <UIcon name="i-lucide:chevron-down" class="size-4 text-gray-500" />
            </template>
          </UButton>
        </UDropdown>

        <UButton 
          v-else 
          color="black"
          size="sm"
          icon="i-lucide:log-in"
          @click="login"
        >
          登录
        </UButton>
      </div>
    </header>

    <!-- Main Content Area -->
    <main class="flex-1 overflow-hidden relative">
      <div class="h-full overflow-auto scroll-smooth">
        <NuxtPage :keepalive="{ max: 10 }" />
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import { request } from '#shared/utils/request';
import LoginModal from '~/components/modal/Login.vue';
import { IMAGE_PROXY } from '~/config';
import type { LogoutResponse } from '~/types/types';

const loginAccount = useLoginAccount();
const modal = useModal();

function login() {
  modal.open(LoginModal);
}

const userMenuItems = computed(() => [
  [
    {
      label: '退出登录',
      icon: 'i-lucide:log-out',
      click: logout,
    },
  ],
]);

async function logout() {
  const { statusCode, statusText } = await request<LogoutResponse>('/api/web/mp/logout');
  if (statusCode === 200) {
    loginAccount.value = null;
  } else {
    alert(statusText);
  }
}
</script>
