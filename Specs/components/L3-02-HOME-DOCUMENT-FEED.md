# L3-02 — Home & Document Feed

<!--
=============================================================================
COMPONENT SPEC — The home dashboard. Where Marie lands after unlock.
Engineer review: E-UX (UI/UX, lead), E-RS (Rust), E-DA (Data), E-QA (QA)
This is the command center — recent documents, quick actions, health pulse.
Marie must see value within 3 seconds of unlocking her profile.
=============================================================================
-->

## Table of Contents

| Section | Offset |
|---------|--------|
| [1] Identity | `offset=20 limit=30` |
| [2] Dependencies | `offset=50 limit=20` |
| [3] Interfaces | `offset=70 limit=65` |
| [4] Home Screen Layout | `offset=135 limit=80` |
| [5] Document Feed | `offset=215 limit=75` |
| [6] Quick Actions | `offset=290 limit=45` |
| [7] Onboarding Milestones | `offset=335 limit=55` |
| [8] Coherence Observations Banner | `offset=390 limit=50` |
| [9] Tauri Commands (IPC) | `offset=440 limit=55` |
| [10] Svelte Components | `offset=495 limit=130` |
| [11] Error Handling | `offset=625 limit=25` |
| [12] Security | `offset=650 limit=20` |
| [13] Testing | `offset=670 limit=50` |
| [14] Performance | `offset=720 limit=15` |
| [15] Open Questions | `offset=735 limit=10` |

---

## [1] Identity

**What:** The home dashboard — the first screen after profile unlock. Shows recent documents in a chronological feed, quick action buttons (load document, ask question, record symptom), onboarding milestones for new users, and a coherence observations summary. Also serves as the main navigation hub via bottom tab bar.

**After this session:**
- Profile unlock → home screen with recent documents feed
- Document feed shows cards: document type, date, professional, status (pending review / confirmed)
- Quick action buttons for the 3 most common tasks
- Empty state for new profiles with onboarding milestones
- Coherence observations banner (non-alarming, calm framing)
- Critical alert banner for lab critical values (persistent until dismissed)
- Bottom tab bar navigation (Home, Chat, Journal, Medications, More)
- Pull-to-refresh / manual refresh for feed
- Document count and last activity shown

**Estimated complexity:** Medium
**Source:** Tech Spec v1.1 Section 9.1, 9.2, 9.3 (Screen Map, Navigation, First-Launch)

---

## [2] Dependencies

**Incoming:**
- L0-03 (encryption — ProfileSession for decrypting document metadata)
- L1-04 (storage pipeline — document records in SQLite, entity counts)
- L2-03 (coherence engine — observation summaries for banner)
- L3-01 (profile management — ProfileGuard ensures active session)

**Outgoing:**
- L3-03 (chat interface — navigated to from quick action or tab)
- L3-04 (review screen — navigated to from pending document card)
- L4-01 (symptom journal — navigated to from quick action or tab)
- L1-01 (document import — triggered from quick action button)

**No new Cargo.toml dependencies.** Uses existing repository traits and Tauri state.

---

## [3] Interfaces

### Backend Types

```rust
// src-tauri/src/home.rs

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A document card for the home feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentCard {
    pub id: Uuid,
    pub document_type: String,       // "Prescription", "Lab Report", "Referral", etc.
    pub source_filename: String,     // Original filename for display
    pub professional_name: Option<String>,
    pub professional_specialty: Option<String>,
    pub document_date: Option<NaiveDate>,
    pub imported_at: NaiveDateTime,
    pub status: DocumentStatus,
    pub entity_summary: EntitySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentStatus {
    PendingReview,   // Stored but patient hasn't confirmed extraction
    Confirmed,       // Patient reviewed and confirmed
    Corrected,       // Patient reviewed and made corrections
}

/// Counts of entities extracted from a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySummary {
    pub medications: u32,
    pub lab_results: u32,
    pub diagnoses: u32,
    pub allergies: u32,
    pub procedures: u32,
    pub referrals: u32,
}

/// Aggregated profile stats for the home header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStats {
    pub total_documents: u32,
    pub documents_pending_review: u32,
    pub total_medications: u32,
    pub total_lab_results: u32,
    pub last_document_date: Option<NaiveDateTime>,
    pub extraction_accuracy: Option<f64>,  // from profile_trust table
}

/// Onboarding milestone tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingProgress {
    pub first_document_loaded: bool,
    pub first_document_reviewed: bool,
    pub first_question_asked: bool,
    pub three_documents_loaded: bool,
    pub first_symptom_recorded: bool,
}

/// Coherence observation for the banner
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceAlert {
    pub id: Uuid,
    pub alert_type: CoherenceAlertType,
    pub severity: AlertSeverity,
    pub summary: String,           // Patient-facing calm text
    pub entity_ids: Vec<Uuid>,     // Related entity IDs for navigation
    pub detected_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoherenceAlertType {
    Conflict,
    Duplicate,
    Gap,
    Drift,
    Temporal,
    Allergy,
    Dose,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,       // Minor observation
    Warning,    // Worth discussing with doctor
    Critical,   // Requires prompt attention (critical lab, allergy match)
}

/// Home screen data — single fetch for all home content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeData {
    pub stats: ProfileStats,
    pub recent_documents: Vec<DocumentCard>,
    pub onboarding: OnboardingProgress,
    pub active_alerts: Vec<CoherenceAlert>,
    pub critical_alerts: Vec<CoherenceAlert>,  // Separate — always shown
}
```

