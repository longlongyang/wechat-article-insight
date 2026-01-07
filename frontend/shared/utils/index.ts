/**
 * url是否是合法的微信公众号文章url
 * @param url
 */
export function urlIsValidMpArticle(url: string) {
  try {
    return new URL(url).hostname === 'mp.weixin.qq.com';
  } catch (e) {
    return false;
  }
}

/**
 * 读取 File 内容
 * @param blob
 */
export async function readBlob(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onloadend = () => {
      resolve(reader.result as string);
    };
    reader.onerror = evt => {
      reject(evt);
    };
    reader.readAsText(blob);
  });
}
