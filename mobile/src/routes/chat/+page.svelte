<!-- M1-02: Chat tab — conversational AI interface -->
<script lang="ts">
	import { tick } from 'svelte';
	import { isConnected, hasData } from '$lib/stores/connection.js';
	import {
		messages,
		streamState,
		isStreaming,
		inputDisabled,
		streamingContent,
		addPatientMessage,
		setMessageFeedback,
		startQuery,
		deferQuestion,
		clearConversation,
		activeConversationId,
		DEFAULT_QUICK_QUESTIONS
	} from '$lib/stores/chat.js';
	import type { ChatSource } from '$lib/types/chat.js';
	import type { Citation } from '$lib/types/chat.js';
	import MessageBubble from '$lib/components/chat/MessageBubble.svelte';
	import QuickQuestions from '$lib/components/chat/QuickQuestions.svelte';
	import ChatDisclaimer from '$lib/components/chat/ChatDisclaimer.svelte';
	import SourceIndicator from '$lib/components/chat/SourceIndicator.svelte';
	import CitationDetailSheet from '$lib/components/chat/CitationDetailSheet.svelte';

	let inputText = $state('');
	let messagesArea: HTMLDivElement | undefined = $state();
	let userScrolledUp = $state(false);
	let selectedCitation: Citation | null = $state(null);

	function scrollToBottom(): void {
		if (messagesArea && !userScrolledUp) {
			messagesArea.scrollTop = messagesArea.scrollHeight;
		}
	}

	function handleMessagesScroll(): void {
		if (!messagesArea) return;
		const { scrollTop, scrollHeight, clientHeight } = messagesArea;
		userScrolledUp = scrollHeight - scrollTop - clientHeight > 50;
	}

	// Auto-scroll when messages change or streaming content updates (RS-M1-02-004)
	$effect(() => {
		void $messages;
		void $streamingContent;
		tick().then(() => scrollToBottom());
	});

	const chatSource: ChatSource = $derived(
		$isConnected ? 'live' : ($hasData ? 'cached' : 'unavailable')
	);

	function handleSend(): void {
		const text = inputText.trim();
		if (!text) return;

		inputText = '';

		if (chatSource === 'unavailable') {
			deferQuestion(text);
			return;
		}

		addPatientMessage($activeConversationId, text);
		startQuery(chatSource);
	}

	function handleQuickQuestion(text: string): void {
		inputText = text;
		handleSend();
	}

	function handleFeedback(messageId: string, helpful: boolean): void {
		setMessageFeedback(messageId, helpful);
	}

	function handleCitationTap(documentId: string): void {
		// Find the citation across all messages
		for (const msg of $messages) {
			const found = msg.citations.find(c => c.documentId === documentId);
			if (found) {
				selectedCitation = found;
				return;
			}
		}
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			handleSend();
		}
	}
</script>

