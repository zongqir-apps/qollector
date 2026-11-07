import { Binary } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

/**
 * 安全解码 Base64 文本
 * 必须解码出可读 UTF-8 文本或 JSON，严防对二进制乱码误报
 * @param {string} text
 * @returns {string|null}
 */
export function safeBase64Decode(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  if (trimmed.length < 16 || trimmed.length > 500000) return null;

  // 必须只包含 Base64 字符集
  if (!/^[A-Za-z0-9+/=_-]+$/.test(trimmed)) return null;

  try {
    let standardB64 = trimmed.replace(/-/g, '+').replace(/_/g, '/');
    while (standardB64.length % 4 !== 0) standardB64 += '=';

    // 浏览器环境下优先使用 atob + decodeURIComponent 转译 UTF-8
    let decoded = '';
    if (typeof atob === 'function') {
      const binary = atob(standardB64);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
      }
      decoded = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    } else {
      decoded = Buffer.from(standardB64, 'base64').toString('utf8');
    }

    // 检查控制字符比例，排除乱码二进制文件
    const nonPrintable = (decoded.match(/[\x00-\x08\x0B\x0C\x0E-\x1F]/g) || []).length;
    if (nonPrintable > 0 || decoded.length === 0 || decoded === trimmed) return null;

    // 若解码后是 JSON，顺带格式化
    try {
      const parsed = JSON.parse(decoded);
      if (typeof parsed === 'object' && parsed !== null) {
        return JSON.stringify(parsed, null, 2);
      }
    } catch {
      // 保持明文
    }

    return decoded;
  } catch {
    return null;
  }
}

export const base64Rule = {
  id: 'base64_decode',
  category: TRANSFORM_CATEGORIES.CONVERT,
  icon: Binary,
  labelKey: 'transforms.base64Decode',
  titleKey: 'transforms.base64DecodeTitle',
  priority: 50,
  previewable: false,

  match: (text) => {
    return safeBase64Decode(text) !== null;
  },

  apply: (text) => {
    const decoded = safeBase64Decode(text);
    return decoded !== null ? decoded : text;
  },
};
