<!-- Visual password strength indicator with 4 segments -->
<script lang="ts">
  import { t } from 'svelte-i18n';

  interface Props {
    password: string;
  }
  let { password }: Props = $props();

  type Strength = 0 | 1 | 2 | 3 | 4;

  let strength = $derived<Strength>(computeStrength(password));

  function computeStrength(pw: string): Strength {
    if (!pw || pw.length === 0) return 0;
    if (pw.length < 10) return 1;

    let score = 1;
    const hasUpper = /[A-Z]/.test(pw);
    const hasLower = /[a-z]/.test(pw);
    const hasDigit = /\d/.test(pw);
    const hasSpecial = /[^A-Za-z0-9]/.test(pw);
    const variety = [hasUpper, hasLower, hasDigit, hasSpecial].filter(Boolean).length;

    if (variety >= 2) score = 2;
    if (variety >= 3 && pw.length >= 12) score = 3;
    if (variety >= 4 && pw.length >= 14) score = 4;

    return score as Strength;
  }

  const labels: Record<Strength, string> = {
    0: '',
    1: 'profile.password_strength_weak',
    2: 'profile.password_strength_fair',
    3: 'profile.password_strength_good',
    4: 'profile.password_strength_strong',
  };

  const colors: Record<Strength, string> = {
    0: 'bg-stone-200 dark:bg-gray-700',
    1: 'bg-red-500',
    2: 'bg-orange-400',
    3: 'bg-emerald-400',
    4: 'bg-emerald-600',
  };
</script>

<div class="flex flex-col gap-1.5">
  <div class="flex gap-1 h-1.5 rounded-full overflow-hidden">
    {#each [1, 2, 3, 4] as segment}
      <div
        class="flex-1 rounded-full transition-colors duration-200 {strength >= segment ? colors[strength] : 'bg-stone-200 dark:bg-gray-700'}"
      ></div>
    {/each}
  </div>
  <span class="text-xs text-stone-500 dark:text-gray-400 min-h-[1rem]">
    {labels[strength] ? $t(labels[strength]) : '\u00a0'}
  </span>
</div>
