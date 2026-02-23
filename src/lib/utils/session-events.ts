/**
 * F7: Typed custom event bridge for profile session transitions.
 * Decouples lock/switch triggers (sidebar, ProfilesScreen) from ProfileGuard
 * without introducing polling delays. Uses DOM CustomEvent for zero-dependency
 * communication within the same window context.
 */

export interface ProfileSwitchDetail {
  targetProfileId?: string;
}

const EVENT_NAME = 'coheara:profile-switch';

/** Dispatch a profile switch event. Pass targetProfileId to pre-select in picker. */
export function dispatchProfileSwitch(targetProfileId?: string): void {
  window.dispatchEvent(
    new CustomEvent<ProfileSwitchDetail>(EVENT_NAME, {
      detail: { targetProfileId },
    }),
  );
}

/** Subscribe to profile switch events. Returns unsubscribe function. */
export function onProfileSwitch(
  handler: (detail: ProfileSwitchDetail) => void,
): () => void {
  function listener(e: Event) {
    handler((e as CustomEvent<ProfileSwitchDetail>).detail);
  }
  window.addEventListener(EVENT_NAME, listener);
  return () => window.removeEventListener(EVENT_NAME, listener);
}
