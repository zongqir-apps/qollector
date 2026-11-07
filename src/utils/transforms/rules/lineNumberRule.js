import { ListOrdered } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

const LINE_NO_REGEX = /^\s*(\d+)[\s.:|]\s*(.*)$/;

/**
 * 诊断并剥离多行文本中的代码行号前缀
 * @param {string} text
 * @returns {string|null}
 */
export function stripLineNumbers(text) {
  if (typeof text !== 'string') return null;
  const lines = text.split('\n');
  if (lines.length < 2) return null;

  const nonEmptyLines = lines.filter((l) => l.trim().length > 0);
  if (nonEmptyLines.length < 2) return null;

  let matchCount = 0;
  let lastNum = null;
  let isSequential = true;

  for (const line of nonEmptyLines) {
    const m = line.match(LINE_NO_REGEX);
    if (m) {
      matchCount++;
      const currentNum = parseInt(m[1], 10);
      if (lastNum !== null && currentNum !== lastNum + 1) {
        isSequential = false;
      }
      lastNum = currentNum;
    }
  }

  // 必须绝大部分非空行符合行号格式，且行号单调递增
  if (matchCount / nonEmptyLines.length >= 0.75 && isSequential && matchCount >= 2) {
    const cleanedLines = lines.map((line) => {
      const m = line.match(LINE_NO_REGEX);
      return m ? m[2] : line;
    });
    return cleanedLines.join('\n');
  }

  return null;
}

export const lineNumberRule = {
  id: 'line_numbers_strip',
  category: TRANSFORM_CATEGORIES.CLEAN,
  icon: ListOrdered,
  labelKey: 'transforms.stripLineNumbers',
  titleKey: 'transforms.stripLineNumbersTitle',
  priority: 25,
  previewable: false,

  match: (text) => {
    return stripLineNumbers(text) !== null;
  },

  apply: (text) => {
    const cleaned = stripLineNumbers(text);
    return cleaned !== null ? cleaned : text;
  },
};
