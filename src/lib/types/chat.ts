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

export type StreamChunkPayload =
  | { type: 'Token'; text: string }
  | { type: 'Citation'; citation: CitationView }
  | { type: 'Done'; full_text: string; confidence: number; boundary_check: string }
  | { type: 'Error'; message: string };

export interface ChatStreamEvent {
  conversation_id: string;
  chunk: StreamChunkPayload;
}

export interface PromptSuggestion {
  text: string;
  category: string;
}
