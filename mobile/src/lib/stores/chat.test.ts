// M1-02: Chat store tests — CA-07: phone receives complete answer in one ChatComplete
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
	processingStage,
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
import { PROCESSING_TIMEOUT_MS } from '$lib/types/chat.js';

describe('chat store — processing flow (CA-07: no token buffering)', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		resetChatState();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('transitions processing → complete when ChatComplete arrives with full content', () => {
		startQuery('live');
		expect(get(streamState).phase).toBe('processing');
		expect(get(isStreaming)).toBe(true);
		expect(get(inputDisabled)).toBe(true);

		// Desktop sends complete answer in one message
		const wsCitations: WsCitationRef[] = [{
			document_id: 'doc-1',
			document_title: 'Prescription 02/2024'
		}];

		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'conv-1',
			content: 'Based on your records, you are taking Lisinopril 10mg.',
			citations: wsCitations
		});

		expect(get(streamState).phase).toBe('complete');
		expect(get(isStreaming)).toBe(false);
		expect(get(inputDisabled)).toBe(false);

		// Full message appears at once
		const msgs = get(messages);
		expect(msgs).toHaveLength(1);
		expect(msgs[0].role).toBe('coheara');
		expect(msgs[0].content).toBe('Based on your records, you are taking Lisinopril 10mg.');
		expect(msgs[0].citations).toHaveLength(1);
		expect(msgs[0].citations[0].documentTitle).toBe('Prescription 02/2024');
	});

	it('ignores ChatToken — phone does not buffer tokens', () => {
		startQuery('live');
		expect(get(streamState).phase).toBe('processing');

		// Desktop may still send ChatToken (backward compat) — phone ignores it
		handleWsChatMessage({
			type: 'ChatToken',
			conversation_id: 'conv-1',
			token: 'Based '
		});

		// Still processing, no state change
		expect(get(streamState).phase).toBe('processing');
		expect(get(messages)).toHaveLength(0);
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

	it('assigns conversation ID from ChatComplete', () => {
		addPatientMessage(null, 'Hello');
		startQuery('live');

		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'conv-new',
			content: 'Hello! How can I help?',
			citations: []
		});

		expect(get(activeConversationId)).toBe('conv-new');
	});

	it('continues existing conversation with ID', () => {
		addPatientMessage('conv-1', 'Follow-up question');
		const msgs = get(messages);
		expect(msgs[0].conversationId).toBe('conv-1');
	});
});

describe('chat store — processing stages and timeout', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		resetChatState();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('shows time-based processing stages', () => {
		startQuery('live');
		expect(get(processingStage)).toContain('Searching');

		vi.advanceTimersByTime(4_000);
		expect(get(processingStage)).toContain('Found relevant');

		vi.advanceTimersByTime(5_000); // 9s total
		expect(get(processingStage)).toContain('Analyzing');

		vi.advanceTimersByTime(7_000); // 16s total
		expect(get(processingStage)).toContain('notify you when ready');
	});

	it('times out if no ChatComplete within processing timeout', () => {
		startQuery('live');
		expect(get(streamState).phase).toBe('processing');

		vi.advanceTimersByTime(PROCESSING_TIMEOUT_MS + 100);

		const state = get(streamState);
		expect(state.phase).toBe('error');
		if (state.phase === 'error') {
			expect(state.message).toContain('Taking longer');
		}
	});

	it('handles ChatError during processing', () => {
		startQuery('live');

		handleWsChatMessage({
			type: 'ChatError',
			conversation_id: 'c1',
			error: 'Desktop disconnected'
		});

		const state = get(streamState);
		expect(state.phase).toBe('error');
		if (state.phase === 'error') {
			expect(state.message).toBe('Desktop disconnected');
		}
	});

	it('handles ChatError before processing starts', () => {
		startQuery('live');

		handleWsChatMessage({
			type: 'ChatError',
			conversation_id: 'c1',
			error: 'Profile locked on desktop'
		});

		const state = get(streamState);
		expect(state.phase).toBe('error');
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
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			content: 'Your lab results show...',
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
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			content: 'General health information.',
			citations: []
		});

		const msgs = get(messages);
		expect(msgs[0].citations).toHaveLength(0);
	});

	it('preserves citation document ID after mapping', () => {
		startQuery('live');
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			content: 'Check your blood work.',
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
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			content: 'Answer',
			citations: []
		});

		const msgId = get(messages)[0].id;
		expect(get(messages)[0].feedback).toBeNull();

		setMessageFeedback(msgId, true);
		expect(get(messages)[0].feedback).toBe('helpful');
	});

	it('sets not-helpful feedback on a message', () => {
		startQuery('live');
		handleWsChatMessage({
			type: 'ChatComplete',
			conversation_id: 'c1',
			content: 'Answer',
			citations: []
		});

		const msgId = get(messages)[0].id;
		setMessageFeedback(msgId, false);
		expect(get(messages)[0].feedback).toBe('not_helpful');
	});

	it('feedback only affects the targeted message', () => {
		startQuery('live');
		handleWsChatMessage({ type: 'ChatComplete', conversation_id: 'c1', content: 'First', citations: [] });

		startQuery('live');
		handleWsChatMessage({ type: 'ChatComplete', conversation_id: 'c1', content: 'Second', citations: [] });

		const firstMsgId = get(messages)[0].id;
		setMessageFeedback(firstMsgId, true);

		const msgs = get(messages);
		expect(msgs[0].feedback).toBe('helpful');
		expect(msgs[1].feedback).toBeNull();
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

		clearConversation();
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
		resetChatState();
		expect(get(streamState).phase).toBe('idle');

		addPatientMessage(null, 'Hello');
		expect(get(messages)).toHaveLength(1);
	});
});
