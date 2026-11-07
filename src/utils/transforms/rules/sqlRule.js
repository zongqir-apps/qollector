import { Database } from 'lucide-react';
import { format as formatSql } from 'sql-formatter';
import { TRANSFORM_CATEGORIES } from '../types.js';

const SQL_KEYWORDS = /^(?:SELECT|INSERT\s+INTO|UPDATE|DELETE\s+FROM|CREATE\s+TABLE|ALTER\s+TABLE|DROP\s+TABLE|WITH|EXPLAIN)\b/i;
const SQL_CLAUSES = /\b(?:FROM|WHERE|JOIN|LEFT\s+JOIN|RIGHT\s+JOIN|INNER\s+JOIN|GROUP\s+BY|ORDER\s+BY|HAVING|LIMIT)\b/i;

/**
 * 诊断并美化 SQL 语句
 * @param {string} text
 * @returns {string|null}
 */
export function safeFormatSql(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  if (trimmed.length < 15 || trimmed.length > 200000) return null;

  if (!SQL_KEYWORDS.test(trimmed) && !SQL_CLAUSES.test(trimmed)) {
    return null;
  }

  try {
    const formatted = formatSql(trimmed, {
      language: 'sql',
      keywordCase: 'upper',
    }).trim();
    return formatted !== trimmed ? formatted : null;
  } catch {
    return null;
  }
}

export const sqlRule = {
  id: 'sql_pretty',
  category: TRANSFORM_CATEGORIES.CONVERT,
  icon: Database,
  labelKey: 'transforms.sqlPretty',
  titleKey: 'transforms.sqlPrettyTitle',
  priority: 65,
  previewable: false,

  match: (text) => {
    return safeFormatSql(text) !== null;
  },

  apply: (text) => {
    const formatted = safeFormatSql(text);
    return formatted !== null ? formatted : text;
  },
};
