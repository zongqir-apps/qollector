use crate::ai::AIService;
use crate::commands::ai_commands::AIState;
use crate::core::prompt_assistant::{
    build_prompt_assistant_prompts, validate_request, PromptAssistantRequest,
    PromptAssistantResponse,
};
use tauri::State;

#[tauri::command]
pub async fn run_prompt_assistant_task(
    state: State<'_, AIState>,
    request: PromptAssistantRequest,
) -> Result<PromptAssistantResponse, String> {
    validate_request(&request)?;

    let config = state.config.lock().clone();
    if config.get_current_api_key().is_empty() {
        return Err("AI is not configured".to_string());
    }

    let default_ui_locale = state
        .config_repo
        .load_general_config()
        .await
        .ok()
        .flatten()
        .map(|cfg| cfg.language)
        .unwrap_or_else(|| "zh-CN".to_string());

    let (system_prompt, user_prompt, detected_input_language, effective_output_language) =
        build_prompt_assistant_prompts(&request, &default_ui_locale);

    let model = config.get_current_model();
    let service = AIService::new(config);
    let output = service
        .generate_structured_text(&system_prompt, &user_prompt, 900, 0.45)
        .await
        .map_err(|e| e.to_string())?;

    Ok(PromptAssistantResponse {
        output,
        model,
        detected_input_language,
        effective_output_language,
    })
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAiActionRequest {
    pub content: String,
    pub prompt: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAiActionResponse {
    pub output: String,
    pub model: String,
}

#[tauri::command]
pub async fn run_custom_ai_action(
    state: State<'_, AIState>,
    request: CustomAiActionRequest,
) -> Result<CustomAiActionResponse, String> {
    let content = request.content.trim();
    if content.is_empty() {
        return Err("Content is empty".to_string());
    }
    let prompt = request.prompt.trim();
    if prompt.is_empty() {
        return Err("Prompt is empty".to_string());
    }

    let config = state.config.lock().clone();
    if !config.enabled {
        return Err("AI is not enabled".to_string());
    }
    if config.get_current_api_key().is_empty() {
        return Err("AI is not configured".to_string());
    }

    let model = config.get_current_model();
    let service = AIService::new(config);
    let output = service
        .generate_structured_text(prompt, content, 1500, 0.3)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CustomAiActionResponse { output, model })
}
