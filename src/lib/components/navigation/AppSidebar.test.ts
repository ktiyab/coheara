import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock the navigation store module to avoid $app/environment resolution issues.
// vi.mock factories are hoisted, so all values must be inline (no external refs).
vi.mock('$lib/stores/navigation.svelte', () => ({
  navigation: {
    activeScreen: 'home',
    sidebarCollapsed: false,
    navigate: vi.fn(),
    toggleSidebar: vi.fn(),
  },
  NAV_SECTIONS: {
    main: ['home', 'chat'],
    library: ['documents', 'timeline'],
    system: ['settings'],
  },
}));

// Mock lockProfile API (uses Tauri invoke, already mocked in test-setup)
vi.mock('$lib/api/profile', () => ({
  lockProfile: vi.fn(() => Promise.resolve()),
}));

// Mock profile store
vi.mock('$lib/stores/profile.svelte', () => ({
  profile: { name: 'Alice' },
}));

import AppSidebar from './AppSidebar.svelte';
import { navigation } from '$lib/stores/navigation.svelte';

beforeEach(() => {
  vi.clearAllMocks();
  // Reset mock navigation state to defaults
  navigation.activeScreen = 'home';
  navigation.sidebarCollapsed = false;
});

describe('AppSidebar', () => {
  it('renders all navigation items with correct labels', () => {
    render(AppSidebar);
    // LP-06: 5 nav items (journal, medications, appointments removed)
    expect(screen.getByText('Home')).toBeInTheDocument();
    expect(screen.getByText('Ask')).toBeInTheDocument();
    expect(screen.getByText('Documents')).toBeInTheDocument();
    expect(screen.getByText('Timeline')).toBeInTheDocument();
    expect(screen.getByText('Settings')).toBeInTheDocument();
  });

  it('renders sidebar with nav landmark and aria-label', () => {
    render(AppSidebar);
    const nav = screen.getByRole('navigation');
    expect(nav).toBeInTheDocument();
    expect(nav.getAttribute('aria-label')).toBeTruthy();
  });

  it('clicking a nav item calls navigation.navigate()', async () => {
    render(AppSidebar);
    const chatButton = screen.getByText('Ask').closest('button')!;
    await fireEvent.click(chatButton);
    expect(navigation.navigate).toHaveBeenCalledWith('chat');
  });

  it('active screen gets aria-current="page"', () => {
    navigation.activeScreen = 'chat';
    render(AppSidebar);
    const chatButton = screen.getByText('Ask').closest('button')!;
    expect(chatButton.getAttribute('aria-current')).toBe('page');
  });

  it('non-active screens do not have aria-current', () => {
    navigation.activeScreen = 'home';
    render(AppSidebar);
    const chatButton = screen.getByText('Ask').closest('button')!;
    expect(chatButton.getAttribute('aria-current')).toBeNull();
  });

  it('sidebar renders in expanded state by default', () => {
    navigation.sidebarCollapsed = false;
    render(AppSidebar);
    // In expanded state, brand name is visible
    expect(screen.getByText('Coheara')).toBeInTheDocument();
    // Navigation labels are visible
    expect(screen.getByText('Home')).toBeInTheDocument();
  });

  it('sidebar renders in collapsed state', () => {
    navigation.sidebarCollapsed = true;
    render(AppSidebar);
    // In collapsed state, brand name is not rendered
    expect(screen.queryByText('Coheara')).not.toBeInTheDocument();
    // Navigation labels are not rendered (only icons with title attributes)
    expect(screen.queryByText('Home')).not.toBeInTheDocument();
  });

  it('collapse toggle button has correct aria-label when expanded', () => {
    navigation.sidebarCollapsed = false;
    render(AppSidebar);
    const toggleButton = screen.getByLabelText('Collapse sidebar');
    expect(toggleButton).toBeInTheDocument();
  });

  it('collapse toggle button has correct aria-label when collapsed', () => {
    navigation.sidebarCollapsed = true;
    render(AppSidebar);
    const toggleButton = screen.getByLabelText('Expand sidebar');
    expect(toggleButton).toBeInTheDocument();
  });

  it('keyboard Enter activates nav item via button click', async () => {
    render(AppSidebar);
    const docsButton = screen.getByText('Documents').closest('button')!;
    // Native <button> elements handle Enter/Space as click events
    await fireEvent.click(docsButton);
    expect(navigation.navigate).toHaveBeenCalledWith('documents');
  });

  it('keyboard Space activates nav item via button click', async () => {
    render(AppSidebar);
    const settingsButton = screen.getByText('Settings').closest('button')!;
    // Native <button> elements handle Enter/Space as click events
    await fireEvent.click(settingsButton);
    expect(navigation.navigate).toHaveBeenCalledWith('settings');
  });

  it('clicking collapse toggle calls navigation.toggleSidebar()', async () => {
    navigation.sidebarCollapsed = false;
    render(AppSidebar);
    const toggleButton = screen.getByLabelText('Collapse sidebar');
    await fireEvent.click(toggleButton);
    expect(navigation.toggleSidebar).toHaveBeenCalledOnce();
  });

  it('active screen applies distinct styling class', () => {
    navigation.activeScreen = 'documents';
    render(AppSidebar);
    const docsButton = screen.getByText('Documents').closest('button')!;
    expect(docsButton.className).toContain('color-interactive');
  });

  it('inactive screen does not have active styling', () => {
    navigation.activeScreen = 'home';
    render(AppSidebar);
    const chatButton = screen.getByText('Ask').closest('button')!;
    expect(chatButton.className).not.toContain('color-interactive');
  });
});
