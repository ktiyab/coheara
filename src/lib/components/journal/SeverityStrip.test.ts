import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import SeverityStrip from './SeverityStrip.svelte';

describe('SeverityStrip', () => {
  it('renders 5 radio buttons', () => {
    render(SeverityStrip, {
      props: { value: 0, onChange: vi.fn() },
    });
    const radios = screen.getAllByRole('radio');
    expect(radios.length).toBe(5);
  });

  it('each button has aria-label with level and severity name', () => {
    render(SeverityStrip, {
      props: { value: 0, onChange: vi.fn() },
    });
    const radios = screen.getAllByRole('radio');
    expect(radios[0].getAttribute('aria-label')).toContain('1');
    expect(radios[2].getAttribute('aria-label')).toContain('3');
    expect(radios[4].getAttribute('aria-label')).toContain('5');
  });

  it('calls onChange with correct level on click', async () => {
    const onChange = vi.fn();
    render(SeverityStrip, {
      props: { value: 0, onChange },
    });
    const radios = screen.getAllByRole('radio');
    await fireEvent.click(radios[2]);
    expect(onChange).toHaveBeenCalledWith(3);
  });

  it('marks selected button with aria-checked true', () => {
    render(SeverityStrip, {
      props: { value: 3, onChange: vi.fn() },
    });
    const radios = screen.getAllByRole('radio');
    expect(radios[2].getAttribute('aria-checked')).toBe('true');
    expect(radios[0].getAttribute('aria-checked')).toBe('false');
    expect(radios[4].getAttribute('aria-checked')).toBe('false');
  });

  it('applies severity color background to selected button', () => {
    render(SeverityStrip, {
      props: { value: 2, onChange: vi.fn() },
    });
    const radios = screen.getAllByRole('radio');
    expect(radios[1].getAttribute('style')).toContain('#a3e635');
  });

  it('unselected buttons have transparent background', () => {
    render(SeverityStrip, {
      props: { value: 3, onChange: vi.fn() },
    });
    const radios = screen.getAllByRole('radio');
    expect(radios[0].getAttribute('style')).toContain('transparent');
  });

  it('has radiogroup role on container', () => {
    render(SeverityStrip, {
      props: { value: 0, onChange: vi.fn() },
    });
    expect(screen.getByRole('radiogroup')).toBeTruthy();
  });

  it('has minimum touch target size', () => {
    render(SeverityStrip, {
      props: { value: 0, onChange: vi.fn() },
    });
    const radios = screen.getAllByRole('radio');
    for (const radio of radios) {
      expect(radio.className).toContain('min-h-[44px]');
    }
  });
});
