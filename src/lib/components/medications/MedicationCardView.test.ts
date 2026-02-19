import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import MedicationCardView from './MedicationCardView.svelte';
import type { MedicationCard } from '$lib/types/medication';

function createMockMedication(overrides: Partial<MedicationCard> = {}): MedicationCard {
  return {
    id: 'med-001',
    generic_name: 'Metformin',
    brand_name: 'Glucophage',
    dose: '500mg',
    frequency: '2x daily',
    frequency_type: 'regular',
    route: 'oral',
    prescriber_name: 'Dr. Chen',
    prescriber_specialty: 'GP',
    start_date: '2026-01-20',
    end_date: null,
    status: 'active',
    reason_start: 'Diabetes management',
    is_otc: false,
    is_compound: false,
    has_tapering: false,
    dose_type: 'fixed',
    administration_instructions: null,
    condition: 'Type 2 Diabetes',
    coherence_alerts: [],
    ...overrides,
  };
}

describe('MedicationCardView', () => {
  it('renders medication generic name', () => {
    const med = createMockMedication();
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Metformin')).toBeInTheDocument();
  });

  it('renders dose', () => {
    const med = createMockMedication();
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('500mg')).toBeInTheDocument();
  });

  it('renders brand name when present', () => {
    const med = createMockMedication({ brand_name: 'Glucophage' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('(Glucophage)')).toBeInTheDocument();
  });

  it('shows prescriber name for prescribed medication', () => {
    const med = createMockMedication({ prescriber_name: 'Dr. Smith' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Dr. Smith')).toBeInTheDocument();
  });

  it('shows OTC label for over-the-counter medication', () => {
    const med = createMockMedication({ is_otc: true, prescriber_name: null });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('OTC')).toBeInTheDocument();
  });

  it('shows as-needed frequency label', () => {
    const med = createMockMedication({ frequency_type: 'as_needed' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('As needed')).toBeInTheDocument();
  });

  it('shows tapering frequency label', () => {
    const med = createMockMedication({ frequency_type: 'tapering' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Tapering')).toBeInTheDocument();
  });

  it('shows regular frequency string directly', () => {
    const med = createMockMedication({ frequency_type: 'regular', frequency: '3x daily' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('3x daily')).toBeInTheDocument();
  });

  it('shows condition when present', () => {
    const med = createMockMedication({ condition: 'Type 2 Diabetes' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Type 2 Diabetes')).toBeInTheDocument();
  });

  it('does not show condition when absent', () => {
    const med = createMockMedication({ condition: null });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.queryByText('Type 2 Diabetes')).not.toBeInTheDocument();
  });

  it('shows Active badge for active status', () => {
    const med = createMockMedication({ status: 'active' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('shows Paused badge for paused status', () => {
    const med = createMockMedication({ status: 'paused' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Paused')).toBeInTheDocument();
  });

  it('shows compound indicator when is_compound is true', () => {
    const med = createMockMedication({ is_compound: true });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Compound')).toBeInTheDocument();
  });

  it('shows tapering indicator when has_tapering is true', () => {
    const med = createMockMedication({ has_tapering: true });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    // "Tapering" from card_tapering key (distinct from frequency_tapering)
    const taperingElements = screen.getAllByText('Tapering');
    expect(taperingElements.length).toBeGreaterThanOrEqual(1);
  });

  it('calls onTap when card is clicked', async () => {
    const onTap = vi.fn();
    const med = createMockMedication();
    render(MedicationCardView, { props: { medication: med, onTap } });
    const button = screen.getByRole('button');
    await fireEvent.click(button);
    expect(onTap).toHaveBeenCalledOnce();
    expect(onTap).toHaveBeenCalledWith(med);
  });

  it('renders coherence alerts when present', () => {
    const med = createMockMedication({
      coherence_alerts: [
        { id: 'a1', alert_type: 'interaction', severity: 'Critical', summary: 'Drug interaction detected' },
      ],
    });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Drug interaction detected')).toBeInTheDocument();
  });

  it('renders critical alert with amber styling', () => {
    const med = createMockMedication({
      coherence_alerts: [
        { id: 'a1', alert_type: 'interaction', severity: 'Critical', summary: 'Critical alert' },
      ],
    });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    const alert = screen.getByRole('status');
    expect(alert.className).toContain('bg-[var(--color-warning-50)]');
  });

  it('has aria-label on the card button', () => {
    const med = createMockMedication();
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    const button = screen.getByRole('button');
    expect(button.getAttribute('aria-label')).toBeTruthy();
  });

  it('formats route with first letter capitalized', () => {
    const med = createMockMedication({ route: 'oral' });
    render(MedicationCardView, { props: { medication: med, onTap: vi.fn() } });
    expect(screen.getByText('Oral')).toBeInTheDocument();
  });
});
