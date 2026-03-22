# Design System -- Neubrutalism

Claude Sentinel uses a strict **neubrutalism** design language across both the desktop app (Tauri v2 + React) and the terminal TUI (ratatui). The system is intentionally constrained: pure black and white, no color, no gradients, no rounded corners.

## Principles

1. **Binary palette** -- `#000000` and `#ffffff` only. No grays in structural elements. The single exception is `#f5f5f5` for hover states on list items and table rows.
2. **Solid borders** -- 2-3px solid black borders on all interactive elements. No hairline or dotted borders.
3. **Offset shadows** -- Hard-edge box shadows (`4px 4px 0px #000`) replace soft drop shadows. Shadows shift on hover/active to create a physical "press" effect.
4. **Zero border-radius** -- All corners are sharp. `border-radius: 0` globally.
5. **Monospace typography** -- JetBrains Mono as the primary font. ALL-CAPS for labels, tab titles, and section headers.
6. **No transitions** -- State changes are instant. `transition: none` globally. The interface feels mechanical.
7. **No anti-aliasing** -- `-webkit-font-smoothing: none` for raw, terminal-like rendering.

## CSS Custom Properties

```css
:root {
  --black: #000000;
  --white: #ffffff;
  --border: 2px solid #000;
  --border-thick: 3px solid #000;
  --shadow: 4px 4px 0px #000;
  --shadow-sm: 2px 2px 0px #000;
  --shadow-inset: inset 2px 2px 0px #000;
  --font-mono: "JetBrains Mono", "Courier New", "Courier", monospace;
  --radius: 0;
  --transition: none;
}
```

## Typography

| Element | Size | Weight | Transform |
|---------|------|--------|-----------|
| h1 | 20px | 900 | uppercase, letter-spacing 0.05em |
| h2 | 16px | 900 | uppercase |
| h3 | 14px | 900 | uppercase |
| `.label` | 11px | 700 | uppercase, letter-spacing 0.08em |
| Body | 13px | normal | none |
| Line height | 1.5 | -- | -- |

## Component Catalog

### Tab Bar

Black background, white text, ALL-CAPS labels. Active tab inverts to white background with black text and a 3px white bottom border that visually "opens" into the content area.

```
+--[ SENTINEL ]--[ PROFILES ]--[ SESSIONS ]--[ AUTO-SWITCH ]--[ STATS ]--+
```

Tab item borders: `1px solid #333` between tabs (the only non-black/white value, used for subtle separation within the black bar).

### Cards

3px solid border, 4px/4px offset shadow. Title has a 2px bottom border separator. The `.card.active` variant inverts to black background with white text and a white title border.

```
+---------------------------+
| CARD TITLE                |
|---------------------------|
| Content here              |
+---------------------------+  <- 4px 4px shadow offset
```

### Buttons

Three variants with a tactile press illusion:

| Class | Default | Hover | Active (pressed) |
|-------|---------|-------|-------------------|
| `.btn` | White bg, black border, 2px shadow | Shadow 3px, translate -1px | Shadow none, translate +2px, inverts to black bg |
| `.btn-primary` | Black bg, white text | Lightens to `#222` | Inverts to white bg, black text |
| `.btn-sm` | Same as `.btn`, smaller padding (4px 10px) and 11px font | Same | Same |

### Inputs

Full-width, 3px border, 2px offset shadow. Focus state upgrades shadow to the standard 4px shadow. No outline or glow. Monospace font at 13px.

### Select Dropdowns

Same border treatment as inputs. 12px bold ALL-CAPS text. `appearance: none` to remove native browser chrome.

### Lists

Border-separated items (shared border between items, no gap). Bottom border only on last item. Hover highlights with `#f5f5f5`. Selected items invert to black background with white text. The `.badge` component uses a 1px border in `currentColor` for inline status labels.

### Quota Bar

A 16px-tall bordered bar with fill states:

| State | CSS Class | Style |
|-------|-----------|-------|
| Normal | (default) | Solid black fill |
| Warning | `.warn` | 45-degree black/white diagonal stripes via `repeating-linear-gradient` |
| Danger | `.danger` | Solid black fill |

All three use `transition: none`.

### Tables

Black header row with white ALL-CAPS text (11px, 700 weight, 0.08em letter-spacing). 3px bottom border on header, 2px on data rows. Rows highlight on hover with `#f5f5f5`. Full-width, collapsed borders.

### Modals

Centered overlay with `rgba(0,0,0,0.5)` backdrop. Modal box: 3px border, 8px/8px offset shadow (larger than standard for depth). Title: 15px, 900 weight, ALL-CAPS, with a 3px bottom border. Action bar at bottom separated by a 2px top border.

### Status Bar

Fixed at bottom. Black background, white ALL-CAPS text, 11px/700 weight, 0.06em letter-spacing. Left side: active profile:session. Right side: daemon status dot + timer count.

The status dot is an 8px square: filled with `currentColor` when on, transparent background when off (`.status-dot.off`).

### Scrollbar

Custom WebKit scrollbar: 8px wide/tall, white track with 1px left border, black thumb. Thumb hover lightens to `#333`.

## Layout Structure (Desktop App)

