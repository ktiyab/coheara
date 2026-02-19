import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import SeverityScale from './SeverityScale.svelte';

describe('SeverityScale', () => {
  it('renders 5 severity level buttons', () => {
    render(SeverityScale, {
      props: { value: 0, onChange: vi.fn(), onNext: vi.fn() },
    });
    const buttons = screen.getAllByRole('button');
    // 5 severity buttons (no "next" button when value is 0)
    expect(buttons.length).toBe(5);
  });

  it('each severity button has an aria-label', () => {
    render(SeverityScale, {
      props: { value: 0, onChange: vi.fn(), onNext: vi.fn() },
    });
    const buttons = screen.getAllByRole('button');
    for (const button of buttons) {
      expect(button.getAttribute('aria-label')).toBeTruthy();
    }
  });

  it('calls onChange when a severity level is clicked', async () => {
    const onChange = vi.fn();
    render(SeverityScale, {
      props: { value: 0, onChange, onNext: vi.fn() },
    });
    const buttons = screen.getAllByRole('button');
    await fireEvent.click(buttons[2]); // Click level 3
    expect(onChange).toHaveBeenCalledWith(3);
  });

  it('shows next button when value >= 1', () => {
    render(SeverityScale, {
      props: { value: 3, onChange: vi.fn(), onNext: vi.fn() },
    });
    // 5 severity buttons + 1 next button
    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBe(6);
  });

  it('does not show next button when value is 0', () => {
    render(SeverityScale, {
      props: { value: 0, onChange: vi.fn(), onNext: vi.fn() },
    });
    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBe(5);
  });

  it('calls onNext when next button is clicked', async () => {
    const onNext = vi.fn();
    render(SeverityScale, {
      props: { value: 2, onChange: vi.fn(), onNext },
    });
    const buttons = screen.getAllByRole('button');
    const nextButton = buttons[buttons.length - 1]; // Last button is "next"
    await fireEvent.click(nextButton);
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('has minimum touch target size on buttons', () => {
    render(SeverityScale, {
      props: { value: 1, onChange: vi.fn(), onNext: vi.fn() },
    });
    const buttons = screen.getAllByRole('button');
    for (const button of buttons) {
      expect(button.className).toContain('min-h-[44px]');
    }
  });
});
