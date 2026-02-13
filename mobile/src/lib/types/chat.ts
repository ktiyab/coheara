// M1-02: Chat interface types

/** Chat message role */
export type ChatRole = 'patient' | 'coheara';

/** Chat source indicator */
export type ChatSource = 'live' | 'cached' | 'unavailable';

/** Chat message */
export interface ChatMessage {
	id: string;
	conversationId: string;
	role: ChatRole;
	content: string;
	timestamp: string;
	citations: Citation[];
	confidence?: number;
	feedback?: 'helpful' | 'not_helpful' | null;
	source: ChatSource;
}

/** Citation reference to a specific document */
export interface Citation {
	documentId: string;
	documentTitle: string;
	documentDate: string;
	professionalName?: string;
	chunkText: string;
	relevanceScore: number;
}

/** Conversation summary for list view */
export interface ConversationSummary {
	id: string;
	title: string;
	lastMessageAt: string;
	messageCount: number;
	lastMessagePreview: string;
	source: ChatSource;
}

/** Streaming state machine */
export type StreamState =
	| { phase: 'idle' }
	| { phase: 'loading'; startedAt: number }
	| { phase: 'streaming'; tokens: string; conversationId: string }
	| { phase: 'complete'; messageId: string; conversationId: string }
	| { phase: 'error'; message: string; partial?: string };

/** Quick question suggestion (Mamadou) */
export interface QuickQuestion {
	text: string;
	category: 'medications' | 'appointments' | 'general';
}

/** Deferred question (queued when offline/unavailable) */
export interface DeferredQuestion {
	id: string;
	text: string;
	createdAt: string;
	answered: boolean;
}

/** WebSocket incoming chat messages (from desktop).
 *  Field names use snake_case to match Rust serde serialization. */
export type WsChatMessage =
	| { type: 'ChatToken'; conversation_id: string; token: string }
	| { type: 'ChatComplete'; conversation_id: string; citations: WsCitationRef[] }
	| { type: 'ChatError'; conversation_id: string; error: string };

/** Citation reference as sent by desktop WsOutgoing::ChatComplete */
export interface WsCitationRef {
	document_id: string;
	document_title: string;
	chunk_id?: string;
}

/** WebSocket outgoing chat messages (to desktop).
 *  Field names use snake_case to match Rust serde deserialization. */
export interface WsChatQuery {
	type: 'ChatQuery';
	conversation_id: string | null;
	message: string;
}

export interface WsChatFeedback {
	type: 'ChatFeedback';
	conversation_id: string;
	message_id: string;
	helpful: boolean;
}

/** Streaming timeout thresholds */
export const LOADING_TIMEOUT_MS = 10_000;
export const TOKEN_TIMEOUT_MS = 15_000;