```
+--------------------------------------------------------------+
| TAB BAR (black bg)         SENTINEL | Profiles | Sessions ...  |
+--------------------------------------------------------------+
|                                                              |
|  TAB CONTENT (flex: 1, overflow hidden)                      |
|                                                              |
|  Profiles: split layout (260px sidebar | flex detail)        |
|  Sessions: card grid                                         |
|  Auto-Switch: configuration panel                            |
|  Stats: statistics dashboard                                 |
|                                                              |
+--------------------------------------------------------------+
| STATUS BAR (black bg)  Active: work:backend     Daemon: On   |
+--------------------------------------------------------------+
```

The app shell is a flex column: tab bar (fixed) + content (flex: 1) + status bar (fixed). Content overflow is hidden; individual panes scroll internally.

### Two-Column Split

The `.split` layout uses flexbox: a 260px fixed-width `.split-left` sidebar with a 3px right border and a flexible `.split-right` detail pane with 16px padding. Both columns scroll independently.

## TUI Design (ratatui)

The terminal TUI mirrors the desktop aesthetic within ratatui constraints:

- **Tab bar**: `Tabs` widget with `BOLD` highlight. White on black default, black on white for active tab. Block border with title `CLAUDE SENTINEL`.
- **Active profile marker**: `>>` prefix and bold style on active items.
- **Borders**: `Borders::ALL` on every panel block.
- **Colors**: White text on default background. `Color::DarkGray` for help text and inactive elements. `Color::Black` on `Color::White` for highlights.
- **Footer**: Two-column layout -- active profile:session on the left, keybinding help on the right in dark gray.

### TUI Tab Content

| Tab | Widget | Layout |
|-----|--------|--------|
| PROFILES | Horizontal split: `List` (40%) + `Paragraph` detail (60%) | `ListState` for selection |
| SESSIONS | `List` with active marker | Single column |
| AUTO-SWITCH | `Paragraph` with scheduler entries | Rate-limit timers and countdown |
| HISTORY | `List` of last 30 switch events | Single column, formatted timestamps |

### `cst top` Dashboard Layout

```
[ Header: 3 lines ]  Spinner + CST TOP | ACTIVE: profile:session | DAEMON ON/OFF
[ Body: flex ]        Table: PROFILE SESSION AUTH IN OUT RATE_LIMITS COST LAST_USED
[ Bottom: 5 lines ]   Horizontal split 50/50: QUOTA TIMERS | RECENT SWITCHES
[ Footer: 1 line ]    "q quit  r refresh  (refreshes every 1s)"
```

The header includes a braille spinner character (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) that rotates on each tick as a liveness indicator. Active row in the table is bolded with a `>>` prefix.

## System Tray (Tauri)

Menu items:
- **Active: {profile}:{session}** -- disabled informational item
- Separator
- **Open Sentinel** -- shows/focuses main window
- Separator
- **Quit** -- exits the app

Left-click on the tray icon toggles window visibility. The menu opens on right-click (macOS) or left-click context menu (Linux/Windows).

## Tauri IPC Commands

The React frontend communicates with `cst-core` through Tauri's `invoke` handler:

| Command | Module | Purpose |
|---------|--------|---------|
| `list_profiles` | profiles | Get all profiles |
| `get_active` | profiles | Current profile:session |
| `switch_profile` | profiles | Activate a profile |
| `create_profile` | profiles | Create new profile |
| `delete_profile` | profiles | Delete profile |
| `list_sessions` | sessions | Sessions for a profile |
| `create_session` | sessions | Create session |
| `delete_session` | sessions | Delete session |
| `daemon_status` | daemon | Is daemon running? |
| `daemon_start` | daemon | Start daemon |
| `daemon_stop` | daemon | Stop daemon |
| `get_switch_log` | daemon | Recent switch events |
| `get_scheduler_state` | daemon | Rate-limit timer state |
| `get_stats` | stats | Usage statistics |

The frontend auto-refreshes profile and daemon state every 30 seconds.

## Spacing Scale

| Name | Value |
|------|-------|
| xs | 4px |
| sm | 8px |
| md | 12-16px |
| lg | 20-24px |

Margins and paddings use multiples of 2px. No sub-pixel values.

## File Reference

| File | Purpose |
|------|---------|
| `apps/desktop/src/styles/neubrutalism.css` | Complete CSS design system |
| `apps/desktop/src/App.tsx` | Root component: tab navigation, status bar |
| `apps/desktop/src/components/ProfileManager.tsx` | Profile list + detail split view |
| `apps/desktop/src/components/SessionGrid.tsx` | Session card grid |
| `apps/desktop/src/components/AutoSwitchConfig.tsx` | Auto-switch configuration panel |
| `apps/desktop/src/components/StatsPanel.tsx` | Usage statistics dashboard |
| `apps/desktop/src/store/profiles.ts` | Zustand store for profile state |
| `apps/desktop/src/store/daemon.ts` | Zustand store for daemon state |
| `apps/desktop/src-tauri/src/tray.rs` | System tray setup and event handling |
| `crates/cst-cli/src/tui/view.rs` | Terminal TUI rendering (ratatui) |
| `crates/cst-cli/src/tui/model.rs` | Terminal TUI state model |
| `crates/cst-cli/src/commands/top.rs` | `cst top` live dashboard |
