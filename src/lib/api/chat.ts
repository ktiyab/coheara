import { invoke } from '@tauri-apps/api/core';
import type {
  Message,
  ConversationSummary,
  PromptSuggestion,
  ChatQueueSnapshot,
  ChatQueueItem,
} from '$lib/types/chat';

export async function startConversation(): Promise<string> {
  return invoke<string>('start_conversation');
}

/** CHAT-QUEUE-01: Returns queue_item_id (non-blocking enqueue). */
export async function sendChatMessage(
  conversationId: string,
  text: string,
): Promise<string> {
  return invoke<string>('send_chat_message', { conversationId, text });
}

/** CHAT-QUEUE-01: Get full chat queue snapshot. */
export async function getChatQueue(): Promise<ChatQueueSnapshot> {
  return invoke<ChatQueueSnapshot>('get_chat_queue');
}

/** CHAT-QUEUE-01: Get pending queue items for a conversation. */
export async function getChatQueueForConversation(
  conversationId: string,
): Promise<ChatQueueItem[]> {
  return invoke<ChatQueueItem[]>('get_chat_queue_for_conversation', { conversationId });
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
