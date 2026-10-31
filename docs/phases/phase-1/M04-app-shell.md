# M04 — Responsive app shell

**Relevant docs:** ARCHITECTURE §11 (UI). reference-notes → RiPlay (mobile-first player patterns), nuclear (UI stack conventions).

## Goal
The navigation frame every later feature plugs into. It's phone-first: bottom tabs, a mini-player and a swipeable full-screen player. On wide screens the same routes render with a side rail and a docked player bar.

## Design
- **Routing:** TanStack Router with a code-defined route tree (no codegen plugin). Routes: `/` Home, `/library`, `/search`, `/settings`. The player is an overlay sheet, not a route, so it can open over any page.
- **Layout** (one tree, switched by Tailwind breakpoints, not separate apps):
  - **< 1024 px:** content, then the mini-player bar, then bottom tabs. Safe-area insets apply on top and bottom.
  - **≥ 1024 px:** a side rail with icons and labels, the content, and a docked player bar along the bottom.
- **Player UI state** (Zustand, UI-only): `sheetOpen`. Playback state arrives from Rust in M05; until then the bar shows "Nothing playing".
  - The mini-player opens the sheet by tap or swipe up.
  - The sheet closes by dragging down (more than 120 px or a fast flick), the close button, or Escape.
  - Implemented with `motion` drag; no extra gesture library.
- **Pages:**
  - Home: a placeholder welcome.
  - Library and Search: empty states.
  - Settings: Appearance (theme picker from M03) and About (app info + storage location).
- **i18n:** i18next + react-i18next, `src/i18n/locales/en.json`. All UI strings go through `t()`. The language follows `settings.language`, with English as the fallback.
- **Icons:** Lucide.

## Out of scope
Real playback (M05), library content (M07), Android back-button handling for the sheet (with M05/M09 media integration).

## Done-check
- Vitest: tab navigation changes the page; Settings shows Appearance and About; the mini-player opens the sheet and the close button closes it.
- 📱 Pixel_4: tabs navigate; swipe up on the mini-player opens the sheet; dragging the sheet down closes it; nothing is hidden under the status or gesture bar.
- 🖥 At 1440 px the side rail and docked player bar show; at 360 px the bottom tabs show.

## Follow-ups found during verification
- Android status bar icons follow the OS light/dark mode, not the app theme, so they're invisible on the AMOLED theme while the OS is in light mode. This needs a small native hook (`WindowInsetsControllerCompat.isAppearanceLightStatusBars`) driven by the theme setting. Scheduled with M09 (OS integration).
