import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import ConfidenceFlag from './ConfidenceFlag.svelte';

describe('ConfidenceFlag', () => {
  it('renders with role="alert" for accessibility', () => {
    render(ConfidenceFlag, { props: { confidence: 0.4, fieldLabel: 'Medication' } });
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('shows very-low confidence message when confidence < 0.50', () => {
    render(ConfidenceFlag, { props: { confidence: 0.3, fieldLabel: 'Dose' } });
    expect(screen.getByText(/very low-quality text/)).toBeInTheDocument();
  });

  it('shows low confidence message when confidence >= 0.50', () => {
    render(ConfidenceFlag, { props: { confidence: 0.6, fieldLabel: 'Dose' } });
    expect(screen.getByText(/low-quality image/)).toBeInTheDocument();
  });

  it('applies danger styling for very low confidence', () => {
    render(ConfidenceFlag, { props: { confidence: 0.3, fieldLabel: 'Test' } });
    const alert = screen.getByRole('alert');
    expect(alert.className).toContain('bg-[var(--color-danger-50)]');
    expect(alert.className).toContain('border-[var(--color-danger-200)]');
  });

  it('applies warning styling for low confidence', () => {
    render(ConfidenceFlag, { props: { confidence: 0.6, fieldLabel: 'Test' } });
    const alert = screen.getByRole('alert');
    expect(alert.className).toContain('bg-[var(--color-warning-50)]');
    expect(alert.className).toContain('border-[var(--color-warning-200)]');
  });

  it('displays confidence percentage', () => {
    render(ConfidenceFlag, { props: { confidence: 0.42, fieldLabel: 'Test' } });
    expect(screen.getByText(/42%/)).toBeInTheDocument();
  });

  it('displays check label text', () => {
    render(ConfidenceFlag, { props: { confidence: 0.5, fieldLabel: 'Test' } });
    expect(screen.getByText(/not sure I read this correctly/)).toBeInTheDocument();
  });

  it('boundary: 0.50 exactly uses low (not very-low) styling', () => {
    render(ConfidenceFlag, { props: { confidence: 0.50, fieldLabel: 'Test' } });
    const alert = screen.getByRole('alert');
    expect(alert.className).toContain('bg-[var(--color-warning-50)]');
  });

  it('boundary: 0.49 uses very-low (danger) styling', () => {
    render(ConfidenceFlag, { props: { confidence: 0.49, fieldLabel: 'Test' } });
    const alert = screen.getByRole('alert');
    expect(alert.className).toContain('bg-[var(--color-danger-50)]');
  });
});
