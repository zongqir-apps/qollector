import { Terminal } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

// 常见命令行动词与开发/系统运维工具
const COMMAND_KEYWORDS = /^(?:npm|pnpm|yarn|bun|npx|git|cargo|rustc|docker|docker-compose|podman|kubectl|sudo|curl|wget|systemctl|service|journalctl|apt|apt-get|brew|dnf|pacman|yum|python|python3|pip|pip3|node|deno|bash|sh|zsh|make|cmake|go|uv|rye|cat|grep|ls|find|chmod|chown|kill|pkill|taskctl|dagu|rtk|tar|unzip|zip|mkdir|rm|cp|mv|ssh|scp|rsync|tail|head|sed|awk|env|export)$/i;

// 自然语言中容易与命令首词同名的常见短语组合（排除误判）
const NATURAL_LANGUAGE_PREFIXES = /^(?:make\s+sure|go\s+to|find\s+(?:a|the|some|any|out)|cat\s+(?:and|is|in|a|the)|ls\s+is)\b/i;

// 排除常见文档标题与代码注释首词（防止 # 误触）
const COMMENT_HEADER_PREFIXES = /^(?:TODO|NOTE|FIXME|WARN|WARNING|TIP|IMPORTANT|CAUTION|DEBUG|INFO|NOTICE|BUG|XXX|SEE|HACK|AUTHOR|LICENSE|COPYRIGHT|CONFIG|CONFIGURATION|PARAMS|TITLE|DESCRIPTION|SETUP|INSTALLATION|USAGE|EXAMPLES?|BUILD|OPTIONS?)\b/i;

/**
 * 诊断并剥离单行的终端提示符。
 * 若非终端命令行，保留原行不变。
 * @param {string} line
 * @returns {{ isCommand: boolean, cleaned: string }}
 */
export function cleanTerminalCommandLine(line) {
  if (typeof line !== 'string') {
    return { isCommand: false, cleaned: '' };
  }

  const trimmed = line.trim();
  if (!trimmed) {
    return { isCommand: false, cleaned: line };
  }

  const indent = line.match(/^\s*/)[0];

  // 1. PowerShell 提示符: PS C:\Users\xxx> command 或 PS /home/xxx> command
  const psMatch = trimmed.match(/^PS(?:\s+[a-zA-Z]:[^\n\r>]*|\s+[/~][^\n\r>]*|\s+[^\n\r>]+)?>\s*(\S.*)$/i);
  if (psMatch) {
    return { isCommand: true, cleaned: indent + psMatch[1] };
  }

  // 2. 带主机/用户信息的 Shell 提示符: root@host:~# cmd, user@host:~$ cmd, [root@centos ~]# cmd
  const hostMatch = trimmed.match(/^(?:[a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+(?:\s*\[[^\]]+\]|\s*:[^\n\r$#]*)?|[a-zA-Z0-9_.-]+:[^\n\r$#]+|[[(][a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+[^\])]*[\])])\s*[$#]\s*(\S.*)$/);
  if (hostMatch) {
    return { isCommand: true, cleaned: indent + hostMatch[1] };
  }

  // 3. 简短 $ 提示符: $ npm install, $ git status, $ ./run.sh
  const dollarMatch = trimmed.match(/^\$\s+([a-zA-Z0-9_./-]+)(.*)$/);
  if (dollarMatch) {
    const firstWord = dollarMatch[1];
    const rest = dollarMatch[2];
    const fullCmd = (firstWord + rest).trim();

    // 排除纯数字（美元货币金额，如 $ 100, $ 29.99）与自然语言短语
    if (!/^\d+(?:\.\d+)?$/.test(firstWord) && !NATURAL_LANGUAGE_PREFIXES.test(fullCmd)) {
      if (
        COMMAND_KEYWORDS.test(firstWord) ||
        /^\.\/|^\//.test(firstWord) ||
        /\s(?:--?[a-zA-Z0-9-]+|&&|\|)/.test(rest)
      ) {
        return { isCommand: true, cleaned: indent + fullCmd };
      }
    }
  }

  // 4. 简短 # 提示符 (root 模式): 严防误伤 Markdown 一级标题（# 标题）与 YAML/代码注释
  const hashMatch = trimmed.match(/^#\s+([a-zA-Z0-9_./-]+)(.*)$/);
  if (hashMatch) {
    // 严禁包含 CJK 字符（彻底排除中文 Markdown 一级标题与中文注释）
    if (!/[\u4e00-\u9fa5]/.test(trimmed)) {
      const firstWord = hashMatch[1];
      const rest = hashMatch[2];
      const fullCmd = (firstWord + rest).trim();

      if (!COMMENT_HEADER_PREFIXES.test(firstWord) && !NATURAL_LANGUAGE_PREFIXES.test(fullCmd)) {
        if (
          COMMAND_KEYWORDS.test(firstWord) ||
          /^\.\/|^\//.test(firstWord) ||
          /\s(?:--?[a-zA-Z0-9-]+|&&|\|)/.test(rest)
        ) {
          return { isCommand: true, cleaned: indent + fullCmd };
        }
      }
    }
  }

  // 5. Markdown 引用格式命令行: > npm install @dnd-kit/core
  const quoteMatch = trimmed.match(/^>\s+([a-zA-Z0-9_./-]+)(.*)$/);
  if (quoteMatch) {
    if (!/[\u4e00-\u9fa5]/.test(trimmed)) {
      const firstWord = quoteMatch[1];
      const rest = quoteMatch[2];
      const fullCmd = (firstWord + rest).trim();

      if (!NATURAL_LANGUAGE_PREFIXES.test(fullCmd)) {
        if (COMMAND_KEYWORDS.test(firstWord) || /^\.\/|^\//.test(firstWord)) {
          return { isCommand: true, cleaned: indent + fullCmd };
        }
      }
    }
  }

  return { isCommand: false, cleaned: line };
}

export const terminalRule = {
  id: 'terminal_clean',
  category: TRANSFORM_CATEGORIES.CLEAN,
  icon: Terminal,
  labelKey: 'transforms.terminalClean',
  titleKey: 'transforms.terminalCleanTitle',
  priority: 20,
  previewable: false,

  match: (text) => {
    if (typeof text !== 'string' || text.length === 0) return false;
    const lines = text.split('\n');
    return lines.some((line) => cleanTerminalCommandLine(line).isCommand);
  },

  apply: (text) => {
    if (typeof text !== 'string') return '';
    const lines = text.split('\n');
    const cleanedLines = lines.map((line) => cleanTerminalCommandLine(line).cleaned);
    return cleanedLines.join('\n');
  },
};
