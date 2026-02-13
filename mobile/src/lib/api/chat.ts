// M1-02: Chat API functions — HTTP fetching + WebSocket message sending
import { apiClient } from './client.js';
import type { ApiResponse } from './client.js';
import type { ChatMessage, ConversationSummary } from '$lib/types/chat.js';

/** Fetch conversation list from desktop */
export async function fetchConversations(): Promise<ApiResponse<ConversationSummary[]>> {
	return apiClient.get<ConversationSummary[]>('/api/chat/conversations');
}

/** Fetch messages for a specific conversation */
export async function fetchConversationMessages(
	conversationId: string
): Promise<ApiResponse<ChatMessage[]>> {
	return apiClient.get<ChatMessage[]>(`/api/chat/conversations/${conversationId}`);
}

/** Send a chat query via WebSocket */
export function sendChatQuery(
	conversationId: string | null,
	message: string
): boolean {
	return apiClient.sendWsMessage({
		type: 'ChatQuery',
		conversation_id: conversationId,
		message
	});
}

/** Send feedback for a message via WebSocket */
export function sendChatFeedback(
	conversationId: string,
	messageId: string,
	helpful: boolean
): boolean {
	return apiClient.sendWsMessage({
		type: 'ChatFeedback',
		conversation_id: conversationId,
		message_id: messageId,
		helpful
	});
}

/** Fetch quick question suggestions from desktop */
export async function fetchQuickQuestions(): Promise<ApiResponse<Array<{ text: string; category: string }>>> {
	return apiClient.get('/api/chat/suggestions');
}
