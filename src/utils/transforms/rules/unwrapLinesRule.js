import { WrapText } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

/**
 * 诊断并合并从 PDF 或窄屏文档中复制出来的被腰斩硬断行
 * @param {string} text
 * @returns {string|null}
 */
export function unwrapPdfLines(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  if (!trimmed.includes('\n')) return null;

  const lines = trimmed.split('\n');
  if (lines.length < 2) return null;

  // 排除列表特征
  const listCount = lines.filter((l) => /^\s*[-*+]\s+|^\s*\d+[\s.:|]\s*/.test(l)).length;
  if (listCount / lines.length > 0.4) return null;

  // 排除代码特征
  const codePunct = (trimmed.match(/[{};=<>()[\]]/g) || []).length;
  if (codePunct / trimmed.length > 0.08) return null;

  const paragraphs = text.split(/\r?\n\s*\r?\n/);
  let changed = false;

  const mergedParagraphs = paragraphs.map((para) => {
    const rawLines = para.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
    if (rawLines.length <= 1) return para;

    let res = '';
    for (let i = 0; i < rawLines.length; i++) {
      const line = rawLines[i];
      if (i === 0) {
        res = line;
        continue;
      }

      // 连字符断行自动愈合: e.g. "configu-\nration" => "configuration"
      if (res.endsWith('-') && /^[a-zA-Z]/.test(line)) {
        res = res.slice(0, -1) + line;
        changed = true;
      } else {
        const lastChar = res.slice(-1);
        const nextChar = line.charAt(0);
        const isCjk = /[\u4e00-\u9fa5\u3000-\u303f\uff00-\uffef]/.test(lastChar) && /[\u4e00-\u9fa5]/.test(nextChar);

        if (isCjk) {
          res += line;
          changed = true;
        } else {
          res += ' ' + line;
          changed = true;
        }
      }
    }
    return res;
  });

  return changed ? mergedParagraphs.join('\n\n') : null;
}

export const unwrapLinesRule = {
  id: 'unwrap_pdf_lines',
  category: TRANSFORM_CATEGORIES.CLEAN,
  icon: WrapText,
  labelKey: 'transforms.unwrapLines',
  titleKey: 'transforms.unwrapLinesTitle',
  priority: 35,
  previewable: false,

  match: (text) => {
    return unwrapPdfLines(text) !== null;
  },

  apply: (text) => {
    const unwrapped = unwrapPdfLines(text);
    return unwrapped !== null ? unwrapped : text;
  },
};
