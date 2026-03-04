<!-- ME-04 B6: Invariant explainer + calibration status. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import type { MeIdentity } from '$lib/types/me';

  let { identity }: { identity: MeIdentity } = $props();

  const ASIAN_ETHNICITIES = ['SouthAsian', 'EastAsian', 'PacificIslander'];
  let hasAsianBmi = $derived(
    identity.ethnicities.some(e => ASIAN_ETHNICITIES.includes(e))
  );

  type CalItem = { set: boolean; text: string };

  let calibrations = $derived<CalItem[]>((() => {
    const items: CalItem[] = [];

    if (identity.age != null) {
      items.push({
        set: true,
        text: $t('me.cal_age_active', { values: { age: identity.age } }),
      });
    } else {
      items.push({ set: false, text: $t('me.cal_age_missing') });
    }

    if (identity.sex) {
      const sexLabel = identity.sex === 'male'
        ? $t('me.sex_male')
        : $t('me.sex_female');
      items.push({
        set: true,
        text: $t('me.cal_sex_active', { values: { sex: sexLabel } }),
      });
    } else {
      items.push({ set: false, text: $t('me.cal_sex_missing') });
    }

    if (identity.ethnicities.length > 0) {
      if (hasAsianBmi) {
        items.push({ set: true, text: $t('me.cal_ethnicity_asian') });
      } else {
        items.push({ set: true, text: $t('me.cal_ethnicity_standard') });
      }
    } else {
      items.push({ set: false, text: $t('me.cal_ethnicity_missing') });
    }

    if (identity.bmi != null) {
      items.push({
        set: true,
        text: $t('me.cal_bmi_active', {
          values: { bmi: identity.bmi.toFixed(1) },
        }),
      });
    } else {
      items.push({ set: false, text: $t('me.cal_wh_missing') });
    }

    return items;
  })());
</script>

<div class="p-4 rounded-xl bg-blue-50/50 dark:bg-blue-950/20 border
            border-blue-100 dark:border-blue-900/30">
  <div class="flex items-start gap-2 mb-2">
    <span class="text-blue-500 dark:text-blue-400 text-sm mt-0.5">i</span>
    <h3 class="text-sm font-semibold text-blue-800 dark:text-blue-200">
      {$t('me.cal_heading')}
    </h3>
  </div>

  <p class="text-xs text-blue-700/80 dark:text-blue-300/70 mb-3 leading-relaxed">
    {$t('me.cal_description')}
  </p>

  <div class="space-y-1.5 mb-3">
    {#each calibrations as item}
      <div class="flex items-center gap-2 text-xs">
        <span class="{item.set
          ? 'text-teal-500 dark:text-teal-400'
          : 'text-stone-400 dark:text-gray-400'}">
          {item.set ? '\u2713' : '\u25cb'}
        </span>
        <span class="{item.set
          ? 'text-blue-700 dark:text-blue-300'
          : 'text-blue-600/60 dark:text-blue-400/50 italic'}">
          {item.text}
        </span>
      </div>
    {/each}
  </div>

  <p class="text-[10px] text-blue-500/60 dark:text-blue-400/40">
    {$t('me.cal_sources')}
  </p>
</div>
