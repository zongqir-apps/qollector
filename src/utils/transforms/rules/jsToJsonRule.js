import { Braces } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

/**
 * 诊断并将宽松 JS 对象字面量转为标准 JSON 字符串
 * @param {string} text
 * @returns {string|null}
 */
export function convertJsObjectToJson(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  if (!((trimmed.startsWith('{') && trimmed.endsWith('}')) || (trimmed.startsWith('[') && trimmed.endsWith(']')))) {
    return null;
  }

  // 若本身已是合法标准 JSON 则不重复匹配
  try {
    JSON.parse(trimmed);
    return null;
  } catch {
    // 继续尝试宽容转换
  }

  try {
    let normalized = trimmed;
    // 替换单引号包裹的字符串为双引号
    normalized = normalized.replace(/'([^'\\]*(?:\\.[^'\\]*)*)'/g, '"$1"');
    // 给无双引号的合法语义 key 补充双引号
    normalized = normalized.replace(/([{,]\s*)([a-zA-Z_$][a-zA-Z0-9_$]*)\s*:/g, '$1"$2":');
    // 去除末尾多余尾随逗号 (trailing commas)
    normalized = normalized.replace(/,\s*([}\]])/g, '$1');

    const parsed = JSON.parse(normalized);
    if (typeof parsed === 'object' && parsed !== null) {
      return JSON.stringify(parsed, null, 2);
    }
    return null;
  } catch {
    return null;
  }
}

export const jsToJsonRule = {
  id: 'js_object_to_json',
  category: TRANSFORM_CATEGORIES.CONVERT,
  icon: Braces,
  labelKey: 'transforms.jsToJson',
  titleKey: 'transforms.jsToJsonTitle',
  priority: 59,
  previewable: false,

  match: (text) => {
    return convertJsObjectToJson(text) !== null;
  },

  apply: (text) => {
    const jsonStr = convertJsObjectToJson(text);
    return jsonStr !== null ? jsonStr : text;
  },
};
