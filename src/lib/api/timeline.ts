/** L4-04: Timeline View API — Tauri IPC wrapper. */

import { invoke } from '@tauri-apps/api/core';
import type { TimelineData, TimelineFilter } from '$lib/types/timeline';

export async function getTimelineData(filter: TimelineFilter): Promise<TimelineData> {
  return invoke<TimelineData>('get_timeline_data', { filter });
}
