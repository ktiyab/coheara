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

/** Processing state machine.
 *  Phone does NOT stream tokens — desktop delivers the complete answer
 *  in one ChatComplete message. No buffering needed on phone. */
export type StreamState =
	| { phase: 'idle' }
	| { phase: 'processing'; startedAt: number }
	| { phase: 'complete'; messageId: string; conversationId: string }
	| { phase: 'error'; message: string };

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
 *  Field names use snake_case to match Rust serde serialization.
 *  Phone ignores ChatToken — only processes ChatComplete (full answer in one shot). */
export type WsChatMessage =
	| { type: 'ChatToken'; conversation_id: string; token: string }
	| { type: 'ChatComplete'; conversation_id: string; content: string; citations: WsCitationRef[] }
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

/** Processing timeout — if no ChatComplete within this time, show long-wait notice */
export const PROCESSING_NOTICE_MS = 15_000;
/** Hard timeout — if no ChatComplete within this time, show error */
export const PROCESSING_TIMEOUT_MS = 180_000;

/** Time-based processing stages (search result pattern, not chat pattern).
 *  Driven by elapsed time, not token count. */
export const PROCESSING_STAGES = [
	{ afterMs: 0, label: 'Searching your documents...' },
	{ afterMs: 3_000, label: 'Found relevant records...' },
	{ afterMs: 8_000, label: 'Analyzing your data...' },
	{ afterMs: 15_000, label: 'Processing — Coheara will notify you when ready' },
	{ afterMs: 30_000, label: 'Verifying accuracy...' },
	{ afterMs: 60_000, label: 'Processing complex query...' },
] as const;