### Frontend Types

```typescript
// src/lib/types/home.ts

export interface DocumentCard {
  id: string;
  document_type: string;
  source_filename: string;
  professional_name: string | null;
  professional_specialty: string | null;
  document_date: string | null;
  imported_at: string;
  status: 'PendingReview' | 'Confirmed' | 'Corrected';
  entity_summary: EntitySummary;
}

export interface EntitySummary {
  medications: number;
  lab_results: number;
  diagnoses: number;
  allergies: number;
  procedures: number;
  referrals: number;
}

export interface ProfileStats {
  total_documents: number;
  documents_pending_review: number;
  total_medications: number;
  total_lab_results: number;
  last_document_date: string | null;
  extraction_accuracy: number | null;
}

export interface OnboardingProgress {
  first_document_loaded: boolean;
  first_document_reviewed: boolean;
  first_question_asked: boolean;
  three_documents_loaded: boolean;
  first_symptom_recorded: boolean;
}

export interface CoherenceAlert {
  id: string;
  alert_type: string;
  severity: 'Info' | 'Warning' | 'Critical';
  summary: string;
  entity_ids: string[];
  detected_at: string;
}

export interface HomeData {
  stats: ProfileStats;
  recent_documents: DocumentCard[];
  onboarding: OnboardingProgress;
  active_alerts: CoherenceAlert[];
  critical_alerts: CoherenceAlert[];
}
```

---

## [4] Home Screen Layout

**E-UX lead:** The home screen is a vertical scroll with distinct zones. Top to bottom: header with greeting → critical alert banner (if any) → quick actions → document feed → onboarding milestones (if new user). The design language is calm — warm colors, rounded corners, generous spacing.

### Layout Zones

```
┌────────────────────────────────────────────┐
│  HEADER                                    │
│  "Welcome back, Marie"                     │
│  3 documents · Last updated 2 hours ago    │
├────────────────────────────────────────────┤
│  CRITICAL ALERT BANNER (conditional)       │
│  "Your lab report flags [test] as needing  │
│   prompt attention."            [Details]  │
├────────────────────────────────────────────┤
│  QUICK ACTIONS (3 buttons, horizontal)     │
│  [ + Document ] [ Ask Question ] [ Journal]│
├────────────────────────────────────────────┤
│  COHERENCE OBSERVATIONS (conditional)      │
│  "1 observation about your medications"    │
│                                 [View]     │
├────────────────────────────────────────────┤
│  RECENT DOCUMENTS FEED                     │
│  ┌──────────────────────────────────────┐  │
│  │ Prescription · Dr. Chen · Jan 15     │  │
│  │ 2 medications · Confirmed ✓          │  │
│  └──────────────────────────────────────┘  │
│  ┌──────────────────────────────────────┐  │
│  │ Lab Report · Lab Central · Jan 10    │  │
│  │ 5 results · Pending review ●         │  │
│  └──────────────────────────────────────┘  │
│  ┌──────────────────────────────────────┐  │
│  │ Referral · Dr. Chen · Jan 8          │  │
│  │ 1 referral · Confirmed ✓             │  │
│  └──────────────────────────────────────┘  │
├────────────────────────────────────────────┤
│  ONBOARDING MILESTONES (new users only)    │
│  ✓ Load your first document                │
│  ○ Review your first extraction            │
│  ○ Ask your first question                 │
│  ○ Load 3 documents                        │
│  ○ Record your first symptom               │
├────────────────────────────────────────────┤
│                                            │
│  [ Home ] [ Chat ] [ Journal ] [ Meds ] […]│
└────────────────────────────────────────────┘
```

### Header Rules

- Greeting uses profile name: "Welcome back, {name}"
- If caregiver profile (managed_by set): "Welcome back, {managed_by} — managing {name}'s profile"
- Stats line: "{n} documents · Last updated {relative_time}"
- If no documents: "No documents yet — let's get started!"

### Empty State

When profile has zero documents:
- Hide document feed and coherence sections entirely
- Show large centered illustration placeholder (simple SVG)
- Show: "Load your first medical document to get started"
- Prominent document import button
- Onboarding milestones shown below

---

## [5] Document Feed

### Feed Query

Documents ordered by `imported_at DESC`. Default limit: 20 most recent. Load more on scroll.

