// CHAT-QUEUE-01: Chat queue reactive store — persists across navigation.
//
// App-level singleton. Listens for `chat-queue-update` Tauri events.
// Tracks pending messages, plays notification sounds when answers arrive
// off-screen, and exposes badge count for the navigation indicator.
//
// Pattern: Mirrors ImportQueueStore (BTL-10 C5).

import { getChatQueue } from '$lib/api/chat';
import type { ChatQueueItem, ChatQueueEvent, ChatStreamEvent } from '$lib/types/chat';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauriEnv } from '$lib/utils/tauri';
import { soundManager } from '$lib/utils/sound';

class ChatQueueStore {
  items = $state<ChatQueueItem[]>([]);
  isProcessing = $state(false);

  private _unlistenQueue: UnlistenFn | null = null;
  private _unlistenStream: UnlistenFn | null = null;

  /**
   * The conversation ID currently visible on ChatScreen.
   * Set by ChatScreen on mount, cleared on destroy.
   * Used to decide whether to play notification sounds.
   */
  activeConversationId = $state<string | null>(null);

  // -- Derived state --

  get pendingCount(): number {
    return this.items.filter(
      (i) => !['Complete', 'Failed'].includes(i.state),
    ).length;
  }

  get hasActiveItems(): boolean {
    return this.pendingCount > 0;
  }

  pendingForConversation(conversationId: string): ChatQueueItem[] {
    return this.items.filter(
      (i) => i.conversation_id === conversationId && !['Complete', 'Failed'].includes(i.state),
    );
  }

  // -- Event subscription --

  /** Start listening to chat queue events. Call once at app startup. */
  async startListening(): Promise<void> {
    if (!isTauriEnv() || this._unlistenQueue) return;

    this._unlistenQueue = await listen<ChatQueueEvent>(
      'chat-queue-update',
      (event) => {
        this.handleQueueEvent(event.payload);
      },
    );

    // Listen for chat-stream Done events for cross-screen notification sounds
    this._unlistenStream = await listen<ChatStreamEvent>(
      'chat-stream',
      (event) => {
        this.handleStreamEvent(event.payload);
      },
    );
  }

  /** Stop listening. Call on cleanup. */
  stopListening(): void {
    this._unlistenQueue?.();
    this._unlistenQueue = null;
    this._unlistenStream?.();
    this._unlistenStream = null;
  }

  /** F7: Clear all state on lock/switch to prevent cross-profile data leakage. */
  reset(): void {
    this.items = [];
    this.isProcessing = false;
    this.activeConversationId = null;
  }

  // -- Event handling --

  private handleQueueEvent(event: ChatQueueEvent): void {
    const idx = this.items.findIndex((i) => i.id === event.queue_item_id);

    if (idx >= 0) {
      // Update existing item
      const updated = { ...this.items[idx] };
      updated.state = event.state;
      updated.queue_position = event.queue_position;
      if (event.error) updated.error = event.error;
      this.items = [
        ...this.items.slice(0, idx),
        updated,
        ...this.items.slice(idx + 1),
      ];
    } else {
      // Item not found locally — full refresh
      this.refresh().catch(() => {});
    }

    // Play error sound on failure (regardless of screen)
    if (event.state === 'Failed') {
      soundManager.play('error');
    }
  }

  private handleStreamEvent(event: ChatStreamEvent): void {
    // Only care about Done events for notification sounds
    if (event.chunk.type !== 'Done') return;

    // If user is NOT viewing this conversation's ChatScreen, play notification
    if (event.conversation_id !== this.activeConversationId) {
      soundManager.play('alert-info');
    }
  }

  // -- API --

  /** Full refresh from backend. */
  async refresh(): Promise<void> {
    try {
      const snapshot = await getChatQueue();
      this.items = snapshot.items;
      this.isProcessing = snapshot.is_processing;
    } catch {
      // Silently ignore — queue may not be available yet at startup
    }
  }
}

export const chatQueue = new ChatQueueStore();
