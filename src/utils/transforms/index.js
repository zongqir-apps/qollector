/**
 * 文本快捷转换规则注册表与调度中枢
 */

import { unicodeRule } from './rules/unicodeRule.js';
import { terminalRule } from './rules/terminalRule.js';
import { lineNumberRule } from './rules/lineNumberRule.js';
import { pathRule } from './rules/pathRule.js';
import { tokenRule } from './rules/tokenRule.js';
import { unescapeRule } from './rules/unescapeRule.js';
import { base64Rule } from './rules/base64Rule.js';
import { urlRule } from './rules/urlRule.js';
import { urlParamsRule } from './rules/urlParamsRule.js';
import { jsonPrettyRule, jsonCompactRule, jsonToYamlRule } from './rules/jsonRule.js';
import { yamlToJsonRule, yamlPrettyRule } from './rules/yamlRule.js';
import { sqlRule } from './rules/sqlRule.js';
import { envRule } from './rules/envRule.js';
import { gitRule } from './rules/gitRule.js';
import { timestampRule } from './rules/timestampRule.js';
import { unwrapLinesRule } from './rules/unwrapLinesRule.js';
import { jwtRule } from './rules/jwtRule.js';
import { tsvToMarkdownRule } from './rules/tsvToMarkdownRule.js';
import { jsToJsonRule } from './rules/jsToJsonRule.js';

/**
 * 已注册的转换规则集（按顺序评估）
 */
export const TRANSFORM_REGISTRY = [
  unicodeRule,
  terminalRule,
  lineNumberRule,
  pathRule,
  tokenRule,
  unescapeRule,
  base64Rule,
  urlRule,
  urlParamsRule,
  jsonPrettyRule,
  jsonCompactRule,
  yamlToJsonRule,
  yamlPrettyRule,
  jsonToYamlRule,
  sqlRule,
  envRule,
  gitRule,
  timestampRule,
  unwrapLinesRule,
  jwtRule,
  tsvToMarkdownRule,
  jsToJsonRule,
];

/**
 * 根据输入文本解析所有命中的转换规则列表
 * @param {string} text - 输入内容
 * @param {object} meta - 辅助元数据（如当前语言、item属性等）
 * @returns {Array} 匹配的规则对象数组（按 priority 升序排序）
 */
export function resolveMatchingTransforms(text, meta = {}) {
  if (typeof text !== 'string' || text.length === 0) {
    return [];
  }

  const matched = [];
  for (const rule of TRANSFORM_REGISTRY) {
    try {
      if (rule.match(text, meta)) {
        matched.push(rule);
      }
    } catch (error) {
      console.warn(`[TransformRegistry] Rule "${rule.id}" match failed:`, error);
    }
  }

  return matched.sort((a, b) => (a.priority || 100) - (b.priority || 100));
}

/**
 * 根据 ID 查询单条规则
 * @param {string} id - 规则唯一标识符
 * @returns {object|null}
 */
export function getTransformRule(id) {
  return TRANSFORM_REGISTRY.find((rule) => rule.id === id) || null;
}

export {
  unicodeRule,
  terminalRule,
  lineNumberRule,
  pathRule,
  tokenRule,
  unescapeRule,
  base64Rule,
  urlRule,
  urlParamsRule,
  jsonPrettyRule,
  jsonCompactRule,
  yamlToJsonRule,
  yamlPrettyRule,
  jsonToYamlRule,
  sqlRule,
  envRule,
  gitRule,
  timestampRule,
  unwrapLinesRule,
  jwtRule,
  tsvToMarkdownRule,
  jsToJsonRule,
};
