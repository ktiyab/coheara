<!-- M1-01/M1-03: More menu — Timeline, Labs, Appointments, Capture, Settings -->
<script lang="ts">
	const menuItems = [
		{ label: 'Timeline', description: 'Health events over time', href: '/more/timeline' },
		{ label: 'Lab Results', description: 'Lab values and trends', href: '/more/labs' },
		{ label: 'Appointments', description: 'Upcoming and past visits', href: '/more/appointments' },
		{ label: 'Document Capture', description: 'Scan documents with camera', href: '/more/capture' },
		{ label: 'Settings', description: 'Connection and preferences', href: '/more/settings' }
	];
</script>

<div class="more-menu">
	<h1>More</h1>

	<nav aria-label="Additional features">
		{#each menuItems as item (item.label)}
			<a class="menu-item" href={item.href}>
				<span class="menu-label">{item.label}</span>
				<span class="menu-description">{item.description}</span>
			</a>
		{/each}
	</nav>
</div>

<style>
	.more-menu {
		padding: 16px;
	}

	h1 {
		font-size: var(--font-header);
		font-weight: 700;
		margin: 0 0 20px;
	}

	.menu-item {
		display: flex;
		flex-direction: column;
		padding: 16px;
		margin-bottom: 8px;
		background: white;
		border-radius: 12px;
		text-decoration: none;
		color: var(--color-text);
		min-height: var(--large-touch-target);
		border: 1px solid #E7E5E4;
	}

	.menu-label {
		font-size: 17px;
		font-weight: 600;
	}

	.menu-description {
		font-size: 14px;
		color: var(--color-text-muted);
		margin-top: 2px;
	}
</style>
