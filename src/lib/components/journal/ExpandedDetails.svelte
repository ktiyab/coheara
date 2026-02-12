<!-- L4-01: Expanded OLDCARTS details — body region, duration, character, aggravating, relieving, timing. -->
<script lang="ts">
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
    onNext: () => void;
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

  const timingLabels: Record<string, string> = {
    Morning: 'Morning',
    Night: 'Night',
    AfterMeals: 'After meals',
    Random: 'Random',
    AllTheTime: 'All the time',
  };

  const bodyRegionLabels: Record<string, string> = {
    head: 'Head', face: 'Face', neck: 'Neck',
    chest_left: 'Left chest', chest_right: 'Right chest', chest_center: 'Center chest',
    abdomen_upper: 'Upper abdomen', abdomen_lower: 'Lower abdomen',
    back_upper: 'Upper back', back_lower: 'Lower back',
    shoulder_left: 'Left shoulder', shoulder_right: 'Right shoulder',
    arm_left: 'Left arm', arm_right: 'Right arm',
    hand_left: 'Left hand', hand_right: 'Right hand',
    hip_left: 'Left hip', hip_right: 'Right hip',
    leg_left: 'Left leg', leg_right: 'Right leg',
    knee_left: 'Left knee', knee_right: 'Right knee',
    foot_left: 'Left foot', foot_right: 'Right foot',
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
    <h3 class="text-sm font-medium text-stone-600 mb-2">Where do you feel it?</h3>
    <div class="flex flex-wrap gap-2">
      {#each commonRegions as region}
        <button
          class="px-3 py-2 rounded-lg border text-xs min-h-[36px] transition-colors
                 {bodyRegion === region
                   ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                   : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
          onclick={() => onBodyRegionChange(bodyRegion === region ? null : region)}
        >
          {bodyRegionLabels[region] ?? region}
        </button>
      {/each}
    </div>
  </div>

  <!-- Duration -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 mb-2">How long does it last each time?</h3>
    <div class="flex gap-2">
      {#each durationOptions as opt}
        <button
          class="flex-1 px-3 py-2 rounded-lg border text-xs min-h-[44px] transition-colors
                 {duration === opt
                   ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                   : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
          onclick={() => onDurationChange(duration === opt ? null : opt)}
        >
          {opt === 'Minutes' ? 'A few minutes' : opt === 'Hours' ? 'A few hours' : opt === 'Days' ? 'Days or more' : opt}
        </button>
      {/each}
    </div>
  </div>

  <!-- Character -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 mb-2">What does it feel like?</h3>
    <div class="flex gap-2">
      {#each characterOptions as opt}
        <button
          class="flex-1 px-3 py-2 rounded-lg border text-xs min-h-[44px] transition-colors
                 {character === opt
                   ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                   : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
          onclick={() => onCharacterChange(character === opt ? null : opt)}
        >
          {opt}
        </button>
      {/each}
    </div>
  </div>

  <!-- Aggravating factors -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 mb-2">What makes it worse?</h3>
    <div class="flex flex-wrap gap-2">
      {#each aggravatingOptions as opt}
        <button
          class="px-3 py-2 rounded-lg border text-xs min-h-[36px] transition-colors
                 {aggravating.includes(opt)
                   ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                   : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
          onclick={() => toggleChip(aggravating, opt, onAggravatingChange)}
        >
          {opt}
        </button>
      {/each}
    </div>
  </div>

  <!-- Relieving factors -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 mb-2">What makes it better?</h3>
    <div class="flex flex-wrap gap-2">
      {#each relievingOptions as opt}
        <button
          class="px-3 py-2 rounded-lg border text-xs min-h-[36px] transition-colors
                 {relieving.includes(opt)
                   ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                   : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
          onclick={() => toggleChip(relieving, opt, onRelievingChange)}
        >
          {opt}
        </button>
      {/each}
    </div>
  </div>

  <!-- Timing pattern -->
  <div>
    <h3 class="text-sm font-medium text-stone-600 mb-2">When does it usually happen?</h3>
    <div class="flex flex-wrap gap-2">
      {#each timingOptions as opt}
        <button
          class="px-3 py-2 rounded-lg border text-xs min-h-[36px] transition-colors
                 {timingPattern === opt
                   ? 'border-[var(--color-primary)] bg-blue-50 text-[var(--color-primary)]'
                   : 'border-stone-200 bg-white text-stone-600 hover:bg-stone-50'}"
          onclick={() => onTimingChange(timingPattern === opt ? null : opt)}
        >
          {timingLabels[opt] ?? opt}
        </button>
      {/each}
    </div>
  </div>

  <!-- Continue -->
  <button
    class="w-full px-4 py-3 bg-[var(--color-primary)] text-white rounded-xl
           font-medium min-h-[44px]"
    onclick={onNext}
  >
    Continue
  </button>
</div>
