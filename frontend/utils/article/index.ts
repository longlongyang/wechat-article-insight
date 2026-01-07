/**
 * 验证文章的 html 内容是否下载完整
 * @param html
 * @return [状态，文章评论id] 二元组
 */
export function validateHTMLContent(html: string): ['Success' | 'Failure' | 'Deleted' | 'Checking', string | null] {
  const parser = new DOMParser();
  const document = parser.parseFromString(html, 'text/html');
  const $jsContent = document.querySelector('#js_content');
  const $layout = document.querySelector('#js_fullscreen_layout_padding');
  const $title = document.querySelector('head > title')!.textContent;

  let commentID = null;
  const commentIdMatchResult = html.match(/var comment_id = '(?<comment_id>\d+)' \|\| '0';/);
  if (commentIdMatchResult && commentIdMatchResult.groups && commentIdMatchResult.groups.comment_id) {
    commentID = commentIdMatchResult.groups.comment_id;
  }

  if ($jsContent) {
    return ['Success', commentID];
  }
  if ($layout || $title === '该页面不存在') {
    return ['Deleted', null];
  }
  if ($title === '内容审核中') {
    return ['Checking', null];
  }
  return ['Failure', null];
}
