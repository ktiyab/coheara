<!-- L4-01: Expanded OLDCARTS details — body region, duration, character, aggravating, relieving, timing. -->
<script lang="ts">
  import { t } from 'svelte-i18n';
  import Button from '$lib/components/ui/Button.svelte';

  interface Props {
    bodyRegion: string | null;
    duration: string | null;
    character: string | null;
    aggravating: string[];
    relieving: string[];
    timingPattern: string | null;
    onBodyRegionChange: (value: string | null) => void;
    onDurationChange: (value: string | null) => void;
    onCharacterChange: (value: string | null) => void;
    onAggravatingChange: (value: string[]) => void;
    onRelievingChange: (value: string[]) => void;
    onTimingChange: (value: string | null) => void;
    onNext?: () => void;
  }
  let {
    bodyRegion, duration, character, aggravating, relieving, timingPattern,
    onBodyRegionChange, onDurationChange, onCharacterChange,
    onAggravatingChange, onRelievingChange, onTimingChange, onNext,
  }: Props = $props();

  const durationOptions = ['Constant', 'Minutes', 'Hours', 'Days'];
  const characterOptions = ['Sharp', 'Dull', 'Burning', 'Pressure', 'Throbbing'];
  const aggravatingOptions = ['Activity', 'Food', 'Stress', 'Position', 'Time of day'];
  const relievingOptions = ['Rest', 'Medication', 'Position change', 'Cold/Heat'];
  const timingOptions = ['Morning', 'Night', 'AfterMeals', 'Random', 'AllTheTime'];

  const durationKeys: Record<string, string> = {
    Constant: 'journal.expanded_duration_constant',
    Minutes: 'journal.expanded_duration_minutes',
    Hours: 'journal.expanded_duration_hours',
    Days: 'journal.expanded_duration_days',
  };

  const characterKeys: Record<string, string> = {
    Sharp: 'journal.expanded_character_sharp',
    Dull: 'journal.expanded_character_dull',
    Burning: 'journal.expanded_character_burning',
    Pressure: 'journal.expanded_character_pressure',
    Throbbing: 'journal.expanded_character_throbbing',
  };

  const aggravatingKeys: Record<string, string> = {
    Activity: 'journal.expanded_aggravating_activity',
    Food: 'journal.expanded_aggravating_food',
    Stress: 'journal.expanded_aggravating_stress',
    Position: 'journal.expanded_aggravating_position',
    'Time of day': 'journal.expanded_aggravating_time',
  };

  const relievingKeys: Record<string, string> = {
    Rest: 'journal.expanded_relieving_rest',
    Medication: 'journal.expanded_relieving_medication',
    'Position change': 'journal.expanded_relieving_position',
    'Cold/Heat': 'journal.expanded_relieving_cold_heat',
  };

  const timingKeys: Record<string, string> = {
    Morning: 'journal.expanded_timing_morning',
    Night: 'journal.expanded_timing_night',
    AfterMeals: 'journal.expanded_timing_after_meals',
    Random: 'journal.expanded_timing_random',
    AllTheTime: 'journal.expanded_timing_all_day',
  };

  const bodyRegionKeys: Record<string, string> = {
    head: 'journal.expanded_region_head',
    face: 'journal.expanded_region_face',
    neck: 'journal.expanded_region_neck',
    chest_left: 'journal.expanded_region_chest_left',
    chest_right: 'journal.expanded_region_chest_right',
    chest_center: 'journal.expanded_region_chest_center',
    abdomen_upper: 'journal.expanded_region_abdomen_upper',
    abdomen_lower: 'journal.expanded_region_abdomen_lower',
    back_upper: 'journal.expanded_region_back_upper',
    back_lower: 'journal.expanded_region_back_lower',
    shoulder_left: 'journal.expanded_region_shoulder_left',
    shoulder_right: 'journal.expanded_region_shoulder_right',
    arm_left: 'journal.expanded_region_arm_left',
    arm_right: 'journal.expanded_region_arm_right',
    hand_left: 'journal.expanded_region_hand_left',
    hand_right: 'journal.expanded_region_hand_right',
    hip_left: 'journal.expanded_region_hip_left',
    hip_right: 'journal.expanded_region_hip_right',
    leg_left: 'journal.expanded_region_leg_left',
    leg_right: 'journal.expanded_region_leg_right',
    knee_left: 'journal.expanded_region_knee_left',
    knee_right: 'journal.expanded_region_knee_right',
    foot_left: 'journal.expanded_region_foot_left',
    foot_right: 'journal.expanded_region_foot_right',
  };

  const commonRegions = [
    'head', 'neck', 'chest_center', 'abdomen_upper', 'abdomen_lower',
    'back_upper', 'back_lower', 'shoulder_left', 'shoulder_right',
    'arm_left', 'arm_right', 'leg_left', 'leg_right',
    'knee_left', 'knee_right',
  ];

  function toggleChip(list: string[], item: string, onChange: (v: string[]) => void) {
    if (list.includes(item)) {
      onChange(list.filter(i => i !== item));
    } else {
      onChange([...list, item]);
    }
  }