```rust
/// Fetches recent documents with entity counts for the home feed
pub fn fetch_recent_documents(
    conn: &rusqlite::Connection,
    session: &ProfileSession,
    limit: u32,
    offset: u32,
) -> Result<Vec<DocumentCard>, CohearaError> {
    let rows = conn.prepare(
        "SELECT d.id, d.type, d.source_filename, d.date, d.imported_at,
                d.review_status,
                p.name AS prof_name, p.specialty AS prof_specialty
         FROM documents d
         LEFT JOIN professionals p ON d.professional_id = p.id
         ORDER BY d.imported_at DESC
         LIMIT ?1 OFFSET ?2"
    )?
    .query_map(params![limit, offset], |row| {
        // Map rows to DocumentCard
        // Decrypt encrypted fields using session.decrypt()
        // ...
    })?;

    let mut cards = Vec::new();
    for row in rows {
        let mut card: DocumentCard = row?;
        card.entity_summary = fetch_entity_counts(conn, card.id)?;
        cards.push(card);
    }
    Ok(cards)
}

/// Counts entities per document across all entity tables
fn fetch_entity_counts(
    conn: &rusqlite::Connection,
    document_id: Uuid,
) -> Result<EntitySummary, CohearaError> {
    let count_query = |table: &str| -> Result<u32, CohearaError> {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {} WHERE document_id = ?1", table),
            params![document_id],
            |row| row.get(0),
        ).map_err(CohearaError::from)
    };

    Ok(EntitySummary {
        medications: count_query("medications")?,
        lab_results: count_query("lab_results")?,
        diagnoses: count_query("diagnoses")?,
        allergies: count_query("allergies")?,
        procedures: count_query("procedures")?,
        referrals: count_query("referrals")?,
    })
}
```

### Document Card Display Rules

| Field | Display Rule |
|-------|-------------|
| Type | Capitalized document type: "Prescription", "Lab Report", "Discharge Summary" |
| Professional | "{name}" if available, else "Unknown professional" |
| Date | Document date if available, else imported_at date |
| Status badge | PendingReview: orange dot "Pending review" · Confirmed: green check "Confirmed" · Corrected: blue check "Corrected" |
| Entity summary | Only show non-zero counts: "2 medications, 5 lab results" |

### Card Tap Behavior

| Status | Tap Action |
|--------|-----------|
| PendingReview | Navigate to L3-04 Review Screen for this document |
| Confirmed | Navigate to document detail view (read-only structured Markdown) |
| Corrected | Navigate to document detail view (read-only structured Markdown) |

### Feed Refresh

- On screen focus (when tab becomes active)
- After document import completes (listen to `document-imported` event)
- Manual pull-to-refresh gesture (scroll past top)
- No auto-polling — event-driven only

---

## [6] Quick Actions

Three horizontal buttons below the header. Consistent height, equal width, subtle background.

```
┌─────────────┐ ┌─────────────┐ ┌─────────────┐
│  + Document  │ │  ? Ask       │ │  ♡ Journal   │
│  Load a file │ │  Ask a       │ │  How are you │
│              │ │  question    │ │  feeling?    │
└─────────────┘ └─────────────┘ └─────────────┘
```

| Button | Label | Sublabel | Action |
|--------|-------|----------|--------|
| Load Document | "+ Document" | "Load a file" | Trigger L1-01 import dialog (file picker or drop zone) |
| Ask Question | "? Ask" | "Ask a question" | Navigate to L3-03 Chat Interface |
| Record Symptom | "Journal" | "How are you feeling?" | Navigate to L4-01 Symptom Journal |

### Adaptive Quick Actions

When profile has no documents yet, the first button is emphasized (larger, primary color). The other two are dimmed but tappable — they show a gentle nudge: "Load a document first to get the most out of this feature."

---

## [7] Onboarding Milestones

Shown only when `onboarding.is_complete()` returns false (not all 5 milestones met).

### Milestone Definitions

```rust
impl OnboardingProgress {
    pub fn is_complete(&self) -> bool {
        self.first_document_loaded
            && self.first_document_reviewed
            && self.first_question_asked
            && self.three_documents_loaded
            && self.first_symptom_recorded
    }

    pub fn completed_count(&self) -> u32 {
        [
            self.first_document_loaded,
            self.first_document_reviewed,
            self.first_question_asked,
            self.three_documents_loaded,
            self.first_symptom_recorded,
        ]
        .iter()
        .filter(|&&v| v)
        .count() as u32
    }
}
```

### Milestone Display

| Milestone | Label | Help Text (on tap) |
|-----------|-------|-------------------|
| first_document_loaded | "Load your first document" | "Use the + Document button above to load a prescription, lab report, or any medical document." |
| first_document_reviewed | "Review your first extraction" | "After loading a document, review what Coheara extracted to make sure it's correct." |
| first_question_asked | "Ask your first question" | "Go to Chat and ask anything about your health documents — like 'What medications am I taking?'" |
| three_documents_loaded | "Load 3 documents" | "The more documents you load, the more Coheara can help you connect the dots." |
| first_symptom_recorded | "Record your first symptom" | "Use the Journal to track how you're feeling. This helps at your next doctor visit." |

### Computing Onboarding State

```rust
pub fn compute_onboarding(
    conn: &rusqlite::Connection,
) -> Result<OnboardingProgress, CohearaError> {
    let doc_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents", [], |row| row.get(0)
    )?;
    let reviewed_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE review_status != 'pending'",
        [], |row| row.get(0)
    )?;
    let question_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE role = 'patient'",
        [], |row| row.get(0)
    )?;
    let symptom_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM symptoms", [], |row| row.get(0)
    )?;

    Ok(OnboardingProgress {
        first_document_loaded: doc_count >= 1,
        first_document_reviewed: reviewed_count >= 1,
        first_question_asked: question_count >= 1,
        three_documents_loaded: doc_count >= 3,
        first_symptom_recorded: symptom_count >= 1,
    })
}
```

---

## [8] Coherence Observations Banner

### Display Rules

