# L3-01 — Profile Management

<!--
=============================================================================
COMPONENT SPEC — First screen. The gateway to all patient data.
Engineer review: E-UX (UI/UX, lead), E-SC (Security), E-RS (Rust), E-QA (QA)
Marie sees THIS before anything else. It must be simple, warm, trustworthy.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=28` |
| [2] Dependencies | `offset=48 limit=18` |
| [3] Interfaces | `offset=66 limit=50` |
| [4] First-Launch Flow | `offset=116 limit=70` |
| [5] Profile Picker Screen | `offset=186 limit=45` |
| [6] Profile Lock/Unlock | `offset=231 limit=35` |
| [7] Recovery Phrase Display | `offset=266 limit=35` |
| [8] Tauri Commands (IPC) | `offset=301 limit=50` |
| [9] Svelte Components | `offset=351 limit=85` |
| [10] Error Handling | `offset=436 limit=25` |
| [11] Security | `offset=461 limit=25` |
| [12] Testing | `offset=486 limit=45` |
| [13] Performance | `offset=531 limit=10` |
| [14] Open Questions | `offset=541 limit=10` |

---

## [1] Identity

**What:** The profile management UI and logic — the very first screen users encounter. Includes: profile picker (list of profiles), profile creation with the trust screen and first-launch walkthrough, profile unlock (password entry), profile switching, caregiver attribution, recovery phrase display (one-time at creation), and session management (lock on inactivity, close on exit).

**After this session:**
- First launch: trust screen → profile creation → password → recovery phrase shown once
- Subsequent launches: profile picker → select profile → enter password → unlocked
- Multiple profiles visible (Sophie managing Marie and child)
- Caregiver attribution: "Managed by Sophie" shown on profile card
- Recovery phrase displayed once after creation, never stored in UI
- Profile lock after configurable inactivity timeout (default: 15 minutes)
- Profile switch without closing app (lock current → pick new)
- Password hint display on profile card (optional, user-set)
- Tauri global state holds active ProfileSession
- Frontend routing guard: no screen accessible without active session

**Estimated complexity:** Medium
**Source:** Tech Spec v1.1 Section 9.3 (First-Launch Flow)

---

## [2] Dependencies

**Incoming:**
- L0-03 (encryption — ProfileManager trait, ProfileSession, RecoveryPhrase)
- L0-01 (project scaffold — Tauri state management, routing)

**Outgoing:**
- All other L3+ components (require active ProfileSession to function)
- L3-02 (home screen — navigated to after profile unlock)

**No new dependencies.** Uses Tauri state and L0-03 crypto primitives.

---

## [3] Interfaces

### Tauri State

```rust
// Global Tauri state for active profile session
pub struct AppState {
    pub active_session: std::sync::Mutex<Option<ProfileSession>>,
    pub profile_manager: Box<dyn ProfileManager + Send + Sync>,
    pub inactivity_timeout_secs: u64,  // Default: 900 (15 min)
    pub last_activity: std::sync::Mutex<std::time::Instant>,
}

impl AppState {
    pub fn is_locked(&self) -> bool {
        self.active_session.lock().unwrap().is_none()
    }

    pub fn session(&self) -> Option<std::sync::MutexGuard<'_, Option<ProfileSession>>> {
        let guard = self.active_session.lock().unwrap();
        if guard.is_some() { Some(guard) } else { None }
    }

    pub fn lock(&self) {
        let mut session = self.active_session.lock().unwrap();
        *session = None;  // ProfileSession::drop() zeros the key
        tracing::info!("Profile locked");
    }

    pub fn update_activity(&self) {
        let mut last = self.last_activity.lock().unwrap();
        *last = std::time::Instant::now();
    }

    pub fn check_timeout(&self) -> bool {
        let last = self.last_activity.lock().unwrap();
        last.elapsed().as_secs() > self.inactivity_timeout_secs
    }
}
```

### Frontend Types

```typescript
// src/lib/types/profile.ts
export interface ProfileInfo {
  id: string;
  name: string;
  created_at: string;
  managed_by: string | null;
  password_hint: string | null;
}

export interface ProfileCreateResult {
  profile: ProfileInfo;
  recovery_phrase: string[];  // 12 words
}

