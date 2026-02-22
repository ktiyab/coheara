<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from 'svelte-i18n';
  import { listen } from '@tauri-apps/api/event';
  import {
    startConversation,
    sendChatMessage,
    getConversationMessages,
    listConversations,
    getPromptSuggestions,
    deleteConversation,
  } from '$lib/api/chat';
  import type {
    Message,
    ConversationSummary,
    ChatStreamEvent,
    CitationView,
    PromptSuggestion,
  } from '$lib/types/chat';
  import { profile } from '$lib/stores/profile.svelte';
  import { ai } from '$lib/stores/ai.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import MessageBubble from './MessageBubble.svelte';
  import StreamingIndicator from './StreamingIndicator.svelte';
  import ConversationList from './ConversationList.svelte';
  import ChatEmptyState from './ChatEmptyState.svelte';
  import QuickActionChips from './QuickActionChips.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import { BarsIcon, PlusIcon } from '$lib/components/icons/md';
  import { soundManager } from '$lib/utils/sound';

  interface Props {
    initialConversationId?: string;
    prefill?: string;
  }
  let { initialConversationId, prefill }: Props = $props();

  // Conversation state
  let currentConversationId: string | null = $state(initialConversationId ?? null);
  let messages: Message[] = $state([]);
  let conversations: ConversationSummary[] = $state([]);
  let suggestions: PromptSuggestion[] = $state([]);

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
  let showConversationList = $state(false);
  let messageContainer: HTMLElement | undefined = $state(undefined);

  // Derived
  let hasMessages = $derived(messages.length > 0);
  let conversationTitle = $derived(
    conversations.find(c => c.id === currentConversationId)?.title ?? $t('chat.new_conversation_title')
  );
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
      scrollToBottom();
    } catch (e) {
      console.error('Failed to load messages:', e);
    }
  }

  async function loadConversations() {
    try {
      conversations = await listConversations();
    } catch (e) {
      console.error('Failed to load conversations:', e);
    }
  }

  async function handleNewConversation() {
    // UA02-05: Lazy creation — don't persist to DB until first message is sent.
    // If current conversation is already empty, just stay on it.
    if (currentConversationId && messages.length === 0) {
      showConversationList = false;
      return;
    }
    currentConversationId = null;
    messages = [];
    streamingText = '';
    pendingCitations = [];
    responseConfidence = null;
    streamError = null;
    showConversationList = false;
    suggestions = await getPromptSuggestions().catch(() => []);
  }

  async function handleSelectConversation(convId: string) {
    if (isStreaming) return;
    currentConversationId = convId;
    streamingText = '';
    pendingCitations = [];
    responseConfidence = null;
    streamError = null;
    showConversationList = false;
    await loadMessages(convId);
  }

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
      await loadConversations();
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
    await loadConversations();
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
  <!-- Header -->
  <header class="flex items-center gap-3 px-4 py-3 bg-white dark:bg-gray-900 border-b border-stone-200 dark:border-gray-700">
    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center text-stone-500 dark:text-gray-400
             hover:text-stone-600 dark:hover:text-gray-300"
      onclick={() => showConversationList = !showConversationList}
      aria-label={$t('chat.toggle_conversations')}
    >
      <BarsIcon class="w-5 h-5" />
    </button>

    <h1 class="flex-1 text-base font-medium text-stone-800 dark:text-gray-100 truncate">
      {conversationTitle}
    </h1>

    <button
      class="min-h-[44px] min-w-[44px] flex items-center justify-center text-stone-500 dark:text-gray-400
             hover:text-[var(--color-interactive)]"
      onclick={handleNewConversation}
      aria-label={$t('chat.new_conversation')}
    >
      <PlusIcon class="w-6 h-6" />
    </button>
  </header>

  <!-- Conversation list drawer -->
  {#if showConversationList}
    <div class="absolute inset-0 z-40 flex">
      <button
        class="absolute inset-0 bg-black/20"
        onclick={() => showConversationList = false}
        aria-label={$t('chat.close_conversations')}
      ></button>
      <div class="relative z-50 w-[280px] bg-white dark:bg-gray-900 h-full shadow-xl overflow-y-auto">
        <ConversationList
          {conversations}
          activeConversationId={currentConversationId}
          onSelect={handleSelectConversation}
          onNewConversation={handleNewConversation}
          onDelete={async (id) => {
            await deleteConversation(id);
            if (currentConversationId === id) {
              currentConversationId = null;
              messages = [];
            }
            await loadConversations();
          }}
        />
      </div>
    </div>
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
  <div class="bg-white dark:bg-gray-900 border-t border-stone-200 dark:border-gray-700 px-4 py-3">
    <div class="flex items-end gap-2 max-w-2xl mx-auto">
      <textarea
        class="flex-1 px-4 py-3 rounded-xl border border-stone-200 dark:border-gray-700 text-base
               bg-white dark:bg-gray-800 text-stone-800 dark:text-gray-100
               resize-none min-h-[44px] max-h-[120px]
               focus:border-[var(--color-interactive)] focus:outline-none
               placeholder:text-stone-500 dark:placeholder:text-gray-500"
        placeholder={$t('chat.input_placeholder')}
        bind:value={inputText}
        onkeydown={handleKeydown}
        rows={1}
        disabled={isStreaming}
        aria-label={$t('chat.input_aria')}
      ></textarea>
      <button
        class="min-h-[44px] min-w-[44px] px-4 py-3 rounded-xl font-medium text-base
               transition-colors
               {canSend
                 ? 'bg-[var(--color-interactive)] text-white hover:bg-[var(--color-interactive-hover)]'
                 : 'bg-stone-100 dark:bg-gray-800 text-stone-500 dark:text-gray-400 cursor-not-allowed'}"
        onclick={handleSend}
        disabled={!canSend}
        aria-label={$t('chat.send_aria')}
      >
        {$t('common.send')}
      </button>
    </div>
  </div>
</div>
