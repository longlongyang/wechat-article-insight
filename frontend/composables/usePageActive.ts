/**
 * 追踪页面在 KeepAlive 中的激活状态
 * 用于解决 KeepAlive + Teleport 导致内容叠加的问题
 */
export function usePageActive() {
  const isActive = ref(true);

  onActivated(() => {
    isActive.value = true;
  });

  onDeactivated(() => {
    isActive.value = false;
  });

  return { isActive };
}
