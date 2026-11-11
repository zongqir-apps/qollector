import { Terminal } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

// 常见命令行动词与开发/系统运维工具
const COMMAND_KEYWORDS = /^(?:npm|pnpm|yarn|bun|npx|git|cargo|rustc|docker|docker-compose|podman|kubectl|sudo|curl|wget|systemctl|service|journalctl|apt|apt-get|brew|dnf|pacman|yum|python|python3|pip|pip3|node|deno|bash|sh|zsh|make|cmake|go|uv|rye|cat|grep|ls|find|chmod|chown|kill|pkill|taskctl|dagu|rtk|tar|unzip|zip|mkdir|rm|cp|mv|cd|echo|touch|source|ssh|scp|rsync|tail|head|sed|awk|env|export)$/i;

// 自然语言中容易与命令首词同名的常见短语组合（排除误判）
const NATURAL_LANGUAGE_PREFIXES = /^(?:make\s+sure|go\s+to|find\s+(?:a|the|some|any|out)|cat\s+(?:and|is|in|a|the)|ls\s+is)\b/i;

// 排除常见文档标题与代码注释首词（防止 # 误触）
const COMMENT_HEADER_PREFIXES = /^(?:TODO|NOTE|FIXME|WARN|WARNING|TIP|IMPORTANT|CAUTION|DEBUG|INFO|NOTICE|BUG|XXX|SEE|HACK|AUTHOR|LICENSE|COPYRIGHT|CONFIG|CONFIGURATION|PARAMS|TITLE|DESCRIPTION|SETUP|INSTALLATION|USAGE|EXAMPLES?|BUILD|OPTIONS?)\b/i;