export type AppScreen = 'trust' | 'create' | 'picker' | 'unlock' | 'recovery' | 'app';
```

---

## [4] First-Launch Flow

**E-UX lead:** This is Marie's first moment with Coheara. Every word matters. No jargon. No intimidation. Warmth and trust.

### Step 0: Trust Screen

Displayed once per installation (before any profile exists).

```svelte
<!-- src/lib/components/profile/TrustScreen.svelte -->
<script lang="ts">
  interface Props {
    onContinue: () => void;
  }
  let { onContinue }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-8 max-w-lg mx-auto">
  <h1 class="text-3xl font-bold text-stone-800">Welcome to Coheara</h1>
  <p class="text-lg text-stone-600 text-center leading-relaxed">
    Your personal medical document assistant.
  </p>

  <div class="flex flex-col gap-4 text-stone-600 text-base">
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>Coheara runs entirely on this computer. Your medical documents are <strong>never sent anywhere</strong>.</p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>No internet connection is needed after installation. <strong>No account required.</strong></p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>Your data is <strong>encrypted</strong> and only you can access it with your password.</p>
    </div>
    <div class="flex items-start gap-3">
      <span class="text-green-600 mt-1">&#x2713;</span>
      <p>Coheara helps you <strong>understand</strong> your health documents. It does <strong>not</strong> give medical advice.</p>
    </div>
  </div>

  <button
    class="mt-4 px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg font-medium
           hover:brightness-110 focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]
           min-h-[44px] min-w-[44px]"
    onclick={onContinue}
  >
    I understand, let's begin
  </button>
