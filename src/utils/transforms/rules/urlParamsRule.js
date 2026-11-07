import { SlidersHorizontal } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

/**
 * 解析 URL Query 参数并转换为格式化 JSON
 * @param {string} text
 * @returns {string|null}
 */
export function parseQueryToJson(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  let queryString = '';

  const qIndex = trimmed.indexOf('?');
  if (qIndex !== -1) {
    queryString = trimmed.slice(qIndex + 1);
  } else if (trimmed.includes('&') && trimmed.includes('=') && !trimmed.includes('\n')) {
    queryString = trimmed;
  } else {
    return null;
  }

  const hashIndex = queryString.indexOf('#');
  if (hashIndex !== -1) {
    queryString = queryString.slice(0, hashIndex);
  }

  try {
    const searchParams = new URLSearchParams(queryString);
    const result = {};
    let count = 0;
    for (const [key, value] of searchParams.entries()) {
      if (!key) continue;
      count++;
      if (value === 'true') result[key] = true;
      else if (value === 'false') result[key] = false;
      else if (/^-?\d+(\.\d+)?$/.test(value) && !/^0\d+/.test(value)) {
        result[key] = Number(value);
      } else {
        result[key] = value;
      }
    }
    return count >= 2 ? JSON.stringify(result, null, 2) : null;
  } catch {
    return null;
  }
}

export const urlParamsRule = {
  id: 'url_query_to_json',
  category: TRANSFORM_CATEGORIES.CONVERT,
  icon: SlidersHorizontal,
  labelKey: 'transforms.queryToJson',
  titleKey: 'transforms.queryToJsonTitle',
  priority: 55,
  previewable: false,

  match: (text) => {
    return parseQueryToJson(text) !== null;
  },

  apply: (text) => {
    const parsed = parseQueryToJson(text);
    return parsed !== null ? parsed : text;
  },
};
