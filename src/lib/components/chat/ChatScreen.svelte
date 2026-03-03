<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { browser } from '$app/environment';
  import { t } from 'svelte-i18n';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {
    startConversation,
    sendChatMessage,
    getConversationMessages,
    getPromptSuggestions,
    getChatQueueForConversation,
  } from '$lib/api/chat';
  import type {
    Message,
    ChatStreamEvent,
    ChatQueueEvent,
    ChatQueueState,
    CitationView,
    GuidelineCitationView,
    PromptSuggestion,
  } from '$lib/types/chat';
  import { ai } from '$lib/stores/ai.svelte';
  import { chatQueue } from '$lib/stores/chatQueue.svelte';
  import { navigation } from '$lib/stores/navigation.svelte';
  import MessageBubble from './MessageBubble.svelte';
  import MessageQueueStatus from './MessageQueueStatus.svelte';
  import StreamingIndicator from './StreamingIndicator.svelte';
  import ChatEmptyState from './ChatEmptyState.svelte';
  import QuickActionChips from './QuickActionChips.svelte';
  import ErrorBanner from '$lib/components/ErrorBanner.svelte';
  import { ArrowUpIcon, PlusIcon } from '$lib/components/icons/md';
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

  // Streaming state
  let isStreaming = $state(false);
  let streamingText = $state('');
  let pendingCitations: CitationView[] = $state([]);
  let pendingGuidelineCitations: GuidelineCitationView[] = $state([]);
  let responseConfidence: number | null = $state(null);
  let streamError: string | null = $state(null);
  let lastSentText: string | null = $state(null);

  // CHAT-QUEUE-01: Queue tracking — maps queue_item_id → patient_message_id
  let pendingMessages = $state<Map<string, string>>(new Map());

  // FIX-1: Per-message queue state for MessageQueueStatus rendering (Signal pattern)
  let queueStates = $state<Map<string, { state: ChatQueueState; position: number }>>(new Map());

  /** Reverse-lookup: find queue state for a patient message ID. */
  function getQueueStateForMessage(messageId: string): { state: ChatQueueState; position: number } | null {
    for (const [queueId, localMsgId] of pendingMessages) {
      if (localMsgId === messageId) {
        return queueStates.get(queueId) ?? null;
      }
    }
    return null;
  }

  // Persistent listener handles (CHAT-QUEUE-01: replaced per-send pattern)
  let unlistenQueue: UnlistenFn | null = null;
  let unlistenStream: UnlistenFn | null = null;

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
  // CHAT-QUEUE-01: Input re-enables immediately — only blocked by empty text
  let canSend = $derived(inputText.trim().length > 0);

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
    pendingMessages = new Map();
    queueStates = new Map();
    conversationTitle = $t('chat.new_conversation_title');
    // CHAT-NAV-01: Clear persisted conversation so next mount shows empty
    navigation.setLastChat(null);
    // CHAT-NAV-01 FIX-2: Clear stale URL hash so page refresh shows empty state.
    // Uses replaceState directly (not navigate) to avoid re-triggering the $effect.
    if (browser) {
      history.replaceState(null, '', '#chat');
    }
    suggestions = await getPromptSuggestions().catch(() => []);
  }

  // React to navigation PROP changes only (History → Chat, or sidebar "Ask" click).
  // Uses untrack() for internal state reads so handleSend() setting
  // currentConversationId doesn't re-trigger this effect and wipe messages.
  $effect(() => {
    const convId = initialConversationId;
    const pre = prefill;

    // Don't interrupt active streaming (read without subscribing)
    if (untrack(() => isStreaming)) return;

    if (convId && convId !== untrack(() => currentConversationId)) {
      // Navigating to a specific conversation from History
      currentConversationId = convId;
      streamingText = '';
      pendingCitations = [];
      responseConfidence = null;
      streamError = null;
      pendingMessages = new Map();
      queueStates = new Map();
      // Update active conversation for chatQueue store sound routing
      chatQueue.activeConversationId = convId;
      // CHAT-NAV-01: Persist for navigation return
      navigation.setLastChat(convId);
      loadMessages(convId);
    } else if (!convId && untrack(() => currentConversationId)) {
      // Sidebar "Ask" click — start new conversation
      handleNewConversation();
    }

    // Handle prefill from other screens
    if (pre) {
      inputText = pre;
    }
  });

  // CHAT-QUEUE-01: Non-blocking send — enqueues and returns immediately
  async function handleSend() {
    if (!canSend) return;

    const text = inputText.trim();
    inputText = '';
    lastSentText = text;

    // Create conversation if needed (fast — DB write only)
    if (!currentConversationId) {
      try {
        const id = await startConversation();
        currentConversationId = id;
        chatQueue.activeConversationId = id;
        // CHAT-NAV-01: Persist for navigation return
        navigation.setLastChat(id);
        // CHAT-NAV-01 FIX-1: Update URL hash directly (not via navigate) so that
        // screenParams stays clean. Using navigate() would set screenParams.conversationId,
        // and a subsequent sidebar "Ask" click would clear it — triggering the $effect
        // to call handleNewConversation() and wipe the active conversation.
        // Direct replaceState updates URL for back/forward without touching screenParams.
        if (browser) {
          history.replaceState(null, '', `#chat?conversationId=${id}`);
        }
      } catch (e) {
        streamError = $t('chat.conversation_start_error');
        return;
      }
    }

    const sendConversationId = currentConversationId!;

    // Update title from first message
    if (messages.length === 0) {
      conversationTitle = text.length > 50 ? text.slice(0, 50) + '...' : text;
    }

    // Optimistic: add patient message to local state immediately
    const patientMessage: Message = {
      id: crypto.randomUUID(),
      conversation_id: sendConversationId,
      role: 'patient',
      content: text,
      timestamp: new Date().toISOString(),
      source_chunks: null,
      confidence: null,
      feedback: null,
    };
    messages = [...messages, patientMessage];
    scrollToBottom();

    // Enqueue — returns immediately with queue_item_id
    try {
      const queueItemId = await sendChatMessage(sendConversationId, text);
      // Track this message as in-flight
      pendingMessages = new Map(pendingMessages).set(queueItemId, patientMessage.id);
      // FIX-1: Set initial Queued state for MessageQueueStatus rendering
      queueStates = new Map(queueStates).set(queueItemId, { state: 'Queued' as ChatQueueState, position: 1 });
    } catch (e) {
      streamError = e instanceof Error ? e.message : String(e);
      ai.handleOperationFailure(e);
      soundManager.play('error');
    }
    // Input is already re-enabled (no isSending flag to unset)
  }

  function handleRetry() {
    if (!lastSentText) return;
    streamError = null;
    inputText = lastSentText;
    handleSend();
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

  // CHAT-QUEUE-01: Persistent listeners — set up on mount, torn down on destroy
  onMount(async () => {
    suggestions = await getPromptSuggestions();

    // CHAT-NAV-01: Determine which conversation to load.
    // Priority: explicit prop (History click) > last active (sidebar return) > none (new)
    const resumeId = initialConversationId ?? navigation.lastChatConversationId;

    if (resumeId) {
      currentConversationId = resumeId;
      await loadMessages(resumeId);

      // CHAT-NAV-01 FIX-3: Detect deleted conversation — if loadMessages returns empty
      // the conversation was deleted from History. Clear stale state so the user sees
      // a clean empty state and handleSend() creates a fresh conversation on next send.
      if (messages.length === 0) {
        currentConversationId = null;
        navigation.setLastChat(null);
      } else {
        // Load any pending queue items for this conversation
        try {
          const pending = await getChatQueueForConversation(resumeId);
          for (const item of pending) {
            pendingMessages = new Map(pendingMessages).set(item.id, item.patient_message_id);
            // Restore queue state for MessageQueueStatus rendering
            if (item.state === 'Queued' || item.state === 'Acquiring') {
              queueStates = new Map(queueStates).set(item.id, { state: item.state, position: item.queue_position });
            }
            // If an item is already streaming, resume streaming display
            if (item.state === 'Streaming') {
              isStreaming = true;
              streamingText = '';
              pendingCitations = [];
            }
          }
        } catch {
          // Queue may not be available yet
        }
      }
    }

    // Register active conversation with chatQueue store (for sound routing)
    chatQueue.activeConversationId = currentConversationId;
    // CHAT-NAV-01: Sync lastChat (covers resume-from-History and fresh load)
    navigation.setLastChat(currentConversationId);

    // Persistent listener: chat-queue-update events
    unlistenQueue = await listen<ChatQueueEvent>('chat-queue-update', (event) => {
      if (event.payload.conversation_id !== currentConversationId) return;

      const { queue_item_id, state, queue_position, error } = event.payload;

      switch (state) {
        case 'Queued':
        case 'Acquiring': {
          // FIX-1: Track queue state for MessageQueueStatus rendering
          const nextQS = new Map(queueStates);
          nextQS.set(queue_item_id, { state, position: queue_position });
          queueStates = nextQS;
          break;
        }
        case 'Streaming': {
          // FIX-1: Clear queue indicator — StreamingIndicator takes over
          const nextQS = new Map(queueStates);
          nextQS.delete(queue_item_id);
          queueStates = nextQS;
          isStreaming = true;
          streamingText = '';
          pendingCitations = [];
          streamError = null;
          break;
        }
        case 'Complete': {
          const nextQS = new Map(queueStates);
          nextQS.delete(queue_item_id);
          queueStates = nextQS;
          const nextComplete = new Map(pendingMessages);
          nextComplete.delete(queue_item_id);
          pendingMessages = nextComplete;
          break;
        }
        case 'Failed': {
          const nextQS = new Map(queueStates);
          nextQS.delete(queue_item_id);
          queueStates = nextQS;
          streamError = error ?? $t('chat.stream_error_guidance');
          isStreaming = false;
          ai.handleOperationFailure(new Error(error ?? 'Chat queue item failed'));
          soundManager.play('error');
          const nextFailed = new Map(pendingMessages);
          nextFailed.delete(queue_item_id);
          pendingMessages = nextFailed;
          break;
        }
      }
    });

    // Persistent listener: chat-stream events (tokens + citations + Done)
    unlistenStream = await listen<ChatStreamEvent>('chat-stream', (event) => {
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
        case 'GuidelineCitations':
          pendingGuidelineCitations = [...pendingGuidelineCitations, ...chunk.citations];
          break;
        case 'Done': {
          // Commit AI message to local state
          const cohearaMessage: Message = {
            id: crypto.randomUUID(),
            conversation_id: conversation_id,
            role: 'coheara',
            content: chunk.full_text,
            timestamp: new Date().toISOString(),
            source_chunks: pendingCitations.length > 0
              ? JSON.stringify(pendingCitations) : null,
            confidence: chunk.confidence,
            feedback: null,
          };
          messages = [...messages, cohearaMessage];
          streamingText = '';
          pendingCitations = [];
          pendingGuidelineCitations = [];
          isStreaming = false;
          responseConfidence = chunk.confidence;
          soundManager.play('completion');
          scrollToBottom();
          break;
        }
        case 'Error':
          streamError = chunk.message;
          isStreaming = false;
          ai.handleOperationFailure(new Error(chunk.message));
          soundManager.play('error');
          break;
      }
    });

    // Spec 48 [CA-05]: Pre-fill input from post-review CTA or other navigation
    if (prefill) {
      inputText = prefill;
    }
  });

  onDestroy(() => {
    unlistenQueue?.();
    unlistenStream?.();
    chatQueue.activeConversationId = null;
  });
</script>

<div class="flex flex-col h-full bg-stone-50 dark:bg-gray-950">
  <!-- Header — title + "New Session" button (Claude.ai/ChatGPT pattern) -->
  {#if hasMessages}
    <header class="flex items-center gap-2 px-4 py-3 bg-stone-50 dark:bg-gray-950">
      <h1 class="flex-1 text-base font-medium text-stone-800 dark:text-gray-100 truncate">
        {conversationTitle}
      </h1>
      <button
        class="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm
               text-stone-500 dark:text-gray-400
               hover:bg-stone-100 dark:hover:bg-gray-800
               transition-colors min-h-[36px] flex-shrink-0"
        onclick={handleNewConversation}
        aria-label={$t('chat.new_conversation')}
      >
        <PlusIcon class="w-4 h-4" />
        <span class="hidden sm:inline">{$t('chat.new_conversation')}</span>
      </button>
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
          {#if message.role === 'patient'}
            {@const qs = getQueueStateForMessage(message.id)}
            {#if qs}
              <MessageQueueStatus queueState={qs.state} queuePosition={qs.position} />
            {/if}
          {/if}
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
              actionLabel={lastSentText ? $t('common.retry') : undefined}
              onAction={lastSentText ? handleRetry : undefined}
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
