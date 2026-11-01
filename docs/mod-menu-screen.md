# Mod menu screen (proposal)

Status: design for review, no code yet. Builds on API 63 (`version`, `authors` on `Installed`).

## Goal
A two-pane mod list in the style of Forge's ModMenu, built from vanilla's widgets and fonts (a game
screen stays vanilla's: [design-system.md](design-system.md)). Toggle on/off and show the version for now.

- Left: a search box (`ui::EditBox`, as `settings_search`), an A-Z / Z-A sort button, the list: icon slot,
  title, version, first author. Scrolls with the wheel.
- Right: the selected mod's title, `Version`, `by <authors>`, `Mod ID`, description, and the On/Off button
  (`Act::ToggleMod`). Native mods with settings keep the Settings button (`Act::ModSettings`).
- Bottom: Done. Rows keep the existing states: Not built, Stopped, Dev (always on).

## Where it lives
`Page::ModMenu` in `options_ui.rs`, reached from a new "Mod menu style" toggle in Horizon extras, **off by
default**; off, Options > Mods is unchanged. It reads `Settings::mods.installed` and writes through the
same `ModList::toggle`, so `horizon-mods.txt` is untouched.

## Not in this PR
Icons (needs a texture per mod: a later PR with `icon` on `Installed`), Configure/Website/Issues buttons,
a hook for mods to own screens (#5543 is another feature).

## Tests
Pure functions with independent expected values: filter by query, sort order, selection after toggle and
after a filter hides the selected row. Drawing is checked by `horizon-ctl screenshot` on a real game.
