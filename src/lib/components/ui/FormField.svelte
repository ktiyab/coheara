<!--
  C8: FormField — Shared UI Primitive
  Spec: 24-UX-COMPONENTS C8
  Replaces: 5+ inline form field implementations

  Labeled input with error display. Does NOT perform validation —
  displays validation state only.
-->
<script lang="ts">
  interface Props {
    label: string;
    name: string;
    type?: 'text' | 'email' | 'password' | 'number' | 'tel' | 'url' | 'search';
    value?: string;
    placeholder?: string;
    required?: boolean;
    disabled?: boolean;
    error?: string;
    hint?: string;
    autocomplete?: HTMLInputElement['autocomplete'];
    oninput?: (value: string) => void;
    onblur?: () => void;
  }

  let {
    label,
    name,
    type = 'text',
    value = '',
    placeholder,
    required = false,
    disabled = false,
    error,
    hint,
    autocomplete,
    oninput,
    onblur,
  }: Props = $props();

  let inputId = $derived(`field-${name}`);
  let descriptionId = $derived(`field-${name}-desc`);
  let hasDescription = $derived(!!error || !!hint);

  let inputClasses = $derived(
    `w-full px-4 py-3 rounded-lg border text-base min-h-[44px] bg-white
     focus:outline-none transition-colors
     ${error
       ? 'border-red-300 focus:border-red-500'
       : 'border-stone-300 focus:border-[var(--color-primary)]'}
     ${disabled ? 'bg-stone-100 text-stone-400 cursor-not-allowed' : ''}`
  );
</script>

<div class="space-y-1">
  <label for={inputId} class="block text-sm font-medium text-stone-600">
    {label}
    {#if required}
      <span class="text-red-500" aria-hidden="true">*</span>
    {/if}
  </label>

  <input
    id={inputId}
    {name}
    {type}
    {value}
    {placeholder}
    {disabled}
    {autocomplete}
    class={inputClasses}
    aria-required={required}
    aria-invalid={!!error}
    aria-describedby={hasDescription ? descriptionId : undefined}
    oninput={(e) => oninput?.(e.currentTarget.value)}
    {onblur}
  />

  {#if error}
    <p id={descriptionId} class="text-sm text-red-600 mt-1" role="alert">
      {error}
    </p>
  {:else if hint}
    <p id={descriptionId} class="text-sm text-stone-400 mt-1">
      {hint}
    </p>
  {/if}
</div>