Two separate banner zones — critical alerts (top, persistent) and standard observations (below quick actions).

#### Critical Alert Banner

Shown when `critical_alerts` is non-empty. Cannot be scrolled past — stays pinned at top.

- Background: soft amber (not red — calm design language)
- Text: "Your lab report from {date} flags {test} as needing prompt attention. Please contact your doctor or pharmacist soon."
- Action: [Details] button → navigates to the specific lab result detail
- Dismiss: requires 2-step confirmation (see L5-01 Trust & Safety)
- Multiple critical alerts stack vertically

#### Standard Observations Banner

Shown when `active_alerts` is non-empty and there are no critical alerts occupying attention.

- Background: light blue-gray
- Text aggregated: "{n} observation(s) about your medications" or "{n} observation(s) to discuss at your next appointment"
- Tap → expands to list individual observations
- Each observation shows calm framing text from `CoherenceAlert.summary`
- Each observation has [Dismiss] → "Has your doctor addressed this?" → [Yes] / [Not yet]

### Fetching Alerts

```rust
pub fn fetch_active_alerts(
    conn: &rusqlite::Connection,
) -> Result<(Vec<CoherenceAlert>, Vec<CoherenceAlert>), CohearaError> {
    // Fetch all non-dismissed coherence observations
    let all_alerts: Vec<CoherenceAlert> = conn.prepare(
        "SELECT co.id, co.alert_type, co.severity, co.summary,
                co.entity_ids, co.detected_at
         FROM coherence_observations co
         LEFT JOIN dismissed_alerts da ON co.id = da.id
         WHERE da.id IS NULL
         ORDER BY co.severity DESC, co.detected_at DESC"
    )?
    .query_map([], |row| {
        // Map rows to CoherenceAlert
        // ...
        todo!()
    })?
    .collect::<Result<Vec<_>, _>>()?;

    let (critical, standard): (Vec<_>, Vec<_>) = all_alerts
        .into_iter()
        .partition(|a| matches!(a.severity, AlertSeverity::Critical));

    Ok((critical, standard))
}
```

---

## [9] Tauri Commands (IPC)

```rust
// src-tauri/src/commands/home.rs

use tauri::State;

/// Fetches all home screen data in a single call
#[tauri::command]
pub async fn get_home_data(
    state: State<'_, AppState>,
) -> Result<HomeData, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;

    let stats = fetch_profile_stats(&conn, session)?;
    let recent_documents = fetch_recent_documents(&conn, session, 20, 0)?;
    let onboarding = compute_onboarding(&conn)?;
    let (critical_alerts, active_alerts) = fetch_active_alerts(&conn)?;

    state.update_activity();

    Ok(HomeData {
        stats,
        recent_documents,
        onboarding,
        active_alerts,
        critical_alerts,
    })
}

/// Fetches more documents for infinite scroll
#[tauri::command]
pub async fn get_more_documents(
    state: State<'_, AppState>,
    offset: u32,
    limit: u32,
) -> Result<Vec<DocumentCard>, String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    let clamped_limit = limit.min(50); // Max 50 per page

    state.update_activity();

    fetch_recent_documents(&conn, session, clamped_limit, offset)
        .map_err(|e| e.to_string())
}

/// Dismisses a coherence observation with reason
#[tauri::command]
pub async fn dismiss_alert(
    state: State<'_, AppState>,
    alert_id: String,
    reason: String,
) -> Result<(), String> {
    let session_guard = state.active_session.lock()
        .map_err(|e| format!("Lock error: {e}"))?;
    let session = session_guard.as_ref()
        .ok_or("No active profile session")?;

    let conn = session.db_connection()?;
    let alert_uuid = Uuid::parse_str(&alert_id)
        .map_err(|e| format!("Invalid alert ID: {e}"))?;

    // Fetch the alert to check severity
    let severity = fetch_alert_severity(&conn, alert_uuid)?;

    // Critical alerts require 2-step — frontend handles the confirmation UI
    // but backend still validates
    if matches!(severity, AlertSeverity::Critical) && reason.is_empty() {
        return Err("Critical alerts require a dismissal reason".into());
    }

    conn.execute(
        "INSERT INTO dismissed_alerts (id, alert_type, entity_ids, dismissed_date, reason, dismissed_by)
         VALUES (?1, ?2, ?3, datetime('now'), ?4, 'patient')",
        params![
            Uuid::new_v4(),
            "coherence",
            serde_json::to_string(&Vec::<Uuid>::new()).unwrap(),
            reason,
        ],
    ).map_err(|e| format!("Failed to dismiss alert: {e}"))?;

    state.update_activity();

    Ok(())
}

/// Fetches profile stats for the header
fn fetch_profile_stats(
    conn: &rusqlite::Connection,
    session: &ProfileSession,
) -> Result<ProfileStats, CohearaError> {
    let total_documents: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents", [], |row| row.get(0)
    )?;
    let documents_pending: u32 = conn.query_row(
        "SELECT COUNT(*) FROM documents WHERE review_status = 'pending'",
        [], |row| row.get(0)
    )?;
    let total_medications: u32 = conn.query_row(
        "SELECT COUNT(*) FROM medications WHERE status = 'active'",
        [], |row| row.get(0)
    )?;
    let total_lab_results: u32 = conn.query_row(
        "SELECT COUNT(*) FROM lab_results", [], |row| row.get(0)
    )?;
    let last_doc_date: Option<String> = conn.query_row(
        "SELECT MAX(imported_at) FROM documents", [], |row| row.get(0)
    ).ok();
    let extraction_accuracy: Option<f64> = conn.query_row(
        "SELECT extraction_accuracy FROM profile_trust LIMIT 1",
        [], |row| row.get(0)
    ).ok();

    Ok(ProfileStats {
        total_documents,
        documents_pending_review: documents_pending,
        total_medications,
        total_lab_results,
        last_document_date: last_doc_date.and_then(|s| {
            NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()
        }),
        extraction_accuracy,
    })
}
```

