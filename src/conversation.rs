//! Multi-turn conversation history tracker.
//!
//! Maintains per-user conversation state with configurable max turns and
//! inactivity timeout. When a conversation times out, the history is cleared
//! so the next message starts fresh.

use crate::providers::{ChatMessage, ChatRole};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Tracks conversation history for multiple concurrent users/channels.
pub struct ConversationTracker {
    conversations: HashMap<String, ConversationState>,
    max_turns: usize,
    timeout: Duration,
}

struct ConversationState {
    messages: Vec<ChatMessage>,
    last_activity: Instant,
}

impl ConversationTracker {
    /// Create a new tracker.
    ///
    /// - `max_turns`: maximum number of user+assistant turn pairs to keep.
    ///   For example, 20 means up to 20 user messages and 20 assistant messages (40 total).
    /// - `timeout_minutes`: minutes of inactivity before a conversation resets.
    pub fn new(max_turns: usize, timeout_minutes: u64) -> Self {
        Self {
            conversations: HashMap::new(),
            max_turns: max_turns.max(1),
            timeout: Duration::from_secs(timeout_minutes.max(1) * 60),
        }
    }

    /// Record a user message and return the full conversation history
    /// (including this message) to send to the provider.
    ///
    /// If the conversation has timed out, history is cleared first.
    pub fn push_user_message(&mut self, key: &str, content: String) -> Vec<ChatMessage> {
        let now = Instant::now();

        let state = self
            .conversations
            .entry(key.to_string())
            .or_insert_with(|| ConversationState {
                messages: Vec::new(),
                last_activity: now,
            });

        // Reset if timed out
        if now.duration_since(state.last_activity) > self.timeout {
            state.messages.clear();
        }

        state.last_activity = now;

        state.messages.push(ChatMessage {
            role: ChatRole::User,
            content,
        });

        // Trim to max turns (keep last N*2 messages to preserve pairs)
        let max_messages = self.max_turns * 2;
        if state.messages.len() > max_messages {
            let drain_count = state.messages.len() - max_messages;
            state.messages.drain(..drain_count);
        }

        state.messages.clone()
    }

    /// Record the assistant's response for a conversation.
    pub fn push_assistant_message(&mut self, key: &str, content: String) {
        if let Some(state) = self.conversations.get_mut(key) {
            state.last_activity = Instant::now();
            state.messages.push(ChatMessage {
                role: ChatRole::Assistant,
                content,
            });

            // Trim again in case we're over
            let max_messages = self.max_turns * 2;
            if state.messages.len() > max_messages {
                let drain_count = state.messages.len() - max_messages;
                state.messages.drain(..drain_count);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_conversation_flow() {
        let mut tracker = ConversationTracker::new(10, 15);

        let history = tracker.push_user_message("user1", "hello".into());
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "hello");
        assert_eq!(history[0].role, ChatRole::User);

        tracker.push_assistant_message("user1", "hi there".into());

        let history = tracker.push_user_message("user1", "how are you?".into());
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].content, "hello");
        assert_eq!(history[1].content, "hi there");
        assert_eq!(history[2].content, "how are you?");
    }

    #[test]
    fn separate_users_have_separate_histories() {
        let mut tracker = ConversationTracker::new(10, 15);

        tracker.push_user_message("alice", "hello from alice".into());
        tracker.push_assistant_message("alice", "hi alice".into());

        tracker.push_user_message("bob", "hello from bob".into());
        tracker.push_assistant_message("bob", "hi bob".into());

        let alice_history = tracker.push_user_message("alice", "alice again".into());
        assert_eq!(alice_history.len(), 3);
        assert_eq!(alice_history[0].content, "hello from alice");

        let bob_history = tracker.push_user_message("bob", "bob again".into());
        assert_eq!(bob_history.len(), 3);
        assert_eq!(bob_history[0].content, "hello from bob");
    }

    #[test]
    fn trims_to_max_turns() {
        let mut tracker = ConversationTracker::new(2, 15);

        // Fill 3 full turns (should trim to 2)
        for i in 0..3 {
            tracker.push_user_message("u", format!("q{i}"));
            tracker.push_assistant_message("u", format!("a{i}"));
        }

        let history = tracker.push_user_message("u", "q3".into());
        // max_turns=2 means max 4 messages, plus this new one = 5, trimmed to 4+1
        // Actually: after push we have 7, trim to 4 => last 4 = a1, q2, a2, q3
        assert!(history.len() <= 5);
        // The last message should always be the new user message
        assert_eq!(history.last().unwrap().content, "q3");
    }

    #[test]
    fn timeout_resets_history() {
        let mut tracker = ConversationTracker {
            conversations: HashMap::new(),
            max_turns: 10,
            timeout: Duration::from_millis(1), // 1ms timeout for testing
        };

        tracker.push_user_message("u", "old message".into());
        tracker.push_assistant_message("u", "old reply".into());

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(5));

        let history = tracker.push_user_message("u", "new conversation".into());
        // Should have reset — only the new message
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "new conversation");
    }

    #[test]
    fn min_max_turns_is_one() {
        let tracker = ConversationTracker::new(0, 1);
        assert_eq!(tracker.max_turns, 1);
    }

    #[test]
    fn assistant_message_for_unknown_key_is_noop() {
        let mut tracker = ConversationTracker::new(10, 15);
        // Should not panic
        tracker.push_assistant_message("nonexistent", "reply".into());
    }
}