<div class="chat-screen">
	<!-- Header with source indicator -->
	<div class="chat-header">
		<h1>Chat</h1>
		<SourceIndicator source={chatSource} />
	</div>

	{#if chatSource === 'unavailable' && !$hasData}
		<!-- Offline without data -->
		<div class="offline-message">
			<p>Chat is available when connected to your desktop.</p>
			<p>Your medications and journal are still available offline.</p>
			<div class="offline-actions">
				<a href="/meds" class="offline-link">View Medications</a>
				<a href="/journal" class="offline-link">Open Journal</a>
			</div>
		</div>
	{:else}
		<!-- Messages area -->
		<div class="messages-area" bind:this={messagesArea} onscroll={handleMessagesScroll} role="log" aria-label="Chat messages">
			{#each $messages as message (message.id)}
				<MessageBubble
					{message}
					onFeedback={handleFeedback}
					onCitationTap={handleCitationTap}
				/>
			{/each}

			{#if $streamState.phase === 'loading'}
				<div class="typing-indicator" aria-label="Coheara is thinking">
					<span class="dot-pulse"></span>
					Thinking&hellip;
				</div>
			{/if}

			{#if $streamState.phase === 'streaming'}
				<div class="streaming-bubble" role="status" aria-label="Coheara is responding">
					<div class="avatar" aria-hidden="true">C</div>
					<div class="bubble-content">
						<p>{$streamingContent}</p>
						<span class="typing-cursor" aria-hidden="true">|</span>
					</div>
				</div>
			{/if}

			{#if $streamState.phase === 'error'}
				<div class="error-message" role="alert">
					{$streamState.message}
				</div>
			{/if}
		</div>

		<!-- Quick questions (Mamadou) -->
		{#if $messages.length === 0}
			<QuickQuestions
				questions={DEFAULT_QUICK_QUESTIONS}
				onSelect={handleQuickQuestion}
				disabled={$inputDisabled}
			/>
		{/if}

		<!-- Disclaimer (Dr. Diallo — always visible) -->
		<ChatDisclaimer />

		<!-- Input area -->
		<div class="input-area">
			<textarea
				class="chat-input"
				placeholder="Type a question&hellip;"
				bind:value={inputText}
				disabled={$inputDisabled || chatSource === 'unavailable'}
				onkeydown={handleKeydown}
				rows={1}
				aria-label="Type your question"
			></textarea>
			<button
				class="send-btn"
				disabled={$inputDisabled || !inputText.trim()}
				onclick={handleSend}
				aria-label="Send message"
			>
				Send
			</button>
		</div>
	{/if}
</div>

<!-- Citation detail bottom sheet (RS-M1-02-003) -->
<CitationDetailSheet
	citation={selectedCitation}
	onClose={() => selectedCitation = null}
/>

<style>
	.chat-screen {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.chat-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		border-bottom: 1px solid #E7E5E4;
	}

	.chat-header h1 {
		font-size: 20px;
		font-weight: 700;
		margin: 0;
	}

	.messages-area {
		flex: 1;
		overflow-y: auto;
		padding: 16px;
		-webkit-overflow-scrolling: touch;
	}

	.typing-indicator {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px 16px;
		color: var(--color-text-muted);
		font-size: 14px;
	}

	.streaming-bubble {
		display: flex;
		gap: 8px;
		margin-bottom: 12px;
		max-width: 85%;
	}

	.streaming-bubble .avatar {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: var(--color-primary);
		color: white;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 14px;
		font-weight: 700;
		flex-shrink: 0;
	}

	.streaming-bubble .bubble-content {
		padding: 12px 16px;
		background: white;
		border: 1px solid #E7E5E4;
		border-radius: 16px;
		border-bottom-left-radius: 4px;
	}

	.streaming-bubble .bubble-content p {
		margin: 0;
		font-size: 16px;
		line-height: 1.5;
		white-space: pre-wrap;
	}

	.typing-cursor {
		animation: blink 1s infinite;
		color: var(--color-primary);
	}

	@keyframes blink {
		0%, 50% { opacity: 1; }
		51%, 100% { opacity: 0; }
	}

	.error-message {
		padding: 12px 16px;
		margin: 8px 0;
		background: #FEF2F2;
		border-radius: 8px;
		color: #991B1B;
		font-size: 14px;
	}

	.input-area {
		display: flex;
		gap: 8px;
		padding: 8px 16px 12px;
		border-top: 1px solid #E7E5E4;
		background: var(--color-surface);
	}

	.chat-input {
		flex: 1;
		padding: 10px 14px;
		border: 1px solid #D6D3D1;
		border-radius: 20px;
		font-size: 16px;
		resize: none;
		min-height: var(--min-touch-target);
		font-family: inherit;
	}

	.chat-input:focus {
		outline: 2px solid var(--color-primary);
		outline-offset: -1px;
	}

	.send-btn {
		padding: 10px 20px;
		background: var(--color-primary);
		color: white;
		border: none;
		border-radius: 20px;
		font-size: 16px;
		font-weight: 600;
		cursor: pointer;
		min-height: var(--min-touch-target);
	}

	.send-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.offline-message {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		flex: 1;
		text-align: center;
		padding: 24px;
	}

	.offline-message p {
		color: var(--color-text-muted);
		font-size: 16px;
		margin: 0 0 8px;
	}

	.offline-actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin-top: 16px;
	}

	.offline-link {
		padding: 12px 24px;
		background: var(--color-primary);
		color: white;
		border-radius: 12px;
		text-decoration: none;
		font-size: 16px;
		font-weight: 600;
		text-align: center;
		min-height: var(--min-touch-target);
		display: flex;
		align-items: center;
		justify-content: center;
	}
</style>
