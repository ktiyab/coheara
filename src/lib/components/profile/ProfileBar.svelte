<!--
  Spec 45 [PU-02]: Profile Indicator Bar
  Persistent bar showing active profile with Switch/Lock actions.
  Visible on ALL screens when a profile is active.
-->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import Avatar from '$lib/components/ui/Avatar.svelte';
  import { PROFILE_COLORS, type ProfileInfo } from '$lib/types/profile';

  interface Props {
    profile: ProfileInfo;
    onSwitch: () => void;
    onLock: () => void;
  }

  let { profile, onSwitch, onLock }: Props = $props();

  let profileColor = $derived(
    profile.color_index != null
      ? PROFILE_COLORS[profile.color_index % PROFILE_COLORS.length]
      : PROFILE_COLORS[0]
  );
</script>

<div
  class="profile-bar"
  role="status"
  aria-label={$t('profile.active_indicator', { values: { name: profile.name } })}
  style:border-left-color={profileColor}
>
  <div class="profile-info">
    <Avatar name={profile.name} size="sm" color={profileColor} />
    <span class="profile-name">{profile.name}</span>
    {#if profile.managed_by}
      <span class="managed-by">
        ({$t('profile.managed_by_label', { values: { managedBy: profile.managed_by } })})
      </span>
    {/if}
  </div>
  <div class="actions">
    <button
      class="bar-action"
      onclick={onSwitch}
      aria-label={$t('profile.switch')}
    >
      {$t('profile.switch')}
    </button>
    <button
      class="bar-action"
      onclick={onLock}
      aria-label={$t('profile.lock')}
    >
      {$t('profile.lock')}
    </button>
  </div>
</div>

<style>
  .profile-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 var(--spacing-md);
    background: var(--surface-secondary);
    border-left: 3px solid;
    position: sticky;
    top: 0;
    z-index: 50;
  }

  .profile-info {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    min-width: 0;
  }

  .profile-name {
    font-size: var(--font-sm);
    font-weight: var(--font-weight-medium);
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .managed-by {
    font-size: var(--font-xs);
    color: var(--text-secondary);
    white-space: nowrap;
  }

  .actions {
    display: flex;
    gap: var(--spacing-xs);
    flex-shrink: 0;
  }

  .bar-action {
    padding: var(--spacing-xs) var(--spacing-sm);
    font-size: var(--font-xs);
    color: var(--text-secondary);
    background: transparent;
    border: 1px solid var(--border-default);
    border-radius: var(--radius-sm);
    cursor: pointer;
    min-height: 28px;
    transition: background 150ms ease;
  }

  .bar-action:hover {
    background: var(--surface-hover);
    color: var(--text-primary);
  }

  .bar-action:focus-visible {
    outline: 2px solid var(--color-primary);
    outline-offset: 1px;
  }
</style>