### Frontend API

```typescript
// src/lib/api/home.ts
import { invoke } from '@tauri-apps/api/core';
import type { HomeData, DocumentCard } from '$lib/types/home';

export async function getHomeData(): Promise<HomeData> {
  return invoke<HomeData>('get_home_data');
}

export async function getMoreDocuments(offset: number, limit: number): Promise<DocumentCard[]> {
  return invoke<DocumentCard[]>('get_more_documents', { offset, limit });
}

export async function dismissAlert(alertId: string, reason: string): Promise<void> {
  return invoke('dismiss_alert', { alertId, reason });
}
```

---

## [10] Svelte Components

### Bottom Tab Bar

```svelte
<!-- src/lib/components/navigation/TabBar.svelte -->
<script lang="ts">
  interface Props {
    activeTab: string;
    onNavigate: (tab: string) => void;
  }
  let { activeTab, onNavigate }: Props = $props();

  const tabs = [
    { id: 'home', label: 'Home', icon: 'home' },
    { id: 'chat', label: 'Chat', icon: 'message-circle' },
    { id: 'journal', label: 'Journal', icon: 'heart' },
    { id: 'medications', label: 'Meds', icon: 'pill' },
    { id: 'more', label: 'More', icon: 'menu' },
  ];

  let showMore = $state(false);

  const moreItems = [
    { id: 'documents', label: 'Documents' },
    { id: 'timeline', label: 'Timeline' },
    { id: 'appointments', label: 'Appointments' },
    { id: 'settings', label: 'Settings' },
  ];
</script>

<nav class="fixed bottom-0 left-0 right-0 bg-white border-t border-stone-200
            flex items-center justify-around h-16 z-50">
  {#each tabs as tab}
    <button
      class="flex flex-col items-center justify-center gap-1 flex-1 h-full
             min-h-[44px] min-w-[44px]
             {activeTab === tab.id ? 'text-[var(--color-primary)]' : 'text-stone-400'}"
      onclick={() => {
        if (tab.id === 'more') {
          showMore = !showMore;
        } else {
          showMore = false;
          onNavigate(tab.id);
        }
      }}
      aria-current={activeTab === tab.id ? 'page' : undefined}
      aria-label={tab.label}
    >
      <!-- Icon placeholder — use actual icon component -->
      <span class="text-lg">{tab.icon}</span>
      <span class="text-xs">{tab.label}</span>
    </button>
  {/each}
</nav>

<!-- More menu dropdown -->
{#if showMore}
  <div class="fixed bottom-16 right-2 bg-white rounded-xl shadow-lg border border-stone-200
              p-2 z-50 min-w-[180px]"
       role="menu">
    {#each moreItems as item}
      <button
        class="w-full text-left px-4 py-3 rounded-lg hover:bg-stone-50
               text-stone-700 text-sm min-h-[44px]"
        role="menuitem"
        onclick={() => { showMore = false; onNavigate(item.id); }}
      >
        {item.label}
      </button>
    {/each}
  </div>
{/if}
```

### Home Screen

