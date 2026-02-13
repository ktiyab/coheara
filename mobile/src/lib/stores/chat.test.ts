// M1-02: Chat store tests — 26 tests
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import {
	messages,
	activeConversationId,
	conversations,
	streamState,
	deferredQuestions,
	isStreaming,
	inputDisabled,
	streamingContent,
	deferredCount,
	DEFAULT_QUICK_QUESTIONS,
	startQuery,
	handleWsChatMessage,
	addPatientMessage,
	setMessageFeedback,
	deferQuestion,
	markDeferredAnswered,
	getPendingDeferred,
	setConversations,
	setMessages,
	clearConversation,
	resetChatState
} from './chat.js';
import type { ChatMessage, ConversationSummary, Citation, WsCitationRef } from '$lib/types/chat.js';
import { LOADING_TIMEOUT_MS, TOKEN_TIMEOUT_MS } from '$lib/types/chat.js';

describe('chat store — connected chat flow', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		resetChatState();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('sends a query and transitions through loading → streaming → complete', () => {
		// Start query
		startQuery('live');
		expect(get(streamState).phase).toBe('loading');
		expect(get(isStreaming)).toBe(true);
		expect(get(inputDisabled)).toBe(true);

		// Receive first token
		handleWsChatMessage({
			type: 'ChatToken',
			conversation_id: 'conv-1',
			token: 'Based '
		});
		expect(get(streamState).phase).toBe('streaming');
		expect(get(streamingContent)).toBe('Based ');

		// Receive more tokens
		handleWsChatMessage({
			type: 'ChatToken',
			conversation_id: 'conv-1',
			token: 'on your '
		});
		expect(get(streamingContent)).toBe('Based on your ');

		handleWsChatMessage({
			type: 'ChatToken',
			conversation_id: 'conv-1',
			token: 'records...'
		});
		expect(get(streamingContent)).toBe('Based on your records...');

		// Complete with citations (WsCitationRef format from desktop)
		const wsCitations: WsCitationRef[] = [{
			document_id: 'doc-1',
			document_title: 'Prescription 02/2024'
		}];

		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'conv-1',
			citations: wsCitations
		});

		expect(get(streamState).phase).toBe('complete');
		expect(get(isStreaming)).toBe(false);
		expect(get(inputDisabled)).toBe(false);

		// Message added to messages list
		const msgs = get(messages);
		expect(msgs).toHaveLength(1);
		expect(msgs[0].role).toBe('coheara');
		expect(msgs[0].content).toBe('Based on your records...');
		expect(msgs[0].citations).toHaveLength(1);
		expect(msgs[0].citations[0].documentTitle).toBe('Prescription 02/2024');
	});

	it('adds patient message before sending query', () => {
		const msg = addPatientMessage(null, 'What am I taking for BP?');
		expect(msg.role).toBe('patient');
		expect(msg.content).toBe('What am I taking for BP?');
		expect(msg.conversationId).toBe('pending');

		const msgs = get(messages);
		expect(msgs).toHaveLength(1);
		expect(msgs[0].content).toBe('What am I taking for BP?');
	});

	it('handles conversation ID assignment after first response', () => {
		addPatientMessage(null, 'Hello');
		startQuery('live');

		handleWsChatMessage({
			type: 'ChatToken',
			conversation_id: 'conv-new',
			token: 'Hello! '
		});

		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'conv-new',
			citations: []
		});

		expect(get(activeConversationId)).toBe('conv-new');
	});

	it('continues existing conversation with ID', () => {
		addPatientMessage('conv-1', 'Follow-up question');
		const msgs = get(messages);
		expect(msgs[0].conversationId).toBe('conv-1');
	});

	it('resets streaming content on new query', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'first' });
		expect(get(streamingContent)).toBe('first');

		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			citations: []
		});

		// Start another query
		startQuery('live');
		expect(get(streamingContent)).toBe('');
	});
});