</script>

<div class="flex flex-col gap-6">
  <!-- Body region -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.expanded_body_title')}</h3>
    <div class="flex flex-wrap gap-2">
      {#each commonRegions as region}
        <button
          class="px-3 py-2 rounded-lg border text-xs min-h-[36px] transition-colors
                 {bodyRegion === region
                   ? 'border-[var(--color-primary)] bg-[var(--color-primary-50)] text-[var(--color-primary)]'
                   : 'border-stone-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
          onclick={() => onBodyRegionChange(bodyRegion === region ? null : region)}
        >
          {bodyRegionKeys[region] ? $t(bodyRegionKeys[region]) : region}
        </button>
      {/each}
    </div>
  </div>

  <!-- Duration -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.expanded_duration_title')}</h3>
    <div class="flex gap-2">
      {#each durationOptions as opt}
        <button
          class="flex-1 px-3 py-2 rounded-lg border text-xs min-h-[44px] transition-colors
                 {duration === opt
                   ? 'border-[var(--color-primary)] bg-[var(--color-primary-50)] text-[var(--color-primary)]'
                   : 'border-stone-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
          onclick={() => onDurationChange(duration === opt ? null : opt)}
        >
          {$t(durationKeys[opt])}
        </button>
      {/each}
    </div>
  </div>

  <!-- Character -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.expanded_character_title')}</h3>
    <div class="flex gap-2">
      {#each characterOptions as opt}
        <button
          class="flex-1 px-3 py-2 rounded-lg border text-xs min-h-[44px] transition-colors
                 {character === opt
                   ? 'border-[var(--color-primary)] bg-[var(--color-primary-50)] text-[var(--color-primary)]'
                   : 'border-stone-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
          onclick={() => onCharacterChange(character === opt ? null : opt)}
        >
          {$t(characterKeys[opt])}
        </button>
      {/each}
    </div>
  </div>

  <!-- Aggravating factors -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.expanded_aggravating_title')}</h3>
    <div class="flex flex-wrap gap-2">
      {#each aggravatingOptions as opt}
        <button
          class="px-3 py-2 rounded-lg border text-xs min-h-[36px] transition-colors
                 {aggravating.includes(opt)
                   ? 'border-[var(--color-primary)] bg-[var(--color-primary-50)] text-[var(--color-primary)]'
                   : 'border-stone-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
          onclick={() => toggleChip(aggravating, opt, onAggravatingChange)}
        >
          {$t(aggravatingKeys[opt])}
        </button>
      {/each}
    </div>
  </div>

  <!-- Relieving factors -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.expanded_relieving_title')}</h3>
    <div class="flex flex-wrap gap-2">
      {#each relievingOptions as opt}
        <button
          class="px-3 py-2 rounded-lg border text-xs min-h-[36px] transition-colors
                 {relieving.includes(opt)
                   ? 'border-[var(--color-primary)] bg-[var(--color-primary-50)] text-[var(--color-primary)]'
                   : 'border-stone-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
          onclick={() => toggleChip(relieving, opt, onRelievingChange)}
        >
          {$t(relievingKeys[opt])}
        </button>
      {/each}
    </div>
  </div>

  <!-- Timing pattern -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 dark:text-gray-300 mb-2">{$t('journal.expanded_timing_title')}</h3>
    <div class="flex flex-wrap gap-2">
      {#each timingOptions as opt}
        <button
          class="px-3 py-2 rounded-lg border text-xs min-h-[36px] transition-colors
                 {timingPattern === opt
                   ? 'border-[var(--color-primary)] bg-[var(--color-primary-50)] text-[var(--color-primary)]'
                   : 'border-stone-200 dark:border-gray-700 bg-white dark:bg-gray-900 text-stone-600 dark:text-gray-300 hover:bg-stone-50 dark:hover:bg-gray-800'}"
          onclick={() => onTimingChange(timingPattern === opt ? null : opt)}
        >
          {$t(timingKeys[opt]) ?? opt}
        </button>
      {/each}
    </div>
  </div>

  <!-- Continue -->
  {#if onNext}
    <Button variant="primary" fullWidth onclick={onNext}>
      {$t('common.continue')}
    </Button>
  {/if}
</div>