```svelte
<!-- src/lib/components/home/HomeScreen.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { getHomeData, getMoreDocuments } from '$lib/api/home';
  import { listen } from '@tauri-apps/api/event';
  import type { HomeData, DocumentCard } from '$lib/types/home';
  import CriticalAlertBanner from './CriticalAlertBanner.svelte';
  import QuickActions from './QuickActions.svelte';
  import ObservationsBanner from './ObservationsBanner.svelte';
  import DocumentFeed from './DocumentFeed.svelte';
  import OnboardingMilestones from './OnboardingMilestones.svelte';
  import EmptyState from './EmptyState.svelte';

  interface Props {
    profileName: string;
    managedBy: string | null;
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { profileName, managedBy, onNavigate }: Props = $props();

  let homeData: HomeData | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  let loadingMore = $state(false);

  async function refresh() {
    try {
      loading = true;
      error = null;
      homeData = await getHomeData();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function loadMore() {
    if (!homeData || loadingMore) return;
    loadingMore = true;
    try {
      const more = await getMoreDocuments(homeData.recent_documents.length, 20);
      homeData.recent_documents = [...homeData.recent_documents, ...more];
    } catch (e) {
      console.error('Failed to load more documents:', e);
    } finally {
      loadingMore = false;
    }
  }

  onMount(() => {
    refresh();
    // Refresh when new document is imported
    const unlisten = listen('document-imported', () => refresh());
    return () => { unlisten.then(fn => fn()); };
  });

  // Greeting text
  let greeting = $derived(
    managedBy
      ? `Welcome back, ${managedBy} — managing ${profileName}'s profile`
      : `Welcome back, ${profileName}`
  );

  // Relative time for last document
  function relativeTime(dateStr: string | null): string {
    if (!dateStr) return 'No documents yet';
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins} minute${diffMins === 1 ? '' : 's'} ago`;
    const diffHours = Math.floor(diffMins / 60);
    if (diffHours < 24) return `${diffHours} hour${diffHours === 1 ? '' : 's'} ago`;
    const diffDays = Math.floor(diffHours / 24);
    if (diffDays < 7) return `${diffDays} day${diffDays === 1 ? '' : 's'} ago`;
    return date.toLocaleDateString();
  }
</script>

<div class="flex flex-col min-h-screen pb-20 bg-stone-50">
  <!-- Header -->
  <header class="px-6 pt-6 pb-4">
    <h1 class="text-2xl font-bold text-stone-800">{greeting}</h1>
    {#if homeData}
      <p class="text-sm text-stone-500 mt-1">
        {homeData.stats.total_documents} document{homeData.stats.total_documents === 1 ? '' : 's'}
        · Last updated {relativeTime(homeData.stats.last_document_date)}
      </p>
    {/if}
  </header>

  {#if loading}
    <div class="flex items-center justify-center flex-1">
      <div class="animate-pulse text-stone-400">Loading...</div>
    </div>
  {:else if error}
    <div class="px-6 py-8 text-center">
      <p class="text-red-600 mb-4">Something went wrong: {error}</p>
      <button class="px-6 py-3 bg-stone-200 rounded-xl text-stone-700 min-h-[44px]"
              onclick={refresh}>
        Try again
      </button>
    </div>
  {:else if homeData}
    <!-- Critical alerts — always top, always visible -->
    {#if homeData.critical_alerts.length > 0}
      <CriticalAlertBanner
        alerts={homeData.critical_alerts}
        {onNavigate}
      />
    {/if}

    <!-- Quick actions -->
    <QuickActions
      hasDocuments={homeData.stats.total_documents > 0}
      {onNavigate}
    />

    <!-- Standard coherence observations -->
    {#if homeData.active_alerts.length > 0}
      <ObservationsBanner
        alerts={homeData.active_alerts}
        onDismiss={async (id, reason) => {
          const { dismissAlert } = await import('$lib/api/home');
          await dismissAlert(id, reason);
          await refresh();
        }}
      />
    {/if}

    <!-- Document feed or empty state -->
    {#if homeData.stats.total_documents === 0}
      <EmptyState {onNavigate} />
    {:else}
      <DocumentFeed
        documents={homeData.recent_documents}
        onDocumentTap={(doc) => {
          if (doc.status === 'PendingReview') {
            onNavigate('review', { documentId: doc.id });
          } else {
            onNavigate('document-detail', { documentId: doc.id });
          }
        }}
        onLoadMore={loadMore}
        {loadingMore}
      />
    {/if}

    <!-- Onboarding milestones (new users) -->
    {#if !homeData.onboarding.first_document_loaded || !homeData.onboarding.first_question_asked}
      <OnboardingMilestones
        progress={homeData.onboarding}
        {onNavigate}
      />
    {/if}
  {/if}
</div>
```

### Document Card Component

```svelte
<!-- src/lib/components/home/DocumentCardView.svelte -->
<script lang="ts">
  import type { DocumentCard } from '$lib/types/home';

  interface Props {
    card: DocumentCard;
    onTap: (card: DocumentCard) => void;
  }
  let { card, onTap }: Props = $props();

  // Format entity summary as readable string
  let entityText = $derived(() => {
    const parts: string[] = [];
    if (card.entity_summary.medications > 0)
      parts.push(`${card.entity_summary.medications} medication${card.entity_summary.medications > 1 ? 's' : ''}`);
    if (card.entity_summary.lab_results > 0)
      parts.push(`${card.entity_summary.lab_results} lab result${card.entity_summary.lab_results > 1 ? 's' : ''}`);
    if (card.entity_summary.diagnoses > 0)
      parts.push(`${card.entity_summary.diagnoses} diagnosis${card.entity_summary.diagnoses > 1 ? 'es' : ''}`);
    if (card.entity_summary.allergies > 0)
      parts.push(`${card.entity_summary.allergies} allergy alert${card.entity_summary.allergies > 1 ? 's' : ''}`);
    if (card.entity_summary.procedures > 0)
      parts.push(`${card.entity_summary.procedures} procedure${card.entity_summary.procedures > 1 ? 's' : ''}`);
    if (card.entity_summary.referrals > 0)
      parts.push(`${card.entity_summary.referrals} referral${card.entity_summary.referrals > 1 ? 's' : ''}`);
    return parts.length > 0 ? parts.join(' · ') : 'Processing...';
  });

  let statusBadge = $derived(() => {
    switch (card.status) {
      case 'PendingReview': return { text: 'Pending review', color: 'bg-amber-100 text-amber-700' };
      case 'Confirmed': return { text: 'Confirmed', color: 'bg-green-100 text-green-700' };
      case 'Corrected': return { text: 'Corrected', color: 'bg-blue-100 text-blue-700' };
    }
  });

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return '';
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short', day: 'numeric', year: 'numeric'
    });
  }
</script>

<button
  class="w-full text-left bg-white rounded-xl p-4 shadow-sm border border-stone-100
         hover:shadow-md transition-shadow min-h-[44px]"
  onclick={() => onTap(card)}
>
  <div class="flex items-start justify-between gap-3">
    <div class="flex-1 min-w-0">
      <div class="flex items-center gap-2">
        <span class="font-medium text-stone-800 truncate">{card.document_type}</span>
        <span class="text-xs px-2 py-0.5 rounded-full {statusBadge().color}">
          {statusBadge().text}
        </span>
      </div>
      <p class="text-sm text-stone-500 mt-1 truncate">
        {card.professional_name ?? 'Unknown professional'}
        {#if card.professional_specialty}
          · {card.professional_specialty}
        {/if}
      </p>
      <p class="text-xs text-stone-400 mt-1">
        {formatDate(card.document_date ?? card.imported_at)}
        · {entityText()}
      </p>
    </div>
    <!-- Chevron right indicator -->
    <span class="text-stone-300 mt-1" aria-hidden="true">&rsaquo;</span>
  </div>
</button>
```