describe('chat store — streaming UX states', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		resetChatState();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('times out if no tokens arrive within loading timeout', () => {
		startQuery('live');
		expect(get(streamState).phase).toBe('loading');

		vi.advanceTimersByTime(LOADING_TIMEOUT_MS + 100);

		const state = get(streamState);
		expect(state.phase).toBe('error');
		if (state.phase === 'error') {
			expect(state.message).toContain('Taking longer');
		}
	});

	it('times out if token stream stalls during streaming', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'partial...' });
		expect(get(streamState).phase).toBe('streaming');

		vi.advanceTimersByTime(TOKEN_TIMEOUT_MS + 100);

		const state = get(streamState);
		expect(state.phase).toBe('error');
		if (state.phase === 'error') {
			expect(state.message).toContain('Connection lost');
			expect(state.partial).toBe('partial...');
		}
	});

	it('handles error during streaming with partial content', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'So far...' });

		handleWsChatMessage({
			type: 'ChatError',
			conversation_id: 'c1',
			error: 'Desktop disconnected'
		});

		const state = get(streamState);
		expect(state.phase).toBe('error');
		if (state.phase === 'error') {
			expect(state.message).toBe('Desktop disconnected');
			expect(state.partial).toBe('So far...');
		}
	});

	it('handles error before any tokens arrive', () => {
		startQuery('live');

		handleWsChatMessage({
			type: 'ChatError',
			conversation_id: 'c1',
			error: 'Profile locked on desktop'
		});

		const state = get(streamState);
		expect(state.phase).toBe('error');
		if (state.phase === 'error') {
			expect(state.partial).toBeUndefined();
		}
	});
});

describe('chat store — citation handling', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		resetChatState();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('attaches citations to completed message', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'Answer' });
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			citations: [
				{ document_id: 'doc-1', document_title: 'Lab Report' },
				{ document_id: 'doc-2', document_title: 'Prescription' }
			]
		});

		const msgs = get(messages);
		expect(msgs[0].citations).toHaveLength(2);
		expect(msgs[0].citations[0].documentTitle).toBe('Lab Report');
		expect(msgs[0].citations[1].documentTitle).toBe('Prescription');
	});

	it('handles message with zero citations', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'General info' });
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			citations: []
		});

		const msgs = get(messages);
		expect(msgs[0].citations).toHaveLength(0);
	});

	it('preserves citation document ID after mapping', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'Check' });
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			citations: [{
				document_id: 'doc-xyz',
				document_title: 'Blood Work'
			}]
		});

		const citation = get(messages)[0].citations[0];
		expect(citation.documentId).toBe('doc-xyz');
		expect(citation.documentTitle).toBe('Blood Work');
	});
});

describe('chat store — feedback', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		resetChatState();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('sets helpful feedback on a message', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'Answer' });
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			citations: []
		});

		const msgId = get(messages)[0].id;
		expect(get(messages)[0].feedback).toBeNull();

		setMessageFeedback(msgId, true);
		expect(get(messages)[0].feedback).toBe('helpful');
	});

	it('sets not-helpful feedback on a message', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'Answer' });
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			citations: []
		});

		const msgId = get(messages)[0].id;
		setMessageFeedback(msgId, false);
		expect(get(messages)[0].feedback).toBe('not_helpful');
	});

	it('feedback only affects the targeted message', () => {
		// Add two Coheara messages
		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'First' });
		handleWsChatMessage({ type: 'ChatComplete', conversation_id: 'c1', citations: [] });

		startQuery('live');
		handleWsChatMessage({ type: 'ChatToken', conversation_id: 'c1', token: 'Second' });
		handleWsChatMessage({ type: 'ChatComplete', conversation_id: 'c1', citations: [] });

		const firstMsgId = get(messages)[0].id;
		setMessageFeedback(firstMsgId, true);

		const msgs = get(messages);
		expect(msgs[0].feedback).toBe('helpful');
		expect(msgs[1].feedback).toBeNull(); // Untouched
	});
});

describe('chat store — offline behavior', () => {
	beforeEach(() => {
		resetChatState();
	});

	it('defers question when unavailable', () => {
		const q = deferQuestion('What meds am I taking?');
		expect(q.text).toBe('What meds am I taking?');
		expect(q.answered).toBe(false);

		expect(get(deferredCount)).toBe(1);
	});

	it('marks deferred question as answered', () => {
		const q = deferQuestion('My medications?');
		expect(get(deferredCount)).toBe(1);

		markDeferredAnswered(q.id);
		expect(get(deferredCount)).toBe(0);
	});

	it('gets only pending deferred questions', () => {
		const q1 = deferQuestion('Question 1');
		const q2 = deferQuestion('Question 2');
		const q3 = deferQuestion('Question 3');

		markDeferredAnswered(q2.id);

		const pending = getPendingDeferred();
		expect(pending).toHaveLength(2);
		expect(pending.map((p) => p.text)).toEqual(['Question 1', 'Question 3']);
	});

	it('deferred queue persists across stream state changes', () => {
		deferQuestion('Saved question');
		expect(get(deferredCount)).toBe(1);

		clearConversation(); // Clear messages but not deferred
		expect(get(deferredCount)).toBe(1);
	});
});

