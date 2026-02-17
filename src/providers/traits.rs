use async_trait::async_trait;

/// A single message in a multi-turn conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

/// Role of a message participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

impl ChatRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
        }
    }
}

#[async_trait]
pub trait Provider: Send + Sync {
    async fn chat(&self, message: &str, model: &str, temperature: f64) -> anyhow::Result<String> {
        self.chat_with_system(None, message, model, temperature)
            .await
    }

    async fn chat_with_system(
        &self,
        system_prompt: Option<&str>,
        message: &str,
        model: &str,
        temperature: f64,
    ) -> anyhow::Result<String>;

    /// Send a multi-turn conversation to the model.
    ///
    /// `history` contains alternating user/assistant messages. The last message
    /// should be the current user turn. Providers that support multi-turn will
    /// send the full history; the default falls back to single-turn using only
    /// the last user message.
    async fn chat_multi_turn(
        &self,
        system_prompt: Option<&str>,
        history: &[ChatMessage],
        model: &str,
        temperature: f64,
    ) -> anyhow::Result<String> {
        // Default: extract last user message and delegate to single-turn
        let last_user = history
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.as_str())
            .unwrap_or("");
        self.chat_with_system(system_prompt, last_user, model, temperature)
            .await
    }
}
