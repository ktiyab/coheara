import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import ConfidenceIndicator from './ConfidenceIndicator.svelte';

describe('ConfidenceIndicator', () => {
  it('renders with role="status" for accessibility', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.9 } });
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('shows well-supported label for high confidence (>=0.8)', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.9 } });
    expect(screen.getByText('Well supported')).toBeInTheDocument();
  });

  it('shows partially-supported label for medium confidence (0.5-0.79)', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.6 } });
    expect(screen.getByText('Partially supported')).toBeInTheDocument();
  });

  it('shows limited-info label for low confidence (<0.5)', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.3 } });
    expect(screen.getByText('Limited information')).toBeInTheDocument();
  });

  it('applies success color class for high confidence', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.85 } });
    const container = screen.getByRole('status');
    expect(container.className).toContain('text-[var(--color-success)]');
  });

  it('applies warning color class for medium confidence', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.55 } });
    const container = screen.getByRole('status');
    expect(container.className).toContain('text-[var(--color-warning)]');
  });

  it('applies stone color class for low confidence', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.2 } });
    const container = screen.getByRole('status');
    expect(container.className).toContain('text-stone-500');
  });

  it('has aria-label for screen readers', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.9 } });
    const container = screen.getByRole('status');
    expect(container.getAttribute('aria-label')).toContain('Well supported');
  });

  it('boundary: 0.8 exactly is well-supported', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.8 } });
    expect(screen.getByText('Well supported')).toBeInTheDocument();
  });

  it('boundary: 0.5 exactly is partially-supported', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.5 } });
    expect(screen.getByText('Partially supported')).toBeInTheDocument();
  });

  it('boundary: 0.49 is limited-info', () => {
    render(ConfidenceIndicator, { props: { confidence: 0.49 } });
    expect(screen.getByText('Limited information')).toBeInTheDocument();
  });
});
