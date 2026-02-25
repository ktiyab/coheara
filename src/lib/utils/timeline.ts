/** L4-04: Timeline View utility functions — coordinate system, zoom, colors. */

import type { EventType, ZoomLevel } from '$lib/types/timeline';

// ── Constants ──────────────────────────────────────────────────────────────

/** Pixels per time unit at each zoom level */
export const SCALE: Record<ZoomLevel, number> = {
  Day: 120,
  Week: 60,
  Month: 20,
  Year: 4,
};

export const LANE_HEIGHT = 48;
export const LANE_GAP = 8;
export const HEADER_HEIGHT = 40;
export const MARKER_RADIUS = 8;
export const TOUCH_TARGET_RADIUS = 22;
export const PADDING_X = 80;
export const PADDING_Y = 16;

/** Event type → lane index mapping (top to bottom) */
const LANE_ORDER: Record<string, number> = {
  insight: 0,
  appointment: 1,
  medication: 2,
  diagnosis: 3,
  lab: 4,
  symptom: 5,
  vital: 6,
  procedure: 7,
  document: 8,
};

const LANE_COUNT = Object.keys(LANE_ORDER).length;

/** Total SVG height (all lanes + header + padding) */
export const CANVAS_HEIGHT =
  HEADER_HEIGHT + PADDING_Y * 2 + LANE_COUNT * (LANE_HEIGHT + LANE_GAP);

/** Color palette for event types — soft pastels per design language */
export const EVENT_COLORS: Record<string, { fill: string; stroke: string; label: string }> = {
  insight:     { fill: '#FFFBEB', stroke: '#D97706', label: 'Insights' },
  medication:  { fill: '#DBEAFE', stroke: '#3B82F6', label: 'Medications' },
  lab:         { fill: '#DCFCE7', stroke: '#22C55E', label: 'Lab Results' },
  symptom:     { fill: '#FFF7ED', stroke: '#F97316', label: 'Symptoms' },
  vital:       { fill: '#EEF2FF', stroke: '#6366F1', label: 'Vitals' },
  procedure:   { fill: '#F3E8FF', stroke: '#A855F7', label: 'Procedures' },
  appointment: { fill: '#CCFBF1', stroke: '#14B8A6', label: 'Appointments' },
  document:    { fill: '#F5F5F4', stroke: '#A8A29E', label: 'Documents' },
  diagnosis:   { fill: '#FCE7F3', stroke: '#EC4899', label: 'Diagnoses' },
};

/** Lane labels in display order */
export const LANE_LABELS = [
  'Insights',
  'Appointments',
  'Medications',
  'Diagnoses',
  'Lab Results',
  'Symptoms',
  'Vitals',
  'Procedures',
  'Documents',
];

// ── Functions ──────────────────────────────────────────────────────────────

/** Maps EventType to color group key */
export function eventColorGroup(eventType: EventType): string {
  switch (eventType) {
    case 'MedicationStart':
    case 'MedicationStop':
    case 'MedicationDoseChange':
      return 'medication';
    case 'LabResult': return 'lab';
    case 'Symptom': return 'symptom';
    case 'Procedure': return 'procedure';
    case 'Appointment': return 'appointment';
    case 'Document': return 'document';
    case 'Diagnosis': return 'diagnosis';
    case 'CoherenceAlert': return 'insight';
    case 'VitalSign': return 'vital';
  }
}

/** Calculate total SVG width from date range and zoom */
export function calculateCanvasWidth(
  earliest: Date,
  latest: Date,
  zoom: ZoomLevel,
): number {
  const days = Math.max(1, Math.ceil((latest.getTime() - earliest.getTime()) / (1000 * 60 * 60 * 24)));
  switch (zoom) {
    case 'Day':   return days * SCALE.Day + PADDING_X * 2;
    case 'Week':  return Math.ceil(days / 7) * SCALE.Week + PADDING_X * 2;
    case 'Month': return Math.ceil(days / 30) * SCALE.Month + PADDING_X * 2;
    case 'Year':  return Math.ceil(days / 365) * SCALE.Year + PADDING_X * 2;
  }
}

/** Convert a date to X position on the SVG canvas */
export function dateToX(
  date: Date,
  earliest: Date,
  zoom: ZoomLevel,
): number {
  const days = (date.getTime() - earliest.getTime()) / (1000 * 60 * 60 * 24);
  switch (zoom) {
    case 'Day':   return PADDING_X + days * SCALE.Day;
    case 'Week':  return PADDING_X + (days / 7) * SCALE.Week;
    case 'Month': return PADDING_X + (days / 30) * SCALE.Month;
    case 'Year':  return PADDING_X + (days / 365) * SCALE.Year;
  }
}

/** Calculate Y position for an event */
export function eventToY(eventType: EventType): number {
  const group = eventColorGroup(eventType);
  const laneIndex = LANE_ORDER[group] ?? 6;
  return HEADER_HEIGHT + PADDING_Y + laneIndex * (LANE_HEIGHT + LANE_GAP) + LANE_HEIGHT / 2;
}

/** Select the best initial zoom level based on total date range */
export function autoSelectZoom(earliest: Date, latest: Date): ZoomLevel {
  const days = Math.ceil((latest.getTime() - earliest.getTime()) / (1000 * 60 * 60 * 24));
  if (days <= 30) return 'Day';
  if (days <= 180) return 'Week';
  if (days <= 730) return 'Month';
  return 'Year';
}

/** Generate date axis tick marks for a zoom level */
export function generateTicks(
  earliest: Date,
  latest: Date,
  zoom: ZoomLevel,
): Array<{ date: Date; label: string; x: number }> {
  const ticks: Array<{ date: Date; label: string; x: number }> = [];
  const current = new Date(earliest);

  switch (zoom) {
    case 'Day':
      while (current <= latest) {
        ticks.push({
          date: new Date(current),
          label: current.toLocaleDateString('en-US', { weekday: 'short', day: 'numeric' }),
          x: dateToX(current, earliest, zoom),
        });
        current.setDate(current.getDate() + 1);
      }
      break;

    case 'Week':
      current.setDate(current.getDate() - current.getDay() + 1);
      while (current <= latest) {
        ticks.push({
          date: new Date(current),
          label: current.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }),
          x: dateToX(current, earliest, zoom),
        });
        current.setDate(current.getDate() + 7);
      }
      break;

    case 'Month':
      current.setDate(1);
      while (current <= latest) {
        ticks.push({
          date: new Date(current),
          label: current.toLocaleDateString('en-US', { month: 'short' }),
          x: dateToX(current, earliest, zoom),
        });
        current.setMonth(current.getMonth() + 1);
      }
      break;

    case 'Year':
      current.setMonth(0, 1);
      while (current <= latest) {
        ticks.push({
          date: new Date(current),
          label: current.getFullYear().toString(),
          x: dateToX(current, earliest, zoom),
        });
        current.setFullYear(current.getFullYear() + 1);
      }
      break;
  }

  return ticks;
}

/** Generate SVG path for a correlation line between two events */
export function correlationPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
): string {
  const midX = (sourceX + targetX) / 2;
  const controlY = Math.min(sourceY, targetY) - 30;
  return `M ${sourceX} ${sourceY} Q ${midX} ${controlY} ${targetX} ${targetY}`;
}
