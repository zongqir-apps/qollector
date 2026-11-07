import { Table } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

/**
 * 诊断并将 TSV 表格数据转换为 Markdown 格式表格
 * @param {string} text
 * @returns {string|null}
 */
export function parseTsvToMarkdown(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  if (trimmed.includes('| --- |') || trimmed.includes('|:---|')) return null;

  const lines = trimmed.split('\n').map((l) => l.trim()).filter((l) => l.length > 0);
  if (lines.length < 2) return null;

  // 必须每一行都含有制表符
  const matrix = lines.map((line) => line.split('\t').map((c) => c.trim()));
  const colCount = matrix[0].length;
  if (colCount < 2) return null;

  for (const row of matrix) {
    if (Math.abs(row.length - colCount) > 1) return null;
  }

  const normalizedMatrix = matrix.map((row) => {
    const copy = [...row];
    while (copy.length < colCount) copy.push('');
    return copy.slice(0, colCount);
  });

  const headerRow = `| ${normalizedMatrix[0].join(' | ')} |`;
  const separatorRow = `| ${Array.from({ length: colCount }, () => '---').join(' | ')} |`;
  const bodyRows = normalizedMatrix.slice(1).map((row) => `| ${row.join(' | ')} |`);

  return [headerRow, separatorRow, ...bodyRows].join('\n');
}

export const tsvToMarkdownRule = {
  id: 'tsv_to_markdown_table',
  category: TRANSFORM_CATEGORIES.CONVERT,
  icon: Table,
  labelKey: 'transforms.tsvToMarkdown',
  titleKey: 'transforms.tsvToMarkdownTitle',
  priority: 58,
  previewable: false,

  match: (text) => {
    return parseTsvToMarkdown(text) !== null;
  },

  apply: (text) => {
    const md = parseTsvToMarkdown(text);
    return md !== null ? md : text;
  },
};
