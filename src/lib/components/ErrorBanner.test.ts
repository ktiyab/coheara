import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import ErrorBanner from './ErrorBanner.svelte';

describe('ErrorBanner', () => {
  it('renders error message', () => {
    render(ErrorBanner, { props: { message: 'Something went wrong' } });
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
  });

  it('has role="alert" for accessibility', () => {
    render(ErrorBanner, { props: { message: 'Error occurred' } });
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('renders guidance text when provided', () => {
    render(ErrorBanner, {
      props: {
        message: 'Import failed',
        guidance: 'Try importing the file again.',
      },
    });
    expect(screen.getByText('Try importing the file again.')).toBeInTheDocument();
  });

  it('does not render guidance when not provided', () => {
    render(ErrorBanner, { props: { message: 'Error' } });
    expect(screen.queryByText('Try importing')).not.toBeInTheDocument();
  });

  it('renders action button when label and handler provided', () => {
    const onAction = vi.fn();
    render(ErrorBanner, {
      props: {
        message: 'Connection lost',
        actionLabel: 'Retry',
        onAction,
      },
    });
    const button = screen.getByText('Retry');
    expect(button).toBeInTheDocument();
  });

  it('calls onAction when action button is clicked', async () => {
    const onAction = vi.fn();
    render(ErrorBanner, {
      props: {
        message: 'Connection lost',
        actionLabel: 'Retry',
        onAction,
      },
    });
    await fireEvent.click(screen.getByText('Retry'));
    expect(onAction).toHaveBeenCalledOnce();
  });

  it('hides banner after dismiss button is clicked', async () => {
    render(ErrorBanner, {
      props: { message: 'Dismissible error', dismissible: true },
    });
    expect(screen.getByRole('alert')).toBeInTheDocument();

    const dismissBtn = screen.getByLabelText('Dismiss');
    await fireEvent.click(dismissBtn);

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('calls onDismiss callback when dismissed', async () => {
    const onDismiss = vi.fn();
    render(ErrorBanner, {
      props: { message: 'Error', dismissible: true, onDismiss },
    });
    await fireEvent.click(screen.getByLabelText('Dismiss'));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('does not show dismiss button when dismissible is false', () => {
    render(ErrorBanner, {
      props: { message: 'Persistent error', dismissible: false },
    });
    expect(screen.queryByLabelText('Dismiss')).not.toBeInTheDocument();
  });

  it('applies warning severity styles', () => {
    render(ErrorBanner, {
      props: { message: 'Warning message', severity: 'warning' },
    });
    const alert = screen.getByRole('alert');
    expect(alert.className).toContain('bg-[var(--color-warning-50)]');
  });

  it('applies info severity styles', () => {
    render(ErrorBanner, {
      props: { message: 'Info message', severity: 'info' },
    });
    const alert = screen.getByRole('alert');
    expect(alert.className).toContain('bg-[var(--color-info-50)]');
  });

  it('defaults to error severity', () => {
    render(ErrorBanner, { props: { message: 'Default error' } });
    const alert = screen.getByRole('alert');
    expect(alert.className).toContain('bg-[var(--color-danger-50)]');
  });
});
