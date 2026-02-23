// M1-02: Chat store — conversation state, processing, deferred questions
// CA-07: Phone receives complete answer in one ChatComplete — no token buffering.
import { writable, derived, get } from 'svelte/store';
import type {
	ChatMessage,
	ChatSource,
	Citation,
	ConversationSummary,
	DeferredQuestion,
	QuickQuestion,
	StreamState,
	WsChatMessage,
	WsCitationRef
} from '$lib/types/chat.js';
import { PROCESSING_TIMEOUT_MS, PROCESSING_STAGES } from '$lib/types/chat.js';

// --- Core stores ---

/** Current conversation messages */
export const messages = writable<ChatMessage[]>([]);

/** Current conversation ID */
export const activeConversationId = writable<string | null>(null);

/** Conversation list */
export const conversations = writable<ConversationSummary[]>([]);

/** Processing state */
export const streamState = writable<StreamState>({ phase: 'idle' });

/** Deferred question queue (offline questions) */
export const deferredQuestions = writable<DeferredQuestion[]>([]);

// --- Derived stores ---

/** Whether the desktop is processing a query */
export const isStreaming = derived(streamState, ($s) =>
	$s.phase === 'processing'
);

/** Whether the chat input should be disabled */
export const inputDisabled = derived(streamState, ($s) =>
	$s.phase === 'processing'
);

/** Number of pending deferred questions */
export const deferredCount = derived(deferredQuestions, ($q) =>
	$q.filter((q) => !q.answered).length
);

/** Current processing stage label (time-based, not token-based) */
export const processingStage = derived(streamState, ($s) => {
	if ($s.phase !== 'processing') return '';
	const elapsed = Date.now() - $s.startedAt;
	let label = PROCESSING_STAGES[0].label;
	for (const stage of PROCESSING_STAGES) {
		if (elapsed >= stage.afterMs) label = stage.label;
	}
	return label;
});

// --- Quick questions (Mamadou) ---

export const DEFAULT_QUICK_QUESTIONS: readonly QuickQuestion[] = [
	{ text: 'What are my medications?', category: 'medications' },
	{ text: 'When is my next appointment?', category: 'appointments' },
	{ text: 'What should I tell my doctor?', category: 'general' }
] as const;

// --- Processing management ---

let processingTimer: ReturnType<typeof setTimeout> | null = null;

/** Start a new query — sets processing state */
export function startQuery(_source: ChatSource): void {
	const now = Date.now();
	streamState.set({ phase: 'processing', startedAt: now });

	clearTimers();
	processingTimer = setTimeout(() => {
		const current = get(streamState);
		if (current.phase === 'processing') {
			streamState.set({
				phase: 'error',
				message: 'Taking longer than usual. Check your desktop is active.'
			});
		}
	}, PROCESSING_TIMEOUT_MS);
}

/** Handle incoming WebSocket chat message.
 *  Phone ignores ChatToken — only processes ChatComplete (full answer). */
export function handleWsChatMessage(msg: WsChatMessage): void {
	switch (msg.type) {
		case 'ChatToken':
			// Intentionally ignored — phone receives complete answer in ChatComplete
			break;
		case 'ChatComplete':
			handleComplete(msg.conversation_id, msg.content, msg.citations);
			break;
		case 'ChatError':
			handleStreamError(msg.error);
			break;
	}
}

let wsMessageCounter = 0;

function handleComplete(conversationId: string, content: string, wsCitations: WsCitationRef[]): void {
	clearTimers();

	const messageId = `ws-${Date.now()}-${++wsMessageCounter}`;

	// Map WS citation refs to display citations
	const citations: Citation[] = wsCitations.map((c) => ({
		documentId: c.document_id,
		documentTitle: c.document_title,
		documentDate: '',
		chunkText: '',
		relevanceScore: 0
	}));

	// Add complete message to messages list — full answer appears at once
	const message: ChatMessage = {
		id: messageId,
		conversationId,
		role: 'coheara',
		content,
		timestamp: new Date().toISOString(),
		citations,
		feedback: null,
		source: 'live'
	};

	messages.update(($m) => [...$m, message]);
	activeConversationId.set(conversationId);
	streamState.set({ phase: 'complete', messageId, conversationId });
}

function handleStreamError(error: string): void {
	clearTimers();
	streamState.set({ phase: 'error', message: error });
}

/** Add a patient message to the conversation */
export function addPatientMessage(conversationId: string | null, text: string): ChatMessage {
	const message: ChatMessage = {
		id: `local-${Date.now()}`,
		conversationId: conversationId ?? 'pending',
		role: 'patient',
		content: text,
		timestamp: new Date().toISOString(),
		citations: [],
		feedback: null,
		source: 'live'
	};

	messages.update(($m) => [...$m, message]);
	return message;
}

/** Set feedback on a message */
export function setMessageFeedback(messageId: string, helpful: boolean): void {
	messages.update(($m) =>
		$m.map((m) =>
			m.id === messageId ? { ...m, feedback: helpful ? 'helpful' : 'not_helpful' } : m
		)
	);
}

// --- Deferred questions ---

let deferredCounter = 0;

/** Add a question to the deferred queue (when offline) */
export function deferQuestion(text: string): DeferredQuestion {
	const question: DeferredQuestion = {
		id: `deferred-${Date.now()}-${++deferredCounter}`,
		text,
		createdAt: new Date().toISOString(),
		answered: false
	};

	deferredQuestions.update(($q) => [...$q, question]);
	return question;
}

/** Mark a deferred question as answered */
export function markDeferredAnswered(questionId: string): void {
	deferredQuestions.update(($q) =>
		$q.map((q) => (q.id === questionId ? { ...q, answered: true } : q))
	);
}

/** Get pending (unanswered) deferred questions */
export function getPendingDeferred(): DeferredQuestion[] {
	return get(deferredQuestions).filter((q) => !q.answered);
}

// --- Conversation management ---

/** Load conversations from desktop API response */
export function setConversations(list: ConversationSummary[]): void {
	conversations.set(list);
}

/** Load messages for a specific conversation */
export function setMessages(msgs: ChatMessage[]): void {
	messages.set(msgs);
}

/** Clear current conversation */
export function clearConversation(): void {
	messages.set([]);
	activeConversationId.set(null);
	streamState.set({ phase: 'idle' });
	clearTimers();
}

/** Reset all chat state (for testing) */
export function resetChatState(): void {
	messages.set([]);
	activeConversationId.set(null);
	conversations.set([]);
	streamState.set({ phase: 'idle' });
	deferredQuestions.set([]);
	wsMessageCounter = 0;
	clearTimers();
}

function clearTimers(): void {
	if (processingTimer !== null) { clearTimeout(processingTimer); processingTimer = null; }
}
