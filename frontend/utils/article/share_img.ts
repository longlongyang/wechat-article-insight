// 处理图片分享消息
export function process_share_img(html: string, url: string) {
  const parser = new DOMParser();
  const document = parser.parseFromString(html, 'text/html');

  console.log(document.body.classList);
  // document.body.classList.contains('page_share_img')
}
