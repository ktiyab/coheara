import { describe, it, expect, vi, afterEach } from 'vitest';
import { dispatchProfileSwitch, onProfileSwitch } from './session-events';

describe('session-events', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('dispatches CustomEvent with targetProfileId', () => {
    const listener = vi.fn();
    window.addEventListener('coheara:profile-switch', listener);

    dispatchProfileSwitch('abc-123');

    expect(listener).toHaveBeenCalledOnce();
    const event = listener.mock.calls[0][0] as CustomEvent;
    expect(event.detail.targetProfileId).toBe('abc-123');

    window.removeEventListener('coheara:profile-switch', listener);
  });

  it('dispatches without targetProfileId', () => {
    const listener = vi.fn();
    window.addEventListener('coheara:profile-switch', listener);

    dispatchProfileSwitch();

    expect(listener).toHaveBeenCalledOnce();
    const event = listener.mock.calls[0][0] as CustomEvent;
    expect(event.detail.targetProfileId).toBeUndefined();

    window.removeEventListener('coheara:profile-switch', listener);
  });

  it('onProfileSwitch subscribes and receives events', () => {
    const handler = vi.fn();
    const unsub = onProfileSwitch(handler);

    dispatchProfileSwitch('xyz-789');

    expect(handler).toHaveBeenCalledOnce();
    expect(handler).toHaveBeenCalledWith({ targetProfileId: 'xyz-789' });

    unsub();
  });

  it('unsubscribe stops receiving events', () => {
    const handler = vi.fn();
    const unsub = onProfileSwitch(handler);

    dispatchProfileSwitch('first');
    expect(handler).toHaveBeenCalledOnce();

    unsub();

    dispatchProfileSwitch('second');
    expect(handler).toHaveBeenCalledOnce(); // Still 1, not 2
  });

  it('multiple subscribers receive the same event', () => {
    const h1 = vi.fn();
    const h2 = vi.fn();
    const unsub1 = onProfileSwitch(h1);
    const unsub2 = onProfileSwitch(h2);

    dispatchProfileSwitch('shared');

    expect(h1).toHaveBeenCalledWith({ targetProfileId: 'shared' });
    expect(h2).toHaveBeenCalledWith({ targetProfileId: 'shared' });

    unsub1();
    unsub2();
  });
});
