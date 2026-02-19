/**
 * Focus trap utility for modal dialogs.
 * Traps Tab/Shift+Tab within the container element.
 */

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Handle keydown to trap focus within a container.
 * Call this from your dialog's onkeydown handler.
 */
export function trapFocus(event: KeyboardEvent, container: HTMLElement): void {
  if (event.key !== 'Tab') return;

  const focusable = Array.from(
    container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)
  );
  if (focusable.length === 0) return;

  const first = focusable[0];
  const last = focusable[focusable.length - 1];

  if (event.shiftKey) {
    if (document.activeElement === first || document.activeElement === container) {
      event.preventDefault();
      last.focus();
    }
  } else {
    if (document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
}

/**
 * Auto-focus the first focusable element in a container,
 * or the container itself if nothing is focusable.
 */
export function autoFocusFirst(container: HTMLElement): void {
  const first = container.querySelector<HTMLElement>(FOCUSABLE_SELECTOR);
  if (first) {
    first.focus();
  } else {
    container.focus();
  }
}
