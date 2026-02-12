import { invoke } from '@tauri-apps/api/core';
import type { Message, ConversationSummary, PromptSuggestion } from '$lib/types/chat';

export async function startConversation(): Promise<string> {
  return invoke<string>('start_conversation');
}

export async function sendChatMessage(
  conversationId: string,
  text: string,
): Promise<void> {
  return invoke('send_chat_message', { conversationId, text });
}

export async function getConversationMessages(
  conversationId: string,
): Promise<Message[]> {
  return invoke<Message[]>('get_conversation_messages', { conversationId });
}

export async function listConversations(): Promise<ConversationSummary[]> {
  return invoke<ConversationSummary[]>('list_conversations');
}

export async function deleteConversation(conversationId: string): Promise<void> {
  return invoke('delete_conversation', { conversationId });
}

export async function setMessageFeedback(
  messageId: string,
  feedback: 'Helpful' | 'NotHelpful' | null,
): Promise<void> {
  return invoke('set_message_feedback', { messageId, feedback });
}

export async function getPromptSuggestions(): Promise<PromptSuggestion[]> {
  return invoke<PromptSuggestion[]>('get_prompt_suggestions');
}
