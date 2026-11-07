import YAML from 'yaml';
import { FileJson, AlignLeft } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

const MAX_YAML_PARSE_LENGTH = 500000;

/**
 * 安全解析 YAML 文本
 * 确保是结构化 YAML（对象或数组），排除纯 JSON（交给 jsonRule 处理）与非结构化纯文本
 * @param {string} text
 * @returns {object|array|null}
 */
export function safeParseYaml(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  if (trimmed.length === 0 || trimmed.length > MAX_YAML_PARSE_LENGTH) return null;

  // 排除纯 JSON 格式（以 { 或 [ 包裹且能被 JSON.parse 解析）
  if ((trimmed.startsWith('{') && trimmed.endsWith('}')) || (trimmed.startsWith('[') && trimmed.endsWith(']'))) {
    try {
      JSON.parse(trimmed);
      return null;
    } catch {
      // 包含花括号但非标准 JSON，允许按 YAML 继续容错解析
    }
  }

  // 启发式过滤：必须包含结构化 YAML 特征（文档头 ---、键值对 key: 或列表项 - ）
  const hasYamlStructure =
    /^---\s*$/m.test(trimmed) ||
    /(?:^|\n)\s*(?:[a-zA-Z0-9_.-]+|'[^']+'|"[^"]+"):\s*(?:$|\S)/.test(trimmed) ||
    /(?:^|\n)\s*-\s+\S/.test(trimmed);
  if (!hasYamlStructure) return null;

  try {
    const parsed = YAML.parse(trimmed);
    if (parsed !== null && typeof parsed === 'object') {
      return parsed;
    }
    return null;
  } catch {
    return null;
  }
}

export const yamlToJsonRule = {
  id: 'yaml_to_json',
  category: TRANSFORM_CATEGORIES.CONVERT,
  icon: FileJson,
  labelKey: 'transforms.yamlToJson',
  titleKey: 'transforms.yamlToJsonTitle',
  priority: 62,
  previewable: false,

  match: (text) => {
    return safeParseYaml(text) !== null;
  },

  apply: (text) => {
    const parsed = safeParseYaml(text);
    if (parsed === null) return text;
    try {
      return JSON.stringify(parsed, null, 2);
    } catch {
      return text;
    }
  },
};

export const yamlPrettyRule = {
  id: 'yaml_pretty',
  category: TRANSFORM_CATEGORIES.CONVERT,
  icon: AlignLeft,
  labelKey: 'transforms.yamlPretty',
  titleKey: 'transforms.yamlPrettyTitle',
  priority: 63,
  previewable: false,

  match: (text) => {
    const parsed = safeParseYaml(text);
    if (parsed === null) return false;
    const trimmed = text.trim();
    if (!trimmed.includes('\n')) return false;
    try {
      const formatted = YAML.stringify(parsed, { indent: 2 }).trim();
      return formatted !== trimmed;
    } catch {
      return false;
    }
  },

  apply: (text) => {
    const parsed = safeParseYaml(text);
    if (parsed === null) return text;
    try {
      return YAML.stringify(parsed, { indent: 2 });
    } catch {
      return text;
    }
  },
};

