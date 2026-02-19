/**
 * Spec 50 [NF-02]: Sound notification manager.
 * Singleton that plays UI sound effects for completion, alerts, milestones, errors.
 * All sounds are local OGG files — no internet, no analytics.
 */

export type SoundEvent =
  | 'completion'
  | 'alert-info'
  | 'alert-urgent'
  | 'milestone'
  | 'error'
  | 'process-tick';

class SoundManager {
  private audioCache = new Map<SoundEvent, HTMLAudioElement>();
  private enabled = true;
  private volume = 0.5;
  private initialized = false;

  /** Preload all sound files for instant playback. Safe to call multiple times. */
  init() {
    if (this.initialized) return;
    this.initialized = true;

    const events: SoundEvent[] = [
      'completion',
      'alert-info',
      'alert-urgent',
      'milestone',
      'error',
      'process-tick',
    ];

    for (const event of events) {
      try {
        const audio = new Audio(`/sounds/${event}.ogg`);
        audio.preload = 'auto';
        this.audioCache.set(event, audio);
      } catch {
        // Sound file may not exist yet — silently skip
      }
    }
  }

  /** Play a sound event. No-op if sounds are disabled or not initialized. */
  play(event: SoundEvent) {
    if (!this.enabled || !this.initialized) return;

    const audio = this.audioCache.get(event);
    if (audio) {
      audio.volume = this.volume;
      audio.currentTime = 0;
      audio.play().catch(() => {
        // Browser autoplay policy — silently fail
      });
    }
  }

  /** Enable or disable all sounds. */
  setEnabled(value: boolean) {
    this.enabled = value;
  }

  /** Check if sounds are enabled. */
  isEnabled(): boolean {
    return this.enabled;
  }

  /** Set master volume (0.0 - 1.0). */
  setVolume(value: number) {
    this.volume = Math.max(0, Math.min(1, value));
  }

  /** Get current volume. */
  getVolume(): number {
    return this.volume;
  }
}

export const soundManager = new SoundManager();
