# Coheara — Accessibility Verification Checklist

> **Target:** WCAG 2.1 AAA | **Persona:** Mamadou (elderly, limited vision, French-speaking)

## Pre-Verification Setup

- [ ] Enable TalkBack (Android) or VoiceOver (iOS)
- [ ] Set system font scale to 150% (simplified layout trigger)
- [ ] Enable high contrast mode
- [ ] Enable reduce motion
- [ ] Test with both portrait and landscape orientations

## Screen-by-Screen Verification

### Home Screen
- [ ] Medications due NOW are the first thing announced by screen reader
- [ ] Touch targets >= 48dp (64dp in simplified layout)
- [ ] Medication names readable at 150% font scale
- [ ] Alert cards have proper ARIA labels
- [ ] Color is never the only indicator (icons + text for severity)

### Medications Tab
- [ ] Each medication has: name, dose, frequency, time group
- [ ] Active/inactive status conveyed by text (not just color)
- [ ] List items focusable individually by screen reader
- [ ] "Morning" / "Evening" group headers announced

### Chat
- [ ] Message bubbles announced with sender + content
- [ ] Input field accessible and labeled
- [ ] Send button has accessible name
- [ ] Loading indicator announced

### Journal
- [ ] Entry form fields labeled
- [ ] Severity slider has value announcement
- [ ] Date picker accessible
- [ ] Body region selector has text labels

### Document Capture
- [ ] Camera overlay instructions announced
- [ ] Quality hints announced (lighting, alignment)
- [ ] Capture button >= 56dp
- [ ] Alternative: gallery picker available (no camera needed)

### Labs
- [ ] Values announced with units and reference ranges
- [ ] Abnormal values identified by text (not just red color)
- [ ] Trend arrows have text alternatives

### Pairing
- [ ] QR code has text instruction fallback
- [ ] Approval/denial buttons clearly labeled
- [ ] Connection status announced

## Interaction Patterns

- [ ] All interactive elements reachable via keyboard/switch control
- [ ] No gesture-only interactions (always a tap alternative)
- [ ] Focus order follows visual layout (top-to-bottom, left-to-right)
- [ ] No content behind modals is focusable
- [ ] Timeout warnings are announced

## Visual Standards

- [ ] Body text contrast ratio >= 7:1 (WCAG AAA)
- [ ] Interactive element contrast >= 4.5:1
- [ ] Focus indicators visible and >= 2px
- [ ] No text over images without background
- [ ] Icons have text labels or sr-only descriptions

## Motion & Animation

- [ ] All animations respect `prefers-reduced-motion`
- [ ] No flashing content (< 3 flashes per second)
- [ ] Loading spinners have text alternatives
- [ ] Page transitions are instant when reduce-motion is on

## Language

- [ ] All UI text available in French (Mamadou's language)
- [ ] Medical terms have plain-language alternatives
- [ ] Error messages are descriptive (not just "Error occurred")
- [ ] Screen reader pronunciations verified for medical terms

## Device-Specific

### Android
- [ ] TalkBack navigation works for all screens
- [ ] Switch Access usable for all interactions
- [ ] System font scale respected (no hardcoded px)
- [ ] Accessibility Scanner (Google) reports 0 critical issues

### iOS
- [ ] VoiceOver navigation works for all screens
- [ ] Dynamic Type respected for all text
- [ ] Bold Text preference reflected
- [ ] Accessibility Inspector reports 0 critical issues
