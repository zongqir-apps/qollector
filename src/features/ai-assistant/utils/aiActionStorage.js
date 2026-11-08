/**
 * AI 动作持久化与管理中枢（解耦 Display Name 与 Prompt）
 * 支持通用文本与代码/日志情境感知
 */

export const AI_CUSTOM_ACTIONS_KEY = 'qollect_ai_custom_actions_v3';
export const AI_PRESET_OVERRIDES_KEY = 'qollect_ai_preset_overrides_v3';

// 通用文本场景预设（人话、清晰直白、零黑话）
export const DEFAULT_AI_ACTIONS = Object.freeze([
  {
    id: 'builtin_summary',
    name: '核心要点',
    prompt:
      '请提炼并总结出这段内容的核心要点与关键结论，按重要度条理化呈现，语言精炼，直奔主题，避免无用废话。',
    isPreset: true,
    category: 'text',
    order: 0,
  },
  {
    id: 'builtin_polish',
    name: '整理润色',
    prompt:
      '对文本进行语句润色与排版规整：消除口语重复与停顿，修正错别字和病句，整理为自然流畅、标点规范的书面规范文本，保持原意不变。',
    isPreset: true,
    category: 'text',
    order: 1,
  },
  {
    id: 'builtin_explain',
    name: '大白话解释',
    prompt:
      '用通俗直白的大白话解释这段内容的核心含义、背景和逻辑机理，深入浅出，让非专业人士也能一眼读懂。',
    isPreset: true,
    category: 'text',
    order: 2,
  },
]);

// 代码与错误日志场景预设（专业针对代码解读、错误排查诊断与重构）
export const CODE_LOG_AI_ACTIONS = Object.freeze([
  {
    id: 'builtin_code_explain',
    name: '代码解释',
    prompt:
      '深入浅出地解释这段代码或命令的执行逻辑、关键入参和工作机制，重点说明核心流程与设计意图。',
    isPreset: true,
    category: 'code_log',
    order: 0,
  },
  {
    id: 'builtin_debug_diagnose',
    name: '排错诊断',
    prompt:
      '深度分析此段报错日志、异常堆栈或代码逻辑缺陷的根本原因，准确指出问题发生位置，并给出直接可运行的修复代码或具体排查方案。',
    isPreset: true,
    category: 'code_log',
    order: 1,
  },
  {
    id: 'builtin_code_refactor',
    name: '规范重构',
    prompt:
      '对这段代码进行重构与健壮性优化：消除代码坏味道、规范命名与边界防御，提升执行效率与可维护性，并输出优化后的完整代码。',
    isPreset: true,
    category: 'code_log',
    order: 2,
  },
]);

/**
 * 轻量且高准确率的内容特征探测器
 * 判断文本是否属于代码片段、执行命令、异常堆栈或格式化数据
 */