### Quick Actions Component

```svelte
<!-- src/lib/components/home/QuickActions.svelte -->
<script lang="ts">
  interface Props {
    hasDocuments: boolean;
    onNavigate: (screen: string) => void;
  }
  let { hasDocuments, onNavigate }: Props = $props();

  const actions = [
    {
      id: 'import',
      label: '+ Document',
      sublabel: 'Load a file',
      primary: true,
    },
    {
      id: 'chat',
      label: 'Ask',
      sublabel: 'Ask a question',
      primary: false,
    },
    {
      id: 'journal',
      label: 'Journal',
      sublabel: 'How are you feeling?',
      primary: false,
    },
  ];
</script>

<div class="px-6 py-3">
  <div class="grid grid-cols-3 gap-3">
    {#each actions as action}
      <button
        class="flex flex-col items-center justify-center gap-1 p-4 rounded-xl
               min-h-[80px] transition-colors
               {!hasDocuments && action.primary
                 ? 'bg-[var(--color-primary)] text-white shadow-md'
                 : 'bg-white text-stone-700 border border-stone-200 hover:bg-stone-50'}"
        onclick={() => {
          if (action.id === 'import') onNavigate('import');
          else if (action.id === 'chat') onNavigate('chat');
          else if (action.id === 'journal') onNavigate('journal');
        }}
      >
        <span class="font-medium text-sm">{action.label}</span>
        <span class="text-xs opacity-70">{action.sublabel}</span>
      </button>
    {/each}
  </div>
</div>
```

### Empty State Component

```svelte
<!-- src/lib/components/home/EmptyState.svelte -->
<script lang="ts">
  interface Props {
    onNavigate: (screen: string) => void;
  }
  let { onNavigate }: Props = $props();
</script>

<div class="flex flex-col items-center justify-center px-8 py-12 text-center">
  <!-- Simple medical document illustration placeholder -->
  <div class="w-24 h-24 bg-stone-100 rounded-2xl flex items-center justify-center mb-6">
    <span class="text-4xl text-stone-300">&#128196;</span>
  </div>

  <h2 class="text-lg font-medium text-stone-700 mb-2">
    No documents yet
  </h2>
  <p class="text-sm text-stone-500 mb-6 max-w-[280px]">
    Load your first medical document to get started. Coheara will help you understand your health records.
  </p>

  <button
    class="px-8 py-4 bg-[var(--color-primary)] text-white rounded-xl text-base font-medium
           hover:brightness-110 focus-visible:outline focus-visible:outline-2
           focus-visible:outline-offset-2 focus-visible:outline-[var(--color-primary)]
           min-h-[44px]"
    onclick={() => onNavigate('import')}
  >
    Load your first document
  </button>
</div>
```

### Critical Alert Banner Component

```svelte
<!-- src/lib/components/home/CriticalAlertBanner.svelte -->
<script lang="ts">
  import type { CoherenceAlert } from '$lib/types/home';

  interface Props {
    alerts: CoherenceAlert[];
    onNavigate: (screen: string, params?: Record<string, string>) => void;
  }
  let { alerts, onNavigate }: Props = $props();
</script>

<div class="px-6 py-2">
  {#each alerts as alert}
    <div class="bg-amber-50 border border-amber-200 rounded-xl p-4 mb-2"
         role="alert">
      <div class="flex items-start gap-3">
        <span class="text-amber-600 mt-0.5 flex-shrink-0" aria-hidden="true">&#9888;</span>
        <div class="flex-1">
          <p class="text-sm text-amber-800">{alert.summary}</p>
          <button
            class="text-sm text-amber-700 font-medium mt-2 underline
                   min-h-[44px] min-w-[44px] -ml-1 px-1"
            onclick={() => onNavigate('lab-detail', { alertId: alert.id })}
          >
            View details
          </button>
        </div>
      </div>
    </div>
  {/each}
</div>
```

### Onboarding Milestones Component

