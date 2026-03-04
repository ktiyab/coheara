export interface Message {
  id: string;
  conversation_id: string;
  role: 'patient' | 'coheara';
  content: string;
  timestamp: string;
  source_chunks: string | null;
  confidence: number | null;
  feedback: 'Helpful' | 'NotHelpful' | null;
}

export interface ConversationSummary {
  id: string;
  title: string;
  last_message_at: string;
  message_count: number;
  last_message_preview: string;
}

export interface CitationView {
  document_id: string;
  document_title: string;
  document_date: string | null;
  professional_name: string | null;
  chunk_text: string;
  relevance_score: number;
}

/** ME-03: Guideline citation from clinical insights (deterministic, not LLM-generated). */
export interface GuidelineCitationView {
  source: string;
  insight_count: number;
}

export type StreamChunkPayload =
  | { type: 'Token'; text: string }
  | { type: 'Citation'; citation: CitationView }
  | { type: 'GuidelineCitations'; citations: GuidelineCitationView[] }
  | { type: 'Done'; full_text: string; confidence: number; boundary_check: string; grounding: string }
  | { type: 'Error'; message: string };

export interface ChatStreamEvent {
  conversation_id: string;
  chunk: StreamChunkPayload;
}

export interface PromptSuggestion {
  template_key: string;
  params: Record<string, string>;
  category: string;
  intent: 'query' | 'expression';
}

// CHAT-QUEUE-01: Deferred chat queue types

export type ChatQueueState = 'Queued' | 'Acquiring' | 'Streaming' | 'Complete' | 'Failed';

export interface ChatQueueItem {
  id: string;
  conversation_id: string;
  patient_message_id: string;
  text: string;
  state: ChatQueueState;
  queue_position: number;
  error: string | null;
  queued_at: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface ChatQueueEvent {
  queue_item_id: string;
  conversation_id: string;
  patient_message_id: string;
  state: ChatQueueState;
  queue_position: number;
  error: string | null;
}

export interface ChatQueueSnapshot {
  items: ChatQueueItem[];
  is_processing: boolean;
}
