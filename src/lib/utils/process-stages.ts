/**
 * Spec 50 [NF-03]: Process stage configurations for ProcessIndicator.
 * Each variant defines time-based stages with i18n message keys and progress hints.
 */

import type { ProcessStage } from '$lib/components/ui/ProcessIndicator.svelte';

export const CHAT_STAGES: ProcessStage[] = [
  { messageKey: 'process.chat_searching', percentHint: 15, durationHint: 2 },
  { messageKey: 'process.chat_found', percentHint: 40, durationHint: 3 },
  { messageKey: 'process.chat_generating', percentHint: 65, durationHint: 5 },
  { messageKey: 'process.chat_safety', percentHint: 85, durationHint: 10 },
  { messageKey: 'process.chat_complex', percentHint: 92, durationHint: 999 },
];

export const IMPORT_STAGES: ProcessStage[] = [
  { messageKey: 'process.import_reading', percentHint: 25, durationHint: 3 },
  { messageKey: 'process.import_analyzing', percentHint: 55, durationHint: 5 },
  { messageKey: 'process.import_quality', percentHint: 80, durationHint: 4 },
  { messageKey: 'process.import_ready', percentHint: 100, durationHint: 999 },
];

export const APPOINTMENT_STAGES: ProcessStage[] = [
  { messageKey: 'process.prep_gathering', percentHint: 30, durationHint: 3 },
  { messageKey: 'process.prep_building', percentHint: 70, durationHint: 7 },
  { messageKey: 'process.prep_ready', percentHint: 100, durationHint: 999 },
];