export function detectContentType(text = '', sourceType = 'text') {
  if (sourceType === 'code') return 'code_log';
  if (!text || typeof text !== 'string') return 'text';

  const trimmed = text.trim();
  if (trimmed.length < 6) return 'text';

  // 1. 异常报错 / 堆栈探测
  const hasErrorPattern =
    /(?:\b(?:Error|Exception|Traceback|fatal error|panic:|failed with exit code)\b)|(?:^\s*at\s+[\w$./<>]+:\d+:\d+)|(?:\[ERROR\]|\[FATAL\]|\[CRITICAL\])/im.test(
      trimmed
    );
  if (hasErrorPattern) return 'code_log';

  // 2. 常见开发语言关键字与语法结构
  const hasCodeKeywords =
    /(?:^(?:function|class|def|import|export|const|let|var|public|private|protected|async|fn|impl|struct|enum)\s+)|(?:=>\s*\{)|(?:<\/[a-zA-Z0-9]+>)|(?:SELECT\s+.*FROM\s+)|(?:INSERT\s+INTO\s+)|(?:CREATE\s+TABLE\s+)/im.test(
      trimmed
    );
  if (hasCodeKeywords) return 'code_log';

  // 3. 常见 Shell / CLI 命令行特征
  const hasCliCommand =
    /^(?:curl\s+-|git\s+(?:clone|commit|checkout|pull|push|status|log)|docker\s+(?:run|ps|compose|build)|npm\s+(?:i|run|test|install)|yarn\s+|pnpm\s+|cargo\s+|kubectl\s+|systemctl\s+|grep\s+-|chmod\s+|chown\s+|sudo\s+)/m.test(
      trimmed
    );
  if (hasCliCommand) return 'code_log';

  // 4. JSON / 配置字面量特征
  if (
    (trimmed.startsWith('{') && trimmed.endsWith('}')) ||
    (trimmed.startsWith('[') && trimmed.endsWith(']'))
  ) {
    try {
      JSON.parse(trimmed);
      return 'code_log';
    } catch {
      // 忽略普通带括号文本
    }
  }

  return 'text';
}

/**
 * 校验单条动作结构是否合法
 */
function isValidAction(item) {
  return (
    item &&
    typeof item.id === 'string' &&
    item.id.trim().length > 0 &&
    typeof item.name === 'string' &&
    item.name.trim().length > 0 &&
    typeof item.prompt === 'string' &&
    item.prompt.trim().length > 0
  );
}

/**
 * 读取预设覆盖记录（用户对内置预设修改的 Prompt/Name）
 */
function getPresetOverrides() {
  if (typeof window === 'undefined' || !window.localStorage) return {};
  try {
    const raw = window.localStorage.getItem(AI_PRESET_OVERRIDES_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

/**
 * 保存预设覆盖记录
 */
function savePresetOverrides(overrides) {
  if (typeof window === 'undefined' || !window.localStorage) return;
  try {
    window.localStorage.setItem(AI_PRESET_OVERRIDES_KEY, JSON.stringify(overrides));
  } catch (error) {
    console.error('[AIActionStorage] Failed to save preset overrides:', error);
  }
}

/**
 * 读取用户自定义动作列表
 */
function getCustomActions() {
  if (typeof window === 'undefined' || !window.localStorage) return [];
  try {
    const raw = window.localStorage.getItem(AI_CUSTOM_ACTIONS_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (Array.isArray(parsed)) {
        return parsed.filter(isValidAction);
      }
    }

    // 尝试迁移 v2 中用户自建的动作（custom_*）
    const legacyRaw = window.localStorage.getItem('qollect_ai_custom_actions_v2');
    if (legacyRaw) {
      const legacyParsed = JSON.parse(legacyRaw);
      if (Array.isArray(legacyParsed)) {
        const customOnly = legacyParsed.filter(
          (item) => isValidAction(item) && String(item.id).startsWith('custom_')
        );
        if (customOnly.length > 0) {
          saveCustomActions(customOnly);
          return customOnly;
        }
      }
    }
  } catch (error) {
    console.warn('[AIActionStorage] Failed to read custom actions:', error);
  }
  return [];
}

/**
 * 保存用户自定义动作列表
 */
function saveCustomActions(actions) {
  if (typeof window === 'undefined' || !window.localStorage) return;
  try {
    const listToSave = (actions || []).map((action, index) => ({
      id: action.id,
      name: action.name.trim(),
      prompt: action.prompt.trim(),
      isPreset: false,
      order: index,
    }));
    window.localStorage.setItem(AI_CUSTOM_ACTIONS_KEY, JSON.stringify(listToSave));
  } catch (error) {
    console.error('[AIActionStorage] Failed to save custom actions:', error);
  }
}

/**
 * 获取指定情境下生效的全部 AI 动作列表（预设 + 自定义）
 * @param {Object} options
 * @param {string} [options.mode] 'auto' | 'text' | 'code_log'
 * @param {string} [options.sourceText] 当前卡片文本
 * @param {string} [options.sourceType] 当前卡片类型
 */
export function getAiActions(options = {}) {
  const { mode = 'auto', sourceText = '', sourceType = 'text' } =
    typeof options === 'string' ? { mode: options } : options;

  const detectedMode =
    mode === 'auto' ? detectContentType(sourceText, sourceType) : mode;

  const basePresets =
    detectedMode === 'code_log' ? CODE_LOG_AI_ACTIONS : DEFAULT_AI_ACTIONS;

  const overrides = getPresetOverrides();
  const customActions = getCustomActions();

  // 合并预设及其可能的用户自定义 Prompt
  const effectivePresets = basePresets.map((preset, index) => {
    const override = overrides[preset.id];
    return {
      ...preset,
      name: override?.name || preset.name,
      prompt: override?.prompt || preset.prompt,
      isPreset: true,
      order: index,
    };
  });

  return [...effectivePresets, ...customActions];
}

/**
 * 添加一条用户自定义动作
 */
export function addAiAction({ name, prompt }) {
  const trimmedName = (name || '').trim();
  const trimmedPrompt = (prompt || '').trim();

  if (!trimmedName || !trimmedPrompt) {
    throw new Error('Action name and prompt cannot be empty');
  }

  const currentCustom = getCustomActions();
  const newAction = {
    id: `custom_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
    name: trimmedName,
    prompt: trimmedPrompt,
    isPreset: false,
    order: currentCustom.length,
  };

  const next = [...currentCustom, newAction];
  saveCustomActions(next);
  return newAction;
}

/**
 * 修改某条动作（名称或 Prompt，预设动作也支持保存自定义 Prompt）
 */
export function updateAiAction(id, { name, prompt }) {
  const trimmedName = (name || '').trim();
  const trimmedPrompt = (prompt || '').trim();

  if (!trimmedName || !trimmedPrompt) {
    throw new Error('Action name and prompt cannot be empty');
  }

  // 1. 判断是否是内置预设
  const allPresets = [...DEFAULT_AI_ACTIONS, ...CODE_LOG_AI_ACTIONS];
  const isPresetTarget = allPresets.some((p) => p.id === id);

  if (isPresetTarget) {
    const overrides = getPresetOverrides();
    overrides[id] = { name: trimmedName, prompt: trimmedPrompt };
    savePresetOverrides(overrides);
    return { id, name: trimmedName, prompt: trimmedPrompt, isPreset: true };
  }

  // 2. 用户自建动作
  const currentCustom = getCustomActions();
  const index = currentCustom.findIndex((a) => a.id === id);
  if (index === -1) {
    throw new Error(`Action with id ${id} not found`);
  }

  const updated = {
    ...currentCustom[index],
    name: trimmedName,
    prompt: trimmedPrompt,
  };

  const next = [...currentCustom];
  next[index] = updated;
  saveCustomActions(next);
  return updated;
}

/**
 * 删除某条动作（预设动作不可删除）
 */
export function deleteAiAction(id) {
  const allPresets = [...DEFAULT_AI_ACTIONS, ...CODE_LOG_AI_ACTIONS];
  if (allPresets.some((p) => p.id === id)) {
    throw new Error('Preset action cannot be deleted');
  }

  const currentCustom = getCustomActions();
  const next = currentCustom.filter((a) => a.id !== id);
  saveCustomActions(next);
  return next;
}

/**
 * 恢复指定预设动作的默认提示词与名称
 */
export function resetAiAction(id) {
  const overrides = getPresetOverrides();
  if (overrides[id]) {
    delete overrides[id];
    savePresetOverrides(overrides);
  }

  const allPresets = [...DEFAULT_AI_ACTIONS, ...CODE_LOG_AI_ACTIONS];
  return allPresets.find((p) => p.id === id) || null;
}

/**
 * 重置当前情境下的所有预设覆盖，恢复为初始默认
 */
export function resetAllAiActions(mode = 'auto') {
  // 清除预设覆盖
  if (typeof window !== 'undefined' && window.localStorage) {
    window.localStorage.removeItem(AI_PRESET_OVERRIDES_KEY);
  }
  return getAiActions({ mode });
}