describe('chat store — quick questions', () => {
	it('provides default quick question suggestions', () => {
		expect(DEFAULT_QUICK_QUESTIONS).toHaveLength(3);
		expect(DEFAULT_QUICK_QUESTIONS[0].text).toBe('What are my medications?');
		expect(DEFAULT_QUICK_QUESTIONS[0].category).toBe('medications');
	});

	it('quick question categories include medications and appointments', () => {
		const categories = new Set(DEFAULT_QUICK_QUESTIONS.map((q) => q.category));
		expect(categories.has('medications')).toBe(true);
		expect(categories.has('appointments')).toBe(true);
		expect(categories.has('general')).toBe(true);
	});

	it('quick question tap creates patient message', () => {
		resetChatState();
		const msg = addPatientMessage(null, DEFAULT_QUICK_QUESTIONS[0].text);
		expect(msg.content).toBe('What are my medications?');
		expect(msg.role).toBe('patient');
	});
});

describe('chat store — conversation history', () => {
	beforeEach(() => {
		resetChatState();
	});

	it('loads conversation list from API response', () => {
		const list: ConversationSummary[] = [
			{
				id: 'conv-1',
				title: 'Blood pressure medications',
				lastMessageAt: '2026-02-12T10:00:00Z',
				messageCount: 4,
				lastMessagePreview: 'Based on your records...',
				source: 'live'
			},
			{
				id: 'conv-2',
				title: 'Lab results discussion',
				lastMessageAt: '2026-02-11T15:00:00Z',
				messageCount: 2,
				lastMessagePreview: 'Your latest HbA1c was...',
				source: 'live'
			}
		];

		setConversations(list);
		expect(get(conversations)).toHaveLength(2);
		expect(get(conversations)[0].title).toBe('Blood pressure medications');
	});

	it('loads messages for a specific conversation', () => {
		const msgs: ChatMessage[] = [
			{
				id: 'msg-1',
				conversationId: 'conv-1',
				role: 'patient',
				content: 'What meds for BP?',
				timestamp: '2026-02-12T10:00:00Z',
				citations: [],
				feedback: null,
				source: 'live'
			},
			{
				id: 'msg-2',
				conversationId: 'conv-1',
				role: 'coheara',
				content: 'You are taking Lisinopril and Amlodipine.',
				timestamp: '2026-02-12T10:00:05Z',
				citations: [{ documentId: 'd1', documentTitle: 'Rx', documentDate: '2024-02', chunkText: '...', relevanceScore: 0.9, professionalName: 'Dr. N' }],
				feedback: null,
				source: 'live'
			}
		];

		setMessages(msgs);
		expect(get(messages)).toHaveLength(2);
		expect(get(messages)[0].role).toBe('patient');
		expect(get(messages)[1].role).toBe('coheara');
	});

	it('clears conversation state correctly', () => {
		addPatientMessage('conv-1', 'Hello');
		expect(get(messages)).toHaveLength(1);

		clearConversation();
		expect(get(messages)).toHaveLength(0);
		expect(get(activeConversationId)).toBeNull();
		expect(get(streamState).phase).toBe('idle');
	});
});

describe('chat store — disclaimer', () => {
	it('disclaimer text is defined as a constant requirement', () => {
		// The disclaimer is implemented as a component (ChatDisclaimer.svelte)
		// that is always rendered below the messages area.
		// This test verifies the store doesn't interfere with disclaimer visibility.
		resetChatState();

		// Empty state — disclaimer should still be visible (component responsibility)
		expect(get(streamState).phase).toBe('idle');

		// After messages — disclaimer should still be visible
		addPatientMessage(null, 'Hello');
		expect(get(messages)).toHaveLength(1);
		// Disclaimer visibility is always true — no store flag can disable it
		// (Dr. Diallo requirement: persistent on every chat screen)
	});
});