```svelte
<!-- src/lib/components/home/OnboardingMilestones.svelte -->
<script lang="ts">
  import type { OnboardingProgress } from '$lib/types/home';

  interface Props {
    progress: OnboardingProgress;
    onNavigate: (screen: string) => void;
  }
  let { progress, onNavigate }: Props = $props();

  const milestones = [
    { key: 'first_document_loaded' as const, label: 'Load your first document', action: 'import' },
    { key: 'first_document_reviewed' as const, label: 'Review your first extraction', action: 'documents' },
    { key: 'first_question_asked' as const, label: 'Ask your first question', action: 'chat' },
    { key: 'three_documents_loaded' as const, label: 'Load 3 documents', action: 'import' },
    { key: 'first_symptom_recorded' as const, label: 'Record your first symptom', action: 'journal' },
  ];
</script>

<div class="px-6 py-4">
  <h3 class="text-sm font-medium text-stone-500 mb-3">Getting started</h3>
  <div class="flex flex-col gap-2">
    {#each milestones as milestone}
      {@const completed = progress[milestone.key]}
      <button
        class="flex items-center gap-3 text-left w-full py-2 min-h-[44px]"
        onclick={() => { if (!completed) onNavigate(milestone.action); }}
        disabled={completed}
      >
        <span class="w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0
                     {completed ? 'bg-green-500 text-white' : 'border-2 border-stone-300'}">
          {#if completed}
            <span class="text-xs">&#x2713;</span>
          {/if}
        </span>
        <span class="text-sm {completed ? 'text-stone-400 line-through' : 'text-stone-700'}">
          {milestone.label}
        </span>
      </button>
    {/each}
  </div>
</div>
```

---

## [11] Error Handling

| Error | User Message | Recovery |
|-------|-------------|----------|
| Database query fails | "Something went wrong loading your documents. Try again." | Retry button on screen |
| Session expired (timeout) | Redirected to profile unlock (ProfileGuard handles) | Re-enter password |
| Single document card fails to load | Skip that card, show remaining | Log warning |
| Alert dismiss fails | "Couldn't dismiss this observation. Please try again." | Retry on next tap |
| No database connection | "Your profile couldn't be opened." | Navigate back to profile picker |

All errors logged via `tracing::warn!` or `tracing::error!`. No sensitive data in error messages.

---

## [12] Security

- All document metadata fields decrypted via `ProfileSession.decrypt()` before display
- No raw UUIDs exposed to frontend logging
- Document content (structured Markdown) not loaded on home screen — only metadata
- Activity timestamp updated on every Tauri command (prevents false timeouts while browsing)
- Alert dismissal recorded with immutable timestamp (audit trail)

---

## [13] Testing

### Unit Tests (Rust)

| Test | What |
|------|------|
| `test_fetch_recent_documents_empty` | Returns empty vec when no documents exist |
| `test_fetch_recent_documents_ordered` | Documents returned in imported_at DESC order |
| `test_fetch_entity_counts` | Correct counts per entity table for a document |
| `test_fetch_entity_counts_zero` | Returns all zeros for document with no entities |
| `test_profile_stats` | Correct aggregated counts from all tables |
| `test_profile_stats_empty` | All zeros for fresh profile |
| `test_onboarding_progress_none` | All false for fresh profile |
| `test_onboarding_progress_partial` | Correct booleans after some milestones met |
| `test_onboarding_progress_complete` | All true after all milestones met |
| `test_fetch_active_alerts` | Excludes dismissed alerts |
| `test_fetch_active_alerts_critical_partition` | Critical alerts separated from standard |
| `test_dismiss_alert_standard` | Standard alert dismissed successfully |
| `test_dismiss_alert_critical_requires_reason` | Critical alert rejected without reason |
| `test_document_card_status_mapping` | Correct DocumentStatus from review_status string |
| `test_more_documents_pagination` | Correct offset/limit pagination |
| `test_more_documents_clamped_limit` | Limit clamped to 50 maximum |

### Frontend Tests

| Test | What |
|------|------|
| `test_home_renders_greeting` | Correct greeting with profile name |
| `test_home_renders_caregiver_greeting` | Shows "managing X's profile" for caregivers |
| `test_home_empty_state` | Shows empty state when 0 documents |
| `test_home_document_feed` | Renders document cards with correct data |
| `test_document_card_pending_navigates_review` | PendingReview card navigates to review screen |
| `test_document_card_confirmed_navigates_detail` | Confirmed card navigates to document detail |
| `test_critical_alert_banner_visible` | Critical alerts shown in persistent banner |
| `test_observations_banner_dismissable` | Standard observations can be dismissed |
| `test_onboarding_milestones_shown` | Milestones shown for new users |
| `test_onboarding_milestones_hidden` | Milestones hidden when all complete |
| `test_quick_actions_emphasized_empty` | Import button emphasized when no documents |
| `test_relative_time_formatting` | relativeTime function returns correct strings |

---

## [14] Performance

- Single `get_home_data` call on mount — no waterfall
- Entity counts computed via simple COUNT queries (fast on indexed tables)
- Document feed limited to 20 initial, lazy-load more on scroll
- No content decryption on home screen — metadata only
- Event-driven refresh, no polling

---

## [15] Open Questions

- **Q1:** Should we cache `HomeData` in frontend state to avoid re-fetch on tab switch? Trade-off: stale data vs. perceived speed. Current answer: always re-fetch, but cache briefly (5s debounce).
- **Q2:** Should onboarding milestones persist dismissal? Current answer: always show until all complete — they serve as feature discovery.
