import { sleep } from '#shared/utils/helpers';

/**
 * 封装 $fetch 无重试请求
 */
export const request = $fetch.create({
  retry: 0,
  method: 'GET',
  async onResponse({ request, response, options, error }) {
    // todo: 这里可以做日志记录
    // 需要注意的是，这里有可能是客户端和服务器端调用
    await sleep(50);

    // 记录 api 调用
    // await updateAPICache({
    //   name: 'appmsgpublish',
    //   account: loginAccount.value?.nickname,
    //   call_time: new Date().getTime(),
    //   is_normal: resp.base_resp.ret === 0 || resp.base_resp.ret === 200003,
    //   payload: {
    //     id: account.fakeid,
    //     begin: begin,
    //     size: ARTICLE_LIST_PAGE_SIZE,
    //     keyword: keyword,
    //   },
    // });
  },
  async onResponseError({ request, response, options, error }) {},
});
