<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { listen } from '@tauri-apps/api/event';
  import {
    startConversation,
    sendChatMessage,
    getConversationMessages,
    getPromptSuggestions,
  } from '$lib/api/chat';
  import type {
    Message,
    ChatStreamEvent,
    CitationView,
    PromptSuggestion,
  } from '$lib/types/chat';
  import { profile } from '$lib/stores/profile.svelte';
  import { ai } from '$lib/stores/ai.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import MessageBubble from './MessageBubble.svelte';
  import StreamingIndicator from './StreamingIndicator.svelte';
  import ChatEmptyState from './ChatEmptyState.svelte';
  import QuickActionChips from './QuickActionChips.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import { ArrowUpIcon } from '$lib/components/icons/md';
  import { soundManager } from '$lib/utils/sound';

  interface Props {
    initialConversationId?: string;
    prefill?: string;
  }
  let { initialConversationId, prefill }: Props = $props();

  // Conversation state
  let currentConversationId: string | null = $state(initialConversationId ?? null);
  let messages: Message[] = $state([]);
  let suggestions: PromptSuggestion[] = $state([]);
  let conversationTitle = $state($t('chat.new_conversation_title'));

  // Input state
  let inputText = $state('');
  let isSending = $state(false);

  // Streaming state
  let isStreaming = $state(false);
  let streamingText = $state('');
  let pendingCitations: CitationView[] = $state([]);
  let responseConfidence: number | null = $state(null);
  let streamError: string | null = $state(null);

  // UI state
  let messageContainer: HTMLElement | undefined = $state(undefined);
  let textareaEl: HTMLTextAreaElement | undefined = $state(undefined);

  // Auto-grow: scrollHeight-based resize (ChatGPT/Claude.ai pattern)
  const INPUT_MAX_H = 200;

  function autoGrow() {
    if (!textareaEl) return;
    textareaEl.style.height = 'auto';
    const h = Math.min(textareaEl.scrollHeight, INPUT_MAX_H);
    textareaEl.style.height = `${h}px`;
    textareaEl.style.overflowY = textareaEl.scrollHeight > INPUT_MAX_H ? 'auto' : 'hidden';
  }

  // Resize on programmatic value changes (send reset, prefill, suggestion tap)
  $effect(() => {
    inputText;
    autoGrow();
  });

  // Derived
  let hasMessages = $derived(messages.length > 0);
  let canSend = $derived(inputText.trim().length > 0 && !isSending && !isStreaming);

  function scrollToBottom() {
    if (messageContainer) {
      requestAnimationFrame(() => {
        messageContainer!.scrollTop = messageContainer!.scrollHeight;
      });
    }
  }

  async function loadMessages(convId: string) {
    try {
      messages = await getConversationMessages(convId);
      // Derive title from first patient message
      const firstPatient = messages.find(m => m.role === 'patient');
      if (firstPatient) {
        const text = firstPatient.content.trim();
        conversationTitle = text.length > 50 ? text.slice(0, 50) + '...' : text;
      }
      scrollToBottom();
    } catch (e) {
      console.error('Failed to load messages:', e);
    }
  }

  async function handleNewConversation() {
    // UA02-05: Lazy creation — don't persist to DB until first message is sent.
    // If current conversation is already empty, just stay on it.
    if (currentConversationId && messages.length === 0) {
      return;
    }
    currentConversationId = null;
    messages = [];
    streamingText = '';
    pendingCitations = [];
    responseConfidence = null;
    streamError = null;
    conversationTitle = $t('chat.new_conversation_title');
    suggestions = await getPromptSuggestions().catch(() => []);
  }

  // React to navigation prop changes (History → Chat, or sidebar "Ask" click)
  $effect(() => {
    const convId = initialConversationId;
    const pre = prefill;

    // Don't interrupt active streaming
    if (isStreaming) return;

    if (convId && convId !== currentConversationId) {
      // Navigating to a specific conversation from History
      currentConversationId = convId;
      streamingText = '';
      pendingCitations = [];
      responseConfidence = null;
      streamError = null;
      loadMessages(convId);
    } else if (!convId && currentConversationId) {
      // Sidebar "Ask" click — start new conversation
      handleNewConversation();
    }

    // Handle prefill from other screens
    if (pre) {
      inputText = pre;
    }
  });

  async function handleSend() {
    if (!canSend) return;

    const text = inputText.trim();
    inputText = '';

    if (!currentConversationId) {
      try {
        const id = await startConversation();
        currentConversationId = id;
      } catch (e) {
        streamError = $t('chat.conversation_start_error');
        return;
      }
    }

    // Update title from first message
    if (messages.length === 0) {
      conversationTitle = text.length > 50 ? text.slice(0, 50) + '...' : text;
    }

    const patientMessage: Message = {
      id: crypto.randomUUID(),
      conversation_id: currentConversationId!,
      role: 'patient',
      content: text,
      timestamp: new Date().toISOString(),
      source_chunks: null,
      confidence: null,
      feedback: null,
    };
    messages = [...messages, patientMessage];
    scrollToBottom();

    isSending = true;
    isStreaming = true;
    streamingText = '';
    pendingCitations = [];
    responseConfidence = null;
    streamError = null;

    const unlisten = await listen<ChatStreamEvent>('chat-stream', (event) => {
      const { conversation_id, chunk } = event.payload;
      if (conversation_id !== currentConversationId) return;

      switch (chunk.type) {
        case 'Token':
          streamingText += chunk.text;
          scrollToBottom();
          break;
        case 'Citation':
          pendingCitations = [...pendingCitations, chunk.citation];
          break;
        case 'Done':
          streamingText = chunk.full_text;
          responseConfidence = chunk.confidence;
          isStreaming = false;

          const cohearaMessage: Message = {
            id: crypto.randomUUID(),
            conversation_id: currentConversationId!,
            role: 'coheara',
            content: chunk.full_text,
            timestamp: new Date().toISOString(),
            source_chunks: pendingCitations.length > 0 ? JSON.stringify(pendingCitations) : null,
            confidence: chunk.confidence,
            feedback: null,
          };
          messages = [...messages, cohearaMessage];
          streamingText = '';
          pendingCitations = [];
          soundManager.play('completion');
          scrollToBottom();
          break;
        case 'Error':
          streamError = chunk.message;
          isStreaming = false;
          ai.handleOperationFailure(new Error(chunk.message));
          break;
      }
    });

    try {
      await sendChatMessage(currentConversationId!, text);
    } catch (e) {
      streamError = e instanceof Error ? e.message : String(e);
      isStreaming = false;
      ai.handleOperationFailure(e);
    } finally {
      unlisten();
      isSending = false;
    }
  }

  function formatSuggestionText(s: PromptSuggestion): string {
    return $t(s.template_key, { values: s.params });
  }

  function handleSuggestionTap(suggestion: PromptSuggestion) {
    const text = formatSuggestionText(suggestion);
    if (suggestion.intent === 'query') {
      inputText = text;
      handleSend();
    } else {
      inputText = text;
      // Focus the textarea for expression — user completes then sends
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  onMount(async () => {
    suggestions = await getPromptSuggestions();

    if (currentConversationId) {
      await loadMessages(currentConversationId);
    }

    // Spec 48 [CA-05]: Pre-fill input from post-review CTA or other navigation
    if (prefill) {
      inputText = prefill;
    }
  });
</script>

<div class="flex flex-col h-full bg-stone-50 dark:bg-gray-950">
  <!-- Header — only shows title when conversation has messages -->
  {#if hasMessages}
    <header class="flex items-center px-4 py-3 bg-stone-50 dark:bg-gray-950">
      <h1 class="flex-1 text-base font-medium text-stone-800 dark:text-gray-100 truncate">
        {conversationTitle}
      </h1>
    </header>
  {/if}

  <!-- Messages area -->
  <div
    class="flex-1 overflow-y-auto px-4 py-4"
    bind:this={messageContainer}
    role="log"
    aria-label={$t('chat.messages_aria')}
    aria-live="polite"
  >
    {#if !hasMessages && !isStreaming}
      <ChatEmptyState
        {suggestions}
        onSuggestionTap={handleSuggestionTap}
        onNavigate={(screen: string) => navigation.navigate(screen)}
      />
    {:else}
      <div class="flex flex-col gap-4 max-w-2xl mx-auto">
        {#each messages as message (message.id)}
          <MessageBubble
            {message}
          />
        {/each}

        {#if isStreaming && streamingText}
          <div class="flex items-start gap-2">
            <div class="w-8 h-8 rounded-full bg-[var(--color-interactive)] flex items-center
                        justify-center text-white text-sm font-bold flex-shrink-0 mt-1">
              {$t('chat.avatar_initial')}
            </div>
            <div class="max-w-[85%] bg-white dark:bg-gray-900 border border-stone-100 dark:border-gray-800 rounded-2xl rounded-bl-md
                        px-4 py-3 shadow-sm">
              <p class="text-stone-800 dark:text-gray-100 text-base leading-relaxed whitespace-pre-wrap">
                {streamingText}<span class="animate-pulse text-[var(--color-interactive)]">|</span>
              </p>
            </div>
          </div>
        {:else if isStreaming && !streamingText}
          <StreamingIndicator />
        {/if}

        {#if streamError}
          <div class="max-w-[85%] ml-10">
            <ErrorBanner
              message={streamError}
              severity="warning"
              guidance={$t('chat.stream_error_guidance')}
              onDismiss={() => { streamError = null; }}
            />
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Quick action chips (expression suggestions) -->
  {#if hasMessages}
    <QuickActionChips
      {suggestions}
      {isStreaming}
      onChipTap={handleSuggestionTap}
    />
  {/if}

  <!-- Input bar -->
  <div class="bg-stone-50 dark:bg-gray-950 px-4 py-3">
    <div class="max-w-2xl mx-auto">
      <!-- Input capsule — Claude.ai pattern: shadow-based boundary, no ring -->
      <div class="input-capsule bg-white dark:bg-gray-800 rounded-[20px]
                  border border-transparent cursor-text">
        <div class="flex flex-col m-3.5 gap-2.5">
          <textarea
            bind:this={textareaEl}
            class="chat-textarea block w-full text-base leading-relaxed
                   border-0 ring-0 focus:ring-0 focus:outline-none
                   bg-transparent text-stone-800 dark:text-gray-100
                   resize-none pl-1.5 pt-1.5
                   placeholder:text-stone-400 dark:placeholder:text-gray-500"
            style="min-height: 24px; max-height: {INPUT_MAX_H}px; overflow-y: hidden;"
            placeholder={$t('chat.input_placeholder')}
            bind:value={inputText}
            oninput={autoGrow}
            onkeydown={handleKeydown}
            rows={1}
            disabled={isStreaming}
            aria-label={$t('chat.input_aria')}
          ></textarea>
          <div class="flex items-center justify-end">
            <button
              class="w-8 h-8 flex items-center justify-center rounded-lg transition-colors
                     {canSend
                       ? 'bg-[var(--color-success)] text-white hover:opacity-90'
                       : 'bg-stone-100 dark:bg-gray-700 text-stone-400 dark:text-gray-500 cursor-not-allowed'}"
              onclick={handleSend}
              disabled={!canSend}
              aria-label={$t('chat.send_aria')}
            >
              <ArrowUpIcon class="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
      <!-- Medical AI disclaimer -->
      <p class="text-xs text-stone-400 dark:text-gray-500 text-center mt-2">
        {$t('chat.ai_disclaimer')}
      </p>
    </div>
  </div>
</div>

<style>
  /* Container — shadow-based boundary (Claude.ai pattern) */
  .input-capsule {
    box-shadow:
      0 0.25rem 1.25rem rgba(0, 0, 0, 0.035),
      0 0 0 0.5px rgba(0, 0, 0, 0.06);
    transition: box-shadow 200ms;
  }
  .input-capsule:hover {
    box-shadow:
      0 0.25rem 1.25rem rgba(0, 0, 0, 0.035),
      0 0 0 0.5px rgba(0, 0, 0, 0.12);
  }
  .input-capsule:focus-within {
    box-shadow:
      0 0.25rem 1.25rem rgba(0, 0, 0, 0.075),
      0 0 0 0.5px rgba(0, 0, 0, 0.12);
  }
  :global(.dark) .input-capsule {
    box-shadow:
      0 0.25rem 1.25rem rgba(0, 0, 0, 0.15),
      0 0 0 0.5px rgba(255, 255, 255, 0.08);
  }
  :global(.dark) .input-capsule:hover {
    box-shadow:
      0 0.25rem 1.25rem rgba(0, 0, 0, 0.15),
      0 0 0 0.5px rgba(255, 255, 255, 0.16);
  }
  :global(.dark) .input-capsule:focus-within {
    box-shadow:
      0 0.25rem 1.25rem rgba(0, 0, 0, 0.3),
      0 0 0 0.5px rgba(255, 255, 255, 0.16);
  }

  /* Textarea — strip all focus chrome */
  .chat-textarea:focus {
    outline: none !important;
    box-shadow: none !important;
  }
</style>
