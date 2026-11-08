/**
 * AI 动作持久化与管理中枢（解耦 Display Name 与 Prompt）
 */

export const AI_ACTIONS_STORAGE_KEY = 'qollect_ai_custom_actions_v2';

export const DEFAULT_AI_ACTIONS = Object.freeze([
  {
    id: 'builtin_tldr',
    name: '白话省流',
    prompt:
      '用1到3句通俗直接的大白话讲清这篇内容的核心结论和要点。严禁多级标题与公文套话，直接输出结论。',
    isPreset: true,
    order: 0,
  },
  {
    id: 'builtin_listify',
    name: '理清清单',
    prompt:
      '将内容梳理为清晰条理的 Markdown 无序列表要点或行动项，剔除所有冗余寒暄，突出核心事实与关键细节。',
    isPreset: true,
    order: 1,
  },
  {
    id: 'builtin_clean',
    name: '去水规整',
    prompt:
      '消除口语停顿词（如“嗯、然后、就是”）、重复啰嗦语句与破损换行，纠正错别字与语病，润色为自然流畅的书面规范文本，严守原意不变，严禁无中生有。',
    isPreset: true,
    order: 2,
  },
]);

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
 * 从本地读取所有 AI 动作（若缺失则自动填充默认预设）
 */
export function getAiActions() {
  if (typeof window === 'undefined' || !window.localStorage) {
    return DEFAULT_AI_ACTIONS.map((a) => ({ ...a }));
  }

  try {
    const raw = window.localStorage.getItem(AI_ACTIONS_STORAGE_KEY);
    if (!raw) {
      saveAiActions(DEFAULT_AI_ACTIONS);
      return DEFAULT_AI_ACTIONS.map((a) => ({ ...a }));
    }

    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed) || parsed.length === 0) {
      saveAiActions(DEFAULT_AI_ACTIONS);
      return DEFAULT_AI_ACTIONS.map((a) => ({ ...a }));
    }

    const validList = parsed.filter(isValidAction);
    if (validList.length === 0) {
      saveAiActions(DEFAULT_AI_ACTIONS);
      return DEFAULT_AI_ACTIONS.map((a) => ({ ...a }));
    }

    return validList.sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
  } catch (error) {
    console.warn('[AIActionStorage] Failed to read actions, fallback to defaults:', error);
    return DEFAULT_AI_ACTIONS.map((a) => ({ ...a }));
  }
}

/**
 * 保存 AI 动作数组
 */
export function saveAiActions(actions) {
  if (typeof window === 'undefined' || !window.localStorage) return;
  try {
    const listToSave = (actions || []).map((action, index) => ({
      id: action.id,
      name: action.name.trim(),
      prompt: action.prompt.trim(),
      isPreset: Boolean(action.isPreset),
      order: index,
    }));
    window.localStorage.setItem(AI_ACTIONS_STORAGE_KEY, JSON.stringify(listToSave));
  } catch (error) {
    console.error('[AIActionStorage] Failed to save actions:', error);
  }
}

/**
 * 添加一条自定义动作
 */
export function addAiAction({ name, prompt }) {
  const current = getAiActions();
  const trimmedName = (name || '').trim();
  const trimmedPrompt = (prompt || '').trim();

  if (!trimmedName || !trimmedPrompt) {
    throw new Error('Action name and prompt cannot be empty');
  }

  const newAction = {
    id: `custom_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`,
    name: trimmedName,
    prompt: trimmedPrompt,
    isPreset: false,
    order: current.length,
  };

  const next = [...current, newAction];
  saveAiActions(next);
  return newAction;
}

/**
 * 修改某条动作（名称或 Prompt，预设动作也允许修改自定义 Prompt）
 */
export function updateAiAction(id, { name, prompt }) {
  const current = getAiActions();
  const index = current.findIndex((a) => a.id === id);
  if (index === -1) {
    throw new Error(`Action with id ${id} not found`);
  }

  const target = current[index];
  const updated = {
    ...target,
    name: (name !== undefined ? name : target.name).trim(),
    prompt: (prompt !== undefined ? prompt : target.prompt).trim(),
  };

  const next = [...current];
  next[index] = updated;
  saveAiActions(next);
  return updated;
}

/**
 * 删除某条动作（预设动作不可删除）
 */
export function deleteAiAction(id) {
  const current = getAiActions();
  const target = current.find((a) => a.id === id);
  if (!target) return current;
  if (target.isPreset) {
    throw new Error('Preset action cannot be deleted');
  }

  const next = current.filter((a) => a.id !== id);
  saveAiActions(next);
  return next;
}

/**
 * 恢复指定预设动作的默认提示词与名称
 */
export function resetAiAction(id) {
  const preset = DEFAULT_AI_ACTIONS.find((a) => a.id === id);
  if (!preset) return null;

  const current = getAiActions();
  const index = current.findIndex((a) => a.id === id);
  if (index === -1) {
    const next = [...current, { ...preset, order: current.length }];
    saveAiActions(next);
    return preset;
  }

  const next = [...current];
  next[index] = { ...preset, order: next[index].order };
  saveAiActions(next);
  return preset;
}

/**
 * 重置全部为初始 3 个预设
 */
export function resetAllAiActions() {
  saveAiActions(DEFAULT_AI_ACTIONS);
  return DEFAULT_AI_ACTIONS.map((a) => ({ ...a }));
}
