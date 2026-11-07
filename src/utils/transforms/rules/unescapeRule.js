import { Code2 } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

const ESCAPE_PATTERN = /(\\"|\\n|\\r|\\t|\\\\|\\\/)/;

/**
 * 诊断并安全反转义字符串内容
 * @param {string} text
 * @returns {string|null}
 */
export function safeUnescape(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  if (trimmed.length < 6) return null;

  // 必须包含反斜杠转义序列
  if (!ESCAPE_PATTERN.test(trimmed)) return null;

  try {
    let unescaped = trimmed;
    // 若整体被双引号包裹，优先尝试按 JSON 字符串反序列化
    if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
      try {
        const parsed = JSON.parse(trimmed);
        if (typeof parsed === 'string') {
          unescaped = parsed;
        }
      } catch {
        // 容错继续
      }
    }

    // 若仍有转义序列，进行标准反转义替换
    if (ESCAPE_PATTERN.test(unescaped)) {
      unescaped = unescaped
        .replace(/\\"/g, '"')
        .replace(/\\n/g, '\n')
        .replace(/\\r/g, '\r')
        .replace(/\\t/g, '\t')
        .replace(/\\\//g, '/')
        .replace(/\\\\/g, '\\');
    }

    // 去转义后若可解析为有效 JSON 结构，顺带输出格式化 JSON
    try {
      const jsonParsed = JSON.parse(unescaped);
      if (typeof jsonParsed === 'object' && jsonParsed !== null) {
        return JSON.stringify(jsonParsed, null, 2);
      }
    } catch {
      // 保持反转义后的纯文本
    }

    return unescaped !== trimmed ? unescaped : null;
  } catch {
    return null;
  }
}

export const unescapeRule = {
  id: 'string_unescape',
  category: TRANSFORM_CATEGORIES.CLEAN,
  icon: Code2,
  labelKey: 'transforms.stringUnescape',
  titleKey: 'transforms.stringUnescapeTitle',
  priority: 45,
  previewable: false,

  match: (text) => {
    return safeUnescape(text) !== null;
  },

  apply: (text) => {
    const unescaped = safeUnescape(text);
    return unescaped !== null ? unescaped : text;
  },
};
