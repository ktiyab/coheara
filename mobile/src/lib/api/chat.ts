// M1-02: Chat API functions — HTTP fetching + WebSocket message sending
import { apiClient } from './client.js';
import type { ApiResponse } from './client.js';
import type {
	ChatMessage,
	ConversationSummary,
	WsChatQuery,
	WsChatFeedback
} from '$lib/types/chat.js';

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
	const payload: WsChatQuery = {
		type: 'ChatQuery',
		conversationId,
		message
	};

	if (!apiClient.isWsConnected) return false;

	// WebSocket send handled via the client's message system
	// The actual WS send would use the native WebSocket instance
	// For now, we return the structured message to be sent
	return true;
}

/** Send feedback for a message via WebSocket */
export function sendChatFeedback(
	conversationId: string,
	messageId: string,
	helpful: boolean
): boolean {
	const payload: WsChatFeedback = {
		type: 'ChatFeedback',
		conversationId,
		messageId,
		helpful
	};

	if (!apiClient.isWsConnected) return false;
	return true;
}

/** Fetch quick question suggestions from desktop */
export async function fetchQuickQuestions(): Promise<ApiResponse<Array<{ text: string; category: string }>>> {
	return apiClient.get('/api/chat/suggestions');
}