</div>
```

### Step 1: Profile Creation

```svelte
<!-- src/lib/components/profile/CreateProfile.svelte -->
<script lang="ts">
  import { createProfile, type ProfileCreateResult } from '$lib/api/profile';

  interface Props {
    onCreated: (result: ProfileCreateResult) => void;
    onError: (error: string) => void;
  }
  let { onCreated, onError }: Props = $props();

  let name = $state('');
  let password = $state('');
  let confirmPassword = $state('');
  let caregiverMode = $state(false);
  let caregiverName = $state('');
  let loading = $state(false);
  let passwordError = $state('');

  function validatePassword(): boolean {
    if (password.length < 6) {
      passwordError = 'Password must be at least 6 characters';
      return false;
    }
    if (password !== confirmPassword) {
      passwordError = 'Passwords do not match';
      return false;
    }
    passwordError = '';
    return true;
  }

  async function handleCreate() {
    if (!name.trim()) return;
    if (!validatePassword()) return;

    loading = true;
    try {
      const result = await createProfile(
        name.trim(),
        password,
        caregiverMode ? caregiverName.trim() : null,
      );
      onCreated(result);
    } catch (e) {
      onError(String(e));
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-md mx-auto">
  <h2 class="text-2xl font-bold text-stone-800">Create your profile</h2>

  <div class="w-full flex flex-col gap-4">
    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">What's your name?</span>
      <input
        type="text"
        bind:value={name}
        placeholder="Marie"
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="off"
      />
    </label>

    <div class="flex items-center gap-2">
      <input type="checkbox" id="caregiver" bind:checked={caregiverMode}
             class="min-h-[44px] min-w-[44px]" />
      <label for="caregiver" class="text-stone-600 text-sm">
        I'm setting this up for someone I care for
      </label>
    </div>

    {#if caregiverMode}
      <label class="flex flex-col gap-1">
        <span class="text-stone-600 text-sm font-medium">Your name (caregiver)</span>
        <input
          type="text"
          bind:value={caregiverName}
          placeholder="Sophie"
          class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
        />
      </label>
    {/if}

    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">Create a password</span>
      <input
        type="password"
        bind:value={password}
        placeholder="At least 6 characters"
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="new-password"
      />
    </label>

    <label class="flex flex-col gap-1">
      <span class="text-stone-600 text-sm font-medium">Confirm password</span>
      <input
        type="password"
        bind:value={confirmPassword}
        placeholder="Type it again"
        class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
               focus:border-[var(--color-primary)] focus:outline-none"
        autocomplete="new-password"
      />
    </label>

    {#if passwordError}
      <p class="text-red-600 text-sm">{passwordError}</p>
    {/if}

    <button
      class="mt-2 px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
             font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
      onclick={handleCreate}
      disabled={loading || !name.trim() || !password}
    >
      {loading ? 'Creating...' : 'Create profile'}
    </button>
  </div>
</div>
```

### Step 2: Recovery Phrase (One-Time Display)

```svelte
<!-- src/lib/components/profile/RecoveryPhraseDisplay.svelte -->
<script lang="ts">
  interface Props {
    words: string[];
    onConfirmed: () => void;
  }
  let { words, onConfirmed }: Props = $props();

  let confirmed = $state(false);
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-lg mx-auto">
  <h2 class="text-2xl font-bold text-stone-800">Your recovery phrase</h2>

  <p class="text-stone-600 text-center leading-relaxed">
    Write these 12 words down on paper and keep them safe.
    If you forget your password, these words are the <strong>only way</strong>
    to recover your data.
  </p>

  <div class="grid grid-cols-3 gap-3 w-full p-6 bg-white rounded-xl border border-stone-200 shadow-sm">
    {#each words as word, i}
      <div class="flex items-center gap-2 p-2 bg-stone-50 rounded-lg">
        <span class="text-stone-400 text-sm w-5 text-right">{i + 1}.</span>
        <span class="text-stone-800 font-mono text-lg">{word}</span>
      </div>
    {/each}
  </div>

  <div class="flex flex-col gap-3 w-full mt-4">
    <p class="text-stone-500 text-sm text-center">
      This phrase will NOT be shown again. Please write it down now.
    </p>

    <label class="flex items-center gap-3 justify-center">
      <input type="checkbox" bind:checked={confirmed}
             class="min-h-[44px] min-w-[44px]" />
      <span class="text-stone-700">I have written down my recovery phrase</span>
    </label>

    <button
      class="px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
             font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
      onclick={onConfirmed}
      disabled={!confirmed}
    >
      Continue
    </button>
  </div>
</div>
```

---

## [5] Profile Picker Screen

Shown when multiple profiles exist or after locking.

```svelte
<!-- src/lib/components/profile/ProfilePicker.svelte -->
<script lang="ts">
  import type { ProfileInfo } from '$lib/types/profile';

  interface Props {
    profiles: ProfileInfo[];
    onSelect: (profile: ProfileInfo) => void;
    onCreateNew: () => void;
  }
  let { profiles, onSelect, onCreateNew }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-8 max-w-md mx-auto">
  <h2 class="text-2xl font-bold text-stone-800">Who's using Coheara?</h2>

  <div class="flex flex-col gap-3 w-full">
    {#each profiles as profile}
      <button
        class="w-full flex items-center gap-4 p-4 bg-white rounded-xl border border-stone-200
               hover:border-[var(--color-primary)] hover:shadow-sm transition-all
               min-h-[44px] text-left"
        onclick={() => onSelect(profile)}
      >
        <div class="w-12 h-12 rounded-full bg-stone-200 flex items-center justify-center
                    text-stone-600 text-xl font-bold">
          {profile.name.charAt(0).toUpperCase()}
        </div>
        <div class="flex flex-col">
          <span class="text-stone-800 font-medium text-lg">{profile.name}</span>
          {#if profile.managed_by}
            <span class="text-stone-400 text-sm">Managed by {profile.managed_by}</span>
          {/if}
        </div>
      </button>
    {/each}
  </div>

  <button
    class="px-6 py-3 border border-dashed border-stone-300 rounded-xl text-stone-500
           hover:border-[var(--color-primary)] hover:text-[var(--color-primary)]
           transition-all min-h-[44px]"
    onclick={onCreateNew}
  >
    + Create new profile
  </button>
</div>
```

---

## [6] Profile Lock/Unlock

### Unlock Screen

```svelte
<!-- src/lib/components/profile/UnlockProfile.svelte -->
<script lang="ts">
  import { unlockProfile } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';

  interface Props {
    profile: ProfileInfo;
    onUnlocked: () => void;
    onBack: () => void;
    onForgotPassword: () => void;
  }
  let { profile, onUnlocked, onBack, onForgotPassword }: Props = $props();

  let password = $state('');
  let error = $state('');
  let loading = $state(false);
  let attempts = $state(0);

  async function handleUnlock() {
    if (!password) return;
    loading = true;
    error = '';

    try {
      await unlockProfile(profile.id, password);
      onUnlocked();
    } catch (e) {
      attempts += 1;
      if (attempts >= 3) {
        error = 'Wrong password. If you forgot it, use your recovery phrase.';
      } else {
        error = 'Wrong password. Please try again.';
      }
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-md mx-auto">
  <button
    class="self-start text-stone-400 hover:text-stone-600 min-h-[44px] min-w-[44px]"
    onclick={onBack}
    aria-label="Back to profile list"
  >
    &larr; Back
  </button>

  <div class="w-16 h-16 rounded-full bg-stone-200 flex items-center justify-center
              text-stone-600 text-2xl font-bold">
    {profile.name.charAt(0).toUpperCase()}
  </div>

  <h2 class="text-2xl font-bold text-stone-800">{profile.name}</h2>

  {#if profile.password_hint}
    <p class="text-stone-400 text-sm">Hint: {profile.password_hint}</p>
  {/if}

  <label class="w-full flex flex-col gap-1">
    <span class="text-stone-600 text-sm font-medium">Password</span>
    <input
      type="password"
      bind:value={password}
      placeholder="Enter your password"
      class="px-4 py-3 rounded-lg border border-stone-300 text-lg min-h-[44px]
             focus:border-[var(--color-primary)] focus:outline-none"
      autocomplete="current-password"
      onkeydown={(e) => { if (e.key === 'Enter') handleUnlock(); }}
    />
  </label>

  {#if error}
    <p class="text-red-600 text-sm">{error}</p>
  {/if}

  <button
    class="w-full px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
           font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
    onclick={handleUnlock}
    disabled={loading || !password}
  >
    {loading ? 'Unlocking...' : 'Unlock'}
  </button>

  {#if attempts >= 3}
    <button
      class="text-[var(--color-primary)] text-sm underline min-h-[44px]"
      onclick={onForgotPassword}
    >
      I forgot my password — use recovery phrase
    </button>
  {/if}
</div>
```

### Inactivity Lock

```rust
// src-tauri/src/commands/profile.rs

/// Check for inactivity timeout — called periodically from frontend
#[tauri::command]
pub fn check_inactivity(state: State<'_, AppState>) -> bool {
    if state.check_timeout() {
        state.lock();
        true  // Profile was locked
    } else {
        false
    }
}

/// Update last activity timestamp — called on user interaction
#[tauri::command]
pub fn update_activity(state: State<'_, AppState>) {
    state.update_activity();
}
```

---

## [7] Recovery Phrase Display

**E-SC + E-UX:** The recovery phrase is shown ONCE at profile creation. It is never stored in the UI, never written to disk by the frontend, and never retrievable after the user navigates away.

The `RecoveryPhraseDisplay.svelte` component (Section 4) handles this. The 12 words come from the `createProfile` IPC response, displayed in a grid, and the user must check "I have written down my recovery phrase" before proceeding.

### Recovery Flow (forgot password)

```svelte
<!-- src/lib/components/profile/RecoverProfile.svelte -->
<script lang="ts">
  import { recoverProfile } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';

  interface Props {
    profile: ProfileInfo;
    onRecovered: () => void;
    onBack: () => void;
  }
  let { profile, onRecovered, onBack }: Props = $props();

  let words = $state(Array(12).fill(''));
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state('');
  let loading = $state(false);

  async function handleRecover() {
    const phrase = words.map(w => w.trim().toLowerCase()).join(' ');
    if (newPassword !== confirmPassword) {
      error = 'Passwords do not match';
      return;
    }
    if (newPassword.length < 6) {
      error = 'Password must be at least 6 characters';
      return;
    }

    loading = true;
    error = '';
    try {
      await recoverProfile(profile.id, phrase, newPassword);
      onRecovered();
    } catch (e) {
      error = 'Recovery failed. Please check your words and try again.';
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex flex-col items-center justify-center min-h-screen px-8 gap-6 max-w-lg mx-auto">
  <button class="self-start text-stone-400 hover:text-stone-600 min-h-[44px]" onclick={onBack}>
    &larr; Back
  </button>

  <h2 class="text-2xl font-bold text-stone-800">Recover {profile.name}'s profile</h2>
  <p class="text-stone-600 text-center">Enter your 12 recovery words in order.</p>

  <div class="grid grid-cols-3 gap-2 w-full">
    {#each words as _, i}
      <label class="flex items-center gap-1">
        <span class="text-stone-400 text-sm w-5 text-right">{i + 1}.</span>
        <input
          type="text"
          bind:value={words[i]}
          class="w-full px-2 py-2 rounded border border-stone-300 font-mono min-h-[44px]
                 focus:border-[var(--color-primary)] focus:outline-none"
          autocomplete="off"
          autocapitalize="off"
        />
      </label>
    {/each}
  </div>

  <label class="w-full flex flex-col gap-1 mt-4">
    <span class="text-stone-600 text-sm font-medium">New password</span>
    <input type="password" bind:value={newPassword}
           class="px-4 py-3 rounded-lg border border-stone-300 min-h-[44px]" />
  </label>
  <label class="w-full flex flex-col gap-1">
    <span class="text-stone-600 text-sm font-medium">Confirm new password</span>
    <input type="password" bind:value={confirmPassword}
           class="px-4 py-3 rounded-lg border border-stone-300 min-h-[44px]" />
  </label>

  {#if error}
    <p class="text-red-600 text-sm">{error}</p>
  {/if}

  <button
    class="w-full px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-lg
           font-medium hover:brightness-110 disabled:opacity-50 min-h-[44px]"
    onclick={handleRecover}
    disabled={loading}
  >
    {loading ? 'Recovering...' : 'Recover and set new password'}
  </button>
</div>
```

---

## [8] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/profile.rs

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileInfo>, String> {
    state.profile_manager.list_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_profile(
    name: String,
    password: String,
    managed_by: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProfileCreateResult, String> {
    let (info, phrase) = state.profile_manager
        .create_profile(&name, &password)
        .map_err(|e| e.to_string())?;

    // Open the newly created profile
    let session = state.profile_manager
        .open_profile(&info.id, &password)
        .map_err(|e| e.to_string())?;

    let mut active = state.active_session.lock().unwrap();
    *active = Some(session);
    state.update_activity();

    Ok(ProfileCreateResult {
        profile: info,
        recovery_phrase: phrase.words().iter().map(|w| w.to_string()).collect(),
    })
}

#[derive(Serialize)]
pub struct ProfileCreateResult {
    pub profile: ProfileInfo,
    pub recovery_phrase: Vec<String>,
}

#[tauri::command]
pub fn unlock_profile(
    profile_id: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<ProfileInfo, String> {
    let id = Uuid::parse_str(&profile_id)
        .map_err(|e| format!("Invalid profile ID: {e}"))?;

    let session = state.profile_manager
        .open_profile(&id, &password)
        .map_err(|e| e.to_string())?;

    let info = ProfileInfo {
        id: session.profile_id,
        name: session.profile_name.clone(),
        created_at: String::new(),
        managed_by: None,
        password_hint: None,
    };

    let mut active = state.active_session.lock().unwrap();
    *active = Some(session);
    state.update_activity();

    Ok(info)
}

#[tauri::command]
pub fn lock_profile(state: State<'_, AppState>) {
    state.lock();
}

#[tauri::command]
pub fn recover_profile(
    profile_id: String,
    recovery_phrase: String,
    new_password: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&profile_id)
        .map_err(|e| format!("Invalid profile ID: {e}"))?;

    // Recover using phrase
    let session = state.profile_manager
        .recover_profile(&id, &recovery_phrase)
        .map_err(|e| e.to_string())?;

    // TODO: Set new password (requires re-deriving key and updating salt/verification)

    let mut active = state.active_session.lock().unwrap();
    *active = Some(session);
    state.update_activity();

    Ok(())
}

#[tauri::command]
pub fn is_profile_active(state: State<'_, AppState>) -> bool {
    !state.is_locked()
}

#[tauri::command]
pub fn delete_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let id = Uuid::parse_str(&profile_id)
        .map_err(|e| format!("Invalid profile ID: {e}"))?;

    // Lock if deleting active profile
    let active_id = state.active_session.lock().unwrap()
        .as_ref().map(|s| s.profile_id);
    if active_id == Some(id) {
        state.lock();
    }

    state.profile_manager.delete_profile(&id)
        .map_err(|e| e.to_string())
}
```

### Frontend API

```typescript
// src/lib/api/profile.ts
import { invoke } from '@tauri-apps/api/core';
import type { ProfileInfo, ProfileCreateResult } from '$lib/types/profile';

export async function listProfiles(): Promise<ProfileInfo[]> {
  return invoke<ProfileInfo[]>('list_profiles');
}

export async function createProfile(
  name: string,
  password: string,
  managedBy: string | null,
): Promise<ProfileCreateResult> {
  return invoke<ProfileCreateResult>('create_profile', {
    name, password, managedBy,
  });
}

export async function unlockProfile(profileId: string, password: string): Promise<ProfileInfo> {
  return invoke<ProfileInfo>('unlock_profile', { profileId, password });
}

export async function lockProfile(): Promise<void> {
  return invoke('lock_profile');
}

export async function recoverProfile(
  profileId: string,
  recoveryPhrase: string,
  newPassword: string,
): Promise<void> {
  return invoke('recover_profile', { profileId, recoveryPhrase, newPassword });
}

export async function isProfileActive(): Promise<boolean> {
  return invoke<boolean>('is_profile_active');
}

export async function deleteProfile(profileId: string): Promise<void> {
  return invoke('delete_profile', { profileId });
}
```

---

## [9] Svelte Components

### Profile Guard (Route Protection)

```svelte
<!-- src/lib/components/profile/ProfileGuard.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { isProfileActive, listProfiles } from '$lib/api/profile';
  import type { ProfileInfo } from '$lib/types/profile';
  import TrustScreen from './TrustScreen.svelte';
  import CreateProfile from './CreateProfile.svelte';
  import ProfilePicker from './ProfilePicker.svelte';
  import UnlockProfile from './UnlockProfile.svelte';
  import RecoveryPhraseDisplay from './RecoveryPhraseDisplay.svelte';
  import RecoverProfile from './RecoverProfile.svelte';

  let screen = $state<'loading' | 'trust' | 'create' | 'picker' | 'unlock' | 'recovery_display' | 'recover' | 'app'>('loading');
  let profiles = $state<ProfileInfo[]>([]);
  let selectedProfile = $state<ProfileInfo | null>(null);
  let recoveryWords = $state<string[]>([]);

  onMount(async () => {
    const active = await isProfileActive();
    if (active) {
      screen = 'app';
      return;
    }

    profiles = await listProfiles();
    if (profiles.length === 0) {
      screen = 'trust';
    } else if (profiles.length === 1) {
      selectedProfile = profiles[0];
      screen = 'unlock';
    } else {
      screen = 'picker';
    }
  });

  // Periodic inactivity check
  let interval: ReturnType<typeof setInterval>;
  onMount(() => {
    interval = setInterval(async () => {
      if (screen === 'app') {
        const locked = await invoke<boolean>('check_inactivity');
        if (locked) {
          screen = 'unlock';
        }
      }
    }, 30_000);  // Check every 30 seconds
    return () => clearInterval(interval);
  });
</script>

{#if screen === 'loading'}
  <div class="flex items-center justify-center min-h-screen">
    <p class="text-stone-400">Loading...</p>
  </div>
{:else if screen === 'trust'}
  <TrustScreen onContinue={() => screen = 'create'} />
{:else if screen === 'create'}
  <CreateProfile
    onCreated={(result) => {
      recoveryWords = result.recovery_phrase;
      selectedProfile = result.profile;
      screen = 'recovery_display';
    }}
    onError={(err) => console.error(err)}
  />
{:else if screen === 'recovery_display'}
  <RecoveryPhraseDisplay
    words={recoveryWords}
    onConfirmed={() => { recoveryWords = []; screen = 'app'; }}
  />
{:else if screen === 'picker'}
  <ProfilePicker
    {profiles}
    onSelect={(p) => { selectedProfile = p; screen = 'unlock'; }}
    onCreateNew={() => screen = 'create'}
  />
{:else if screen === 'unlock' && selectedProfile}
  <UnlockProfile
    profile={selectedProfile}
    onUnlocked={() => screen = 'app'}
    onBack={() => { selectedProfile = null; screen = 'picker'; }}
    onForgotPassword={() => screen = 'recover'}
  />
{:else if screen === 'recover' && selectedProfile}
  <RecoverProfile
    profile={selectedProfile}
    onRecovered={() => screen = 'app'}
    onBack={() => screen = 'unlock'}
  />
{:else if screen === 'app'}
  <slot />
{/if}
```

---

## [10] Error Handling

User-facing error messages follow the calm design language:

| Error | User sees |
|-------|-----------|
| Wrong password | "That password didn't work. Please try again." |
| Wrong password 3+ times | "Wrong password. If you forgot it, you can use your recovery phrase." |
| Invalid recovery phrase | "Those words don't seem right. Please check your recovery phrase and try again." |
| Profile creation failed (disk space) | "Couldn't create your profile. Please check that your computer has enough free space." |
| Profile corrupted | "There's a problem with this profile. If you have your recovery phrase, you can try to recover it." |

---

## [11] Security

| Concern | Mitigation |
|---------|-----------|
| Password in memory | Password passed to PBKDF2 immediately, then dropped. Never stored in frontend state beyond the input field. |
| Recovery phrase in memory | Displayed once, then Svelte state cleared (`recoveryWords = []`). Zeroize in Rust before returning. |
| Brute force on unlock | PBKDF2 600K iterations (~0.5s per attempt). No rate limiting needed at this speed. |
| Inactivity exposure | Auto-lock after 15 minutes (configurable). Key zeroed on lock. |
| Profile names visible | Accepted design decision (David approved). Names are not medical data. |
| Delete confirmation | Profile deletion requires typing the profile name (future enhancement). Currently: single confirmation. |

---

## [12] Testing

### Acceptance Criteria

| # | Test | Expected |
|---|------|----------|
| T-01 | First launch shows trust screen | Trust screen visible, no profile picker |
| T-02 | Trust → Create → Recovery → App flow | Full first-launch walkthrough completes |
| T-03 | Create profile stores on disk | profiles.json updated, profile directory exists |
| T-04 | Recovery phrase is 12 words | 12 valid BIP39 English words displayed |
| T-05 | Recovery phrase cleared from UI after confirm | Svelte state is empty array |
| T-06 | Correct password unlocks profile | ProfileSession created, app screen shown |
| T-07 | Wrong password rejected | Error message, session NOT created |
| T-08 | 3 wrong attempts shows recovery option | "Use recovery phrase" button visible |
| T-09 | Profile picker shows all profiles | Multiple profiles listed with names |
| T-10 | Caregiver attribution shown | "Managed by Sophie" visible on card |
| T-11 | Inactivity lock after timeout | Profile locks, unlock screen shown |
| T-12 | Lock button works | Manual lock from settings, back to unlock screen |
| T-13 | Recovery phrase unlocks profile | New password set, profile accessible |
| T-14 | Delete profile removes all data | Profile directory and profiles.json updated |
| T-15 | No screen accessible without session | Route guard redirects to unlock |
| T-16 | Password input autofocused | Unlock screen focuses password field |

---

## [13] Performance

| Metric | Target |
|--------|--------|
| Profile list load | < 50ms |
| Profile unlock (PBKDF2) | 200-800ms (intentionally slow) |
| Lock transition | < 50ms |
| Screen navigation | < 100ms |

---

## [14] Open Questions

| # | Question | Status |
|---|---------|--------|
| OQ-01 | Should we allow password change from settings? | Yes, Phase 1. Requires re-deriving key and updating verification file. |
| OQ-02 | Profile photo / avatar selection? | Deferred to Phase 2. Use initial letter for now. |
| OQ-03 | Biometric unlock (Windows Hello, Touch ID)? | Deferred to Phase 2. Password-only for Phase 1. |