// 括号粘贴模式（Bracketed Paste Mode）转义残留: \x1b[200~, ^[[200~, 200~, ~200 及结尾 ~201, 201~ 等
const BRACKETED_PASTE_START_RE = /^(?:\x1b\[200~|\^\[\[200~|\[200~|200~|~200)\s*/;
const BRACKETED_PASTE_END_RE = /\s*(?:\x1b\[201~|\^\[\[201~|\[201~|201~|~201)$/;

// Shell 带主机/用户/目录提示符: root@host:~# cmd, user@host:~$ cmd, [root@centos ~]# cmd, (base) user@mac:~/code$ cmd
const HOST_PROMPT_RE = /^(?:(?:\([a-zA-Z0-9_.-]+\)|\[[^\]]+\])\s*)?(?:[a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+(?:\s*\[[^\]]+\]|:[~/.][^\s$#]*)?|(?!https?:|ftp:|file:)[a-zA-Z0-9_.-]+:[~/.][^\s$#]*|[[(][a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+[^\])]*[\])])\s*[$#]\s*(\S.*)$/;

/**
 * 诊断输入文本是否属于结构化 Markdown 文档（包含多级标题、代码块、表格、Frontmatter 等）。
 * 若为 Markdown 文档，则严禁将其整体误判为终端命令行。
 * @param {string} text
 * @returns {boolean}
 */
export function isMarkdownDocument(text) {
  if (typeof text !== 'string') return false;

  // 1. 包含 Markdown 二级至六级 ATX 标题（##, ### 等）：Shell 终端提示符绝对不可能出现连续多个 #
  if (/^#{2,6}\s+\S/m.test(text)) {
    return true;
  }

  // 2. 包含 Markdown 代码块围栏（``` 或 ~~~）
  if (/^(?:```|~~~)[a-zA-Z0-9_-]*\s*$/m.test(text)) {
    return true;
  }

  // 3. 包含 Markdown 表格分割线（例如 |---|---| 或 |:---:|）
  if (/(?:^|\n)\|(?:\s*:?-+:?\s*\|)+\s*(?:\n|$)/m.test(text)) {
    return true;
  }

  // 4. 包含 YAML / Markdown Frontmatter 头部元数据
  if (/^---\r?\n[\s\S]*?\r?\n---\r?(?:\n|$)/.test(text.trim())) {
    return true;
  }

  // 5. 包含 Markdown 任务列表（- [ ] 或 - [x]）
  if (/^\s*[-*]\s+\[[ xX]\]\s+\S/m.test(text)) {
    return true;
  }

  return false;
}

/**
 * 诊断并剥离单行的终端提示符与括号粘贴控制残渣（~200 / 200~ 等）。
 * 若非终端命令行或污染行，保留原行不变。
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

  // 1. 括号粘贴模式（Bracketed Paste Mode）残留检测与预脱壳: ~200 / 200~ / ^[[200~ 等
  let stripped = trimmed;
  let hasBracketedPaste = false;
  if (BRACKETED_PASTE_START_RE.test(stripped)) {
    stripped = stripped.replace(BRACKETED_PASTE_START_RE, '');
    hasBracketedPaste = true;
  }
  if (BRACKETED_PASTE_END_RE.test(stripped)) {
    stripped = stripped.replace(BRACKETED_PASTE_END_RE, '');
    hasBracketedPaste = true;
  }

  // 2. PowerShell 提示符: PS C:\Users\xxx> command 或 PS /home/xxx> command
  const psMatch = stripped.match(/^PS(?:\s+[a-zA-Z]:[^>\n\r]*|\s+[~/][^>\n\r]*)?>\s*(\S.*)$/i);
  if (psMatch) {
    return { isCommand: true, cleaned: indent + psMatch[1] };
  }

  // 3. 带主机/用户/目录信息的 Shell 提示符: root@host:~# cmd, user@host:~$ cmd, [root@centos ~]# cmd
  const hostMatch = stripped.match(HOST_PROMPT_RE);
  if (hostMatch) {
    return { isCommand: true, cleaned: indent + hostMatch[1] };
  }

  // 4. 简短 $ 提示符: $ npm install, $ git status, $ ./run.sh
  const dollarMatch = stripped.match(/^\$\s+([a-zA-Z0-9_./-]+)(.*)$/);
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

  // 5. 简短 # 提示符 (root 模式): 严防误伤 Markdown 标题（# 标题）与 YAML/代码注释
  const hashMatch = stripped.match(/^#\s+([a-zA-Z0-9_./-]+)(.*)$/);
  if (hashMatch) {
    // 严禁包含 CJK 字符（彻底排除中文 Markdown 一级标题与中文注释）
    if (!/[\u4e00-\u9fa5]/.test(stripped)) {
      const firstWord = hashMatch[1];
      const rest = hashMatch[2];
      const fullCmd = (firstWord + rest).trim();

      // 首字母大写（如 # Git Workflow, # Docker Architecture, # Python Guide）必然属于标题，严禁误判为命令
      if (!/^[A-Z]/.test(firstWord) && !COMMENT_HEADER_PREFIXES.test(firstWord) && !NATURAL_LANGUAGE_PREFIXES.test(fullCmd)) {
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

  // 6. Markdown 引用格式命令行: > npm install @dnd-kit/core
  const quoteMatch = stripped.match(/^>\s+([a-zA-Z0-9_./-]+)(.*)$/);
  if (quoteMatch) {
    if (!/[\u4e00-\u9fa5]/.test(stripped)) {
      const firstWord = quoteMatch[1];
      const rest = quoteMatch[2];
      const fullCmd = (firstWord + rest).trim();

      // 首字母大写（如 > Make it work, > Go beyond）属于普通英文引用段落，严禁误判为命令
      if (!/^[A-Z]/.test(firstWord) && !NATURAL_LANGUAGE_PREFIXES.test(fullCmd)) {
        if (COMMAND_KEYWORDS.test(firstWord) || /^\.\/|^\//.test(firstWord)) {
          return { isCommand: true, cleaned: indent + fullCmd };
        }
      }
    }
  }

  // 7. 如果本身携带了明确的 bracketed paste 杂质（如 ~200, 200~, ^[[200~），直接认定为受污染的终端行并返回净化后的内容
  if (hasBracketedPaste && stripped.length > 0) {
    return { isCommand: true, cleaned: indent + stripped };
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

  match: (text, meta = {}) => {
    if (typeof text !== 'string' || text.length === 0) return false;

    // 若明确识别为 Markdown 语言，直接跳过
    if (meta?.code_language === 'markdown') return false;

    // 1. 括号粘贴模式（Bracketed Paste Mode）转义残留属于确定性终端残渣，直接命中
    if (BRACKETED_PASTE_START_RE.test(text) || BRACKETED_PASTE_END_RE.test(text)) {
      return true;
    }

    // 2. 结构化 Markdown 文档防御：排版文档与多级标题严禁作为命令行清洗
    if (isMarkdownDocument(text)) {
      return false;
    }

    const lines = text.split('\n');
    const nonEmptyLines = lines.map((l) => l.trim()).filter((l) => l.length > 0);
    if (nonEmptyLines.length === 0) return false;

    let commandCount = 0;
    for (const line of nonEmptyLines) {
      if (cleanTerminalCommandLine(line).isCommand) {
        commandCount++;
      }
    }

    if (commandCount === 0) return false;

    // 3. 单行文本：诊断为命令行即命中
    if (nonEmptyLines.length === 1) {
      return commandCount === 1;
    }

    // 4. 多行文本：必须有足够高的命令密度（至少 50% 的非空行判定为命令行）
    // 防止大段文档或日志中偶发命中单行而被整篇错误改写
    return (commandCount / nonEmptyLines.length) >= 0.5;
  },

  apply: (text) => {
    if (typeof text !== 'string') return '';
    const lines = text.split('\n');
    const cleanedLines = lines.map((line) => cleanTerminalCommandLine(line).cleaned);
    return cleanedLines.join('\n');
  },
};
