import { invoke } from '@tauri-apps/api/core';

export const runPromptAssistantTask = async (request) => {
  return invoke('run_prompt_assistant_task', { request });
};

export const runCustomAiAction = async ({ content, prompt }) => {
  try {
    return await invoke('run_custom_ai_action', {
      request: { content, prompt },
    });
  } catch (err) {
    // Fallback compatibility for environments where run_custom_ai_action is mocked
    const errMsg = String(err || '');
    if (errMsg.includes('not found') || errMsg.includes('unknown command')) {
      const response = await invoke('run_prompt_assistant_task', {
        request: {
          taskType: 'summarize',
          input: content,
          extraInstruction: prompt,
        },
      });
      return {
        output: response?.output || '',
        model: response?.model || '',
      };
    }
    throw err;
  }
};
