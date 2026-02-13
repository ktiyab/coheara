<!-- M1-03: Medication search — in-memory filter over cache -->
<script lang="ts">
	const { query, onQueryChange, visible }: {
		query: string;
		onQueryChange: (query: string) => void;
		visible: boolean;
	} = $props();

	function handleInput(event: Event): void {
		const target = event.target as HTMLInputElement;
		onQueryChange(target.value);
	}

	function handleClear(): void {
		onQueryChange('');
	}
</script>

{#if visible}
	<div class="search-bar">
		<input
			type="search"
			class="search-input"
			placeholder="Search medications..."
			value={query}
			oninput={handleInput}
			aria-label="Search medications"
		/>
		{#if query}
			<button
				class="clear-btn"
				onclick={handleClear}
				aria-label="Clear search"
			>
				&times;
			</button>
		{/if}
	</div>
{/if}

<style>
	.search-bar {
		position: relative;
		margin-bottom: 12px;
	}

	.search-input {
		width: 100%;
		padding: 10px 40px 10px 14px;
		border: 1px solid #D6D3D1;
		border-radius: 12px;
		font-size: 16px;
		font-family: inherit;
		min-height: var(--min-touch-target);
		box-sizing: border-box;
	}

	.search-input:focus {
		outline: 2px solid var(--color-primary);
		outline-offset: -1px;
	}

	.clear-btn {
		position: absolute;
		right: 4px;
		top: 50%;
		transform: translateY(-50%);
		width: 36px;
		height: 36px;
		border: none;
		background: transparent;
		font-size: 20px;
		cursor: pointer;
		color: var(--color-text-muted);
		display: flex;
		align-items: center;
		justify-content: center;
	}
</style>
