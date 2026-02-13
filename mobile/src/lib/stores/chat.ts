// M1-02: Chat store — conversation state, streaming, deferred questions
import { writable, derived, get } from 'svelte/store';
import type {
	ChatMessage,
	ChatSource,
	Citation,
	ConversationSummary,
	DeferredQuestion,
	QuickQuestion,
	StreamState,
	WsChatMessage
} from '$lib/types/chat.js';
import { LOADING_TIMEOUT_MS, TOKEN_TIMEOUT_MS } from '$lib/types/chat.js';

// --- Core stores ---

/** Current conversation messages */
export const messages = writable<ChatMessage[]>([]);

/** Current conversation ID */
export const activeConversationId = writable<string | null>(null);

/** Conversation list */
export const conversations = writable<ConversationSummary[]>([]);

/** Streaming state */
export const streamState = writable<StreamState>({ phase: 'idle' });

/** Deferred question queue (offline questions) */
export const deferredQuestions = writable<DeferredQuestion[]>([]);

// --- Derived stores ---

/** Whether a response is currently streaming */
export const isStreaming = derived(streamState, ($s) =>
	$s.phase === 'loading' || $s.phase === 'streaming'
);

/** Whether the chat input should be disabled */
export const inputDisabled = derived(streamState, ($s) =>
	$s.phase === 'loading' || $s.phase === 'streaming'
);

/** Number of pending deferred questions */
export const deferredCount = derived(deferredQuestions, ($q) =>
	$q.filter((q) => !q.answered).length
);

/** Accumulated streaming tokens for display */
export const streamingContent = derived(streamState, ($s) =>
	$s.phase === 'streaming' ? $s.tokens : ''
);

// --- Quick questions (Mamadou) ---

export const DEFAULT_QUICK_QUESTIONS: readonly QuickQuestion[] = [
	{ text: 'What are my medications?', category: 'medications' },
	{ text: 'When is my next appointment?', category: 'appointments' },
	{ text: 'What should I tell my doctor?', category: 'general' }
] as const;

// --- Streaming management ---

let loadingTimer: ReturnType<typeof setTimeout> | null = null;
let tokenTimer: ReturnType<typeof setTimeout> | null = null;

/** Start a new chat query — sets loading state */
export function startQuery(source: ChatSource): void {
	const now = Date.now();
	streamState.set({ phase: 'loading', startedAt: now });

	clearTimers();
	loadingTimer = setTimeout(() => {
		const current = get(streamState);
		if (current.phase === 'loading') {
			streamState.set({
				phase: 'error',
				message: 'Taking longer than usual. Check your desktop is active.'
			});
		}
	}, LOADING_TIMEOUT_MS);
}

/** Handle incoming WebSocket chat message */
export function handleWsChatMessage(msg: WsChatMessage): void {
	switch (msg.type) {
		case 'ChatToken':
			handleToken(msg.conversationId, msg.token);
			break;
		case 'ChatComplete':
			handleComplete(msg.conversationId, msg.messageId, msg.citations);
			break;
		case 'ChatError':
			handleStreamError(msg.conversationId, msg.error);
			break;
	}
}

function handleToken(conversationId: string, token: string): void {
	clearTimers();

	streamState.update(($s) => {
		if ($s.phase === 'loading') {
			return { phase: 'streaming', tokens: token, conversationId };
		}
		if ($s.phase === 'streaming') {
			return { ...$s, tokens: $s.tokens + token };
		}
		return $s;
	});

	// Reset token timeout
	tokenTimer = setTimeout(() => {
		const current = get(streamState);
		if (current.phase === 'streaming') {
			streamState.set({
				phase: 'error',
				message: 'Connection lost. Your answer may be incomplete.',
				partial: current.tokens
			});
		}
	}, TOKEN_TIMEOUT_MS);
}

function handleComplete(conversationId: string, messageId: string, citations: Citation[]): void {
	clearTimers();

	const current = get(streamState);
	const content = current.phase === 'streaming' ? current.tokens : '';

	// Add complete message to messages list
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

function handleStreamError(conversationId: string, error: string): void {
	clearTimers();

	const current = get(streamState);
	const partial = current.phase === 'streaming' ? current.tokens : undefined;

	streamState.set({ phase: 'error', message: error, partial });
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
	clearTimers();
}

function clearTimers(): void {
	if (loadingTimer !== null) { clearTimeout(loadingTimer); loadingTimer = null; }
	if (tokenTimer !== null) { clearTimeout(tokenTimer); tokenTimer = null; }
}
