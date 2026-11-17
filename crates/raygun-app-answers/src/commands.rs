//! Search's answers as commands.

use std::sync::Arc;

use raygun_model::{AnswerRequest, Answers};

use crate::AnswersService;

/// Answers to a question or calculation typed in Search.
#[tauri::command]
#[specta::specta]
pub async fn answers_ask(
    answers: tauri::State<'_, Arc<AnswersService>>,
    request: AnswerRequest,
) -> Result<Answers, String> {
    Ok(answers.ask(request).await)
}
