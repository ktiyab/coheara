import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import { tick } from 'svelte';
import TaperingSchedule from './TaperingSchedule.svelte';
import type { TaperingStepView } from '$lib/types/medication';

function makeSteps(): TaperingStepView[] {
  return [
    { step_number: 1, dose: '20mg', duration_days: 14, start_date: '2026-01-01', instructions: null, is_current: false },
    { step_number: 2, dose: '15mg', duration_days: 14, start_date: '2026-01-15', instructions: null, is_current: true },
    { step_number: 3, dose: '10mg', duration_days: 14, start_date: '2026-01-29', instructions: null, is_current: false },
  ];
}

describe('TaperingSchedule', () => {
  it('renders all tapering steps', async () => {
    render(TaperingSchedule, { props: { steps: makeSteps() } });
    await tick();
    expect(screen.getByText('20mg')).toBeInTheDocument();
    expect(screen.getByText('15mg')).toBeInTheDocument();
    expect(screen.getByText('10mg')).toBeInTheDocument();
  });

  it('highlights current step with aria-current', async () => {
    const { container } = render(TaperingSchedule, { props: { steps: makeSteps() } });
    await tick();
    const current = container.querySelector('[aria-current="step"]');
    expect(current).toBeTruthy();
    expect(current?.textContent).toContain('15mg');
  });

  it('applies visual highlight to current step', async () => {
    const { container } = render(TaperingSchedule, { props: { steps: makeSteps() } });
    await tick();
    const current = container.querySelector('[aria-current="step"]');
    expect(current?.className).toContain('bg-[var(--color-info-50)]');
  });
});
