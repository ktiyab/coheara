import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import { tick } from 'svelte';
import ConfidenceSummary from './ConfidenceSummary.svelte';

describe('ConfidenceSummary', () => {
  it('renders overall confidence percentage', async () => {
    render(ConfidenceSummary, {
      props: { totalFields: 10, confidentFields: 8, flaggedFields: 2, overallConfidence: 0.85 },
    });
    await tick();
    expect(screen.getByText(/85%/)).toBeInTheDocument();
  });

  it('shows success bar when no fields are flagged', async () => {
    const { container } = render(ConfidenceSummary, {
      props: { totalFields: 5, confidentFields: 5, flaggedFields: 0, overallConfidence: 1.0 },
    });
    await tick();
    const bar = container.querySelector('.bg-\\[var\\(--color-success\\)\\]');
    expect(bar).toBeTruthy();
  });

  it('shows warning bar when 1-2 fields are flagged', async () => {
    const { container } = render(ConfidenceSummary, {
      props: { totalFields: 8, confidentFields: 6, flaggedFields: 2, overallConfidence: 0.75 },
    });
    await tick();
    const bar = container.querySelector('.bg-\\[var\\(--color-warning\\)\\]');
    expect(bar).toBeTruthy();
  });

  it('shows danger bar when more than 2 fields are flagged', async () => {
    const { container } = render(ConfidenceSummary, {
      props: { totalFields: 10, confidentFields: 5, flaggedFields: 5, overallConfidence: 0.5 },
    });
    await tick();
    const bar = container.querySelector('.bg-\\[var\\(--color-danger\\)\\]');
    expect(bar).toBeTruthy();
  });

  it('sets correct fill width based on confident/total ratio', async () => {
    const { container } = render(ConfidenceSummary, {
      props: { totalFields: 10, confidentFields: 7, flaggedFields: 3, overallConfidence: 0.7 },
    });
    await tick();
    const bar = container.querySelector('[style*="width"]');
    expect(bar?.getAttribute('style')).toContain('70%');
  });
});
