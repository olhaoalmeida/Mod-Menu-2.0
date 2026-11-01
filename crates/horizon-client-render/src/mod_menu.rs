//! The two-pane mod menu (Horizon extra "Mod menu", off by default): a searchable, sortable list of the
//! installed mods on the left, the picked mod's details and its On/Off switch on the right. It reads
//! `ModList::installed` and changes nothing itself: a press returns what to do (`Click`), and the
//! options screen toggles through `ModList::toggle`, as Options > Mods' rows do. Built from vanilla's
//! widgets and fonts, as Horizon's own pages are.
use crate::{
    graphics_style::STYLE,
    hud::{HudAtlas, HudBatch, rgba},
    lang,
    mod_list::{Installed, ModList},
    split_profiles::wrap,
    text::Text,
    ui,
};

/// A row of the list, in GUI pixels.
const ROW: f32 = 26.0;
const TOP: f32 = 46.0;

/// What a press did, for the options screen.
#[derive(Debug, PartialEq)]
pub enum Click {
    Nothing,
    /// Something of the page changed (a pick, the sort, a focus): play the button's click.
    Pressed,
    /// Turn installed mod `i` on or off.
    Toggle(usize),
    /// Open native mod `i`'s settings page.
    Settings(usize),
    /// Done: the page closes.
    Done,
}

/// The name alone: `Name 1.0` without its version.
pub fn name_of(mod_: &Installed) -> &str {
    let title = mod_.title.as_str();
    if mod_.version.is_empty() {
        return title;
    }
    title
        .strip_suffix(mod_.version.as_str())
        .map_or(title, str::trim_end)
}

/// Whether the mod can be switched: a dev mod is always on, and a file that can't be read has no id.
pub fn can_toggle(mod_: &Installed) -> bool {
    !(mod_.dev || mod_.id.is_empty())
}

/// The indices into `installed` that `query` leaves (a part of the name, the id or an author,
/// whatever the case), in alphabetical order of the name (or the reverse).
pub fn visible(installed: &[Installed], query: &str, descending: bool) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut shown: Vec<usize> = (0..installed.len())
        .filter(|&i| {
            let m = &installed[i];
            query.is_empty()
                || name_of(m).to_lowercase().contains(&query)
                || m.id.to_lowercase().contains(&query)
                || m.authors.iter().any(|a| a.to_lowercase().contains(&query))
        })
        .collect();
    shown.sort_by_key(|&i| (name_of(&installed[i]).to_lowercase(), i));
    if descending {
        shown.reverse();
    }
    shown
}

/// Where everything is on a screen `w` x `h`.
struct Layout {
    search: [f32; 4],
    sort: [f32; 4],
    list: [f32; 4],
    detail: [f32; 4],
    toggle: [f32; 4],
    settings: [f32; 4],
    done: [f32; 4],
    /// Rows that fit in the list.
    rows: usize,
}

impl Layout {
    fn new(w: f32, h: f32) -> Self {
        let left = ((w * 0.44).floor()).clamp(150.0, 230.0).min(w - 20.0);
        let (bottom, margin) = (h - 36.0, 10.0);
        let sort_w = 44.0;
        let right_x = margin + left + 10.0;
        let right_w = (w - right_x - margin).max(40.0);
        let button_w = if right_w >= 190.0 { 92.0 } else { right_w };
        Self {
            search: [margin, 24.0, left - sort_w - 4.0, 18.0],
            sort: [margin + left - sort_w, 24.0, sort_w, 18.0],
            list: [margin, TOP, left, (bottom - TOP).max(ROW)],
            detail: [right_x, 24.0, right_w, (bottom - 24.0).max(40.0)],
            toggle: [right_x, bottom - 20.0, button_w, 20.0],
            settings: [right_x + button_w + 4.0, bottom - 20.0, button_w, 20.0],
            done: [(w / 2.0).floor() - 100.0, h - 28.0, 200.0, 20.0],
            rows: (((bottom - TOP) / ROW).floor() as usize).max(1),
        }
    }
}

fn inside(at: (f32, f32), [x, y, w, h]: [f32; 4]) -> bool {
    ui::hovered(at, x, y, w, h)
}

pub struct ModMenu {
    pub search: ui::EditBox,
    descending: bool,
    /// The picked mod (an index into `installed`); the first one shown when none, or when the
    /// search hides it.
    selected: Option<usize>,
    /// The first row of the list shown.
    first: usize,
}

impl Default for ModMenu {
    fn default() -> Self {
        let mut search = ui::EditBox::new(0.0, 0.0, 100.0, 18.0);
        search.max_length = 32;
        search.focused = false;
        Self {
            search,
            descending: false,
            selected: None,
            first: 0,
        }
    }
}

impl ModMenu {
    /// Opened: nothing typed, nothing picked.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The picked mod, if any is shown.
    pub fn picked(&self, installed: &[Installed]) -> Option<usize> {
        let shown = visible(installed, &self.search.value, self.descending);
        self.selected
            .filter(|i| shown.contains(i))
            .or_else(|| shown.first().copied())
    }

    /// The first row, kept where the list still has it.
    fn first_row(&self, shown: usize, rows: usize) -> usize {
        self.first.min(shown.saturating_sub(rows))
    }

    /// Picks the mod `by` rows after (or before) the picked one in the list, and scrolls to it.
    fn step(&mut self, installed: &[Installed], by: isize, rows: usize) {
        let shown = visible(installed, &self.search.value, self.descending);
        let Some(now) = self
            .picked(installed)
            .and_then(|p| shown.iter().position(|&i| i == p))
        else {
            return;
        };
        let to = (now as isize + by).clamp(0, shown.len() as isize - 1) as usize;
        self.selected = Some(shown[to]);
        let first = self.first_row(shown.len(), rows);
        if to < first {
            self.first = to;
        } else if to >= first + rows {
            self.first = to + 1 - rows;
        }
    }

    pub fn draw(
        &mut self,
        mods: &ModList,
        batch: &mut HudBatch,
        atlas: &HudAtlas,
        cursor: (f32, f32),
        in_world: bool,
    ) {
        let (w, h) = (batch.width.ceil(), batch.height.ceil());
        let at = Layout::new(w, h);
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, in_world);
        STYLE.title(batch, atlas, &Text::literal("Mods"));
        let grey = rgba(160, 160, 160, 255);
        let installed = &mods.installed;
        let shown = visible(installed, &self.search.value, self.descending);
        let picked = self.picked(installed);
        // The search box and the sort.
        let [x, y, bw, bh] = at.search;
        (
            self.search.x,
            self.search.y,
            self.search.width,
            self.search.height,
        ) = (x, y, bw, bh);
        self.search
            .draw(batch, atlas, Some(&Text::literal("Search")));
        let [x, y, bw, bh] = at.sort;
        let sort = if self.descending { "Z-A" } else { "A-Z" };
        let hover = inside(cursor, at.sort);
        ui::button(
            batch,
            atlas,
            x,
            y,
            bw,
            bh,
            &Text::literal(sort),
            true,
            hover,
        );
        // The list.
        let [lx, ly, lw, lh] = at.list;
        batch.fill(atlas, lx, ly, lw, lh, rgba(0, 0, 0, 110));
        if installed.is_empty() {
            batch.text(atlas, "No mods installed", lx + 6.0, ly + 8.0, grey, true);
        } else if shown.is_empty() {
            batch.text(atlas, "No mods match", lx + 6.0, ly + 8.0, grey, true);
        }
        let first = self.first_row(shown.len(), at.rows);
        let room = (lw - 12.0).max(10.0);
        for (k, &i) in shown.iter().skip(first).take(at.rows).enumerate() {
            let m = &installed[i];
            let y = ly + k as f32 * ROW;
            let row = [lx, y, lw, ROW];
            if picked == Some(i) {
                batch.fill(atlas, lx, y, lw, ROW, rgba(255, 255, 255, 60));
            } else if inside(cursor, row) {
                batch.fill(atlas, lx, y, lw, ROW, rgba(255, 255, 255, 28));
            }
            let on = mods.is_on(i);
            let name = clip(atlas, name_of(m), room - 24.0);
            let color = if m.problem.is_some() || !on {
                grey
            } else {
                u32::MAX
            };
            batch.text(atlas, &name, lx + 6.0, y + 4.0, color, true);
            let sub = match (m.version.is_empty(), m.authors.first()) {
                (false, Some(a)) => format!("{} - {a}", m.version),
                (false, None) => m.version.clone(),
                (true, Some(a)) => a.clone(),
                (true, None) => String::new(),
            };
            batch.text(
                atlas,
                &clip(atlas, &sub, room),
                lx + 6.0,
                y + 15.0,
                grey,
                true,
            );
            let (state, tint) = if on {
                ("On", rgba(85, 255, 85, 255))
            } else {
                ("Off", rgba(255, 85, 85, 255))
            };
            let sw = HudBatch::text_width(atlas, state);
            batch.text(atlas, state, lx + lw - sw - 8.0, y + 4.0, tint, true);
            crate::widgets::record(&m.title, row, true);
        }
        // The list's own scrollbar: a thumb, as there is no partial row.
        if shown.len() > at.rows {
            let (top, bottom) = (ly, ly + lh);
            let thumb = ((bottom - top) * at.rows as f32 / shown.len() as f32).max(8.0);
            let span = (bottom - top - thumb).max(0.0);
            let range = (shown.len() - at.rows).max(1) as f32;
            let y = top + span * first as f32 / range;
            batch.fill(
                atlas,
                lx + lw - 3.0,
                y,
                2.0,
                thumb,
                rgba(200, 200, 200, 200),
            );
        }
        // The picked mod.
        let [dx, dy, dw, _] = at.detail;
        match picked.map(|i| (i, &installed[i])) {
            None => {}
            Some((i, m)) => {
                let name = clip(atlas, name_of(m), dw);
                batch.text(atlas, &name, dx, dy, u32::MAX, true);
                let mut y = dy + 14.0;
                let mut line = |batch: &mut HudBatch, text: &str, color: u32| {
                    batch.text(atlas, &clip(atlas, text, dw), dx, y, color, true);
                    y += 11.0;
                };
                if !m.version.is_empty() {
                    line(batch, &format!("Version: {}", m.version), grey);
                }
                if !m.authors.is_empty() {
                    line(batch, &format!("by {}", m.authors.join(", ")), grey);
                }
                if !m.id.is_empty() {
                    line(batch, &format!("Mod ID: {}", m.id), grey);
                }
                y += 4.0;
                let width = |t: &str| HudBatch::text_width(atlas, t).ceil();
                let (message, color) = match m.problem.as_deref() {
                    Some(p) => (p, rgba(255, 85, 85, 255)),
                    None => (m.description.as_str(), rgba(220, 220, 220, 255)),
                };
                let lines = ((at.toggle[1] - y - 6.0) / 10.0).max(0.0) as usize;
                for t in wrap(message, dw, width).iter().take(lines) {
                    batch.text(atlas, t, dx, y, color, true);
                    y += 10.0;
                }
                let [x, ty, bw, bh] = at.toggle;
                let label = format!("Enabled: {}", if mods.is_on(i) { "On" } else { "Off" });
                let hover = inside(cursor, at.toggle);
                let label = if m.dev {
                    "Always on (dev)".into()
                } else {
                    label
                };
                ui::button(
                    batch,
                    atlas,
                    x,
                    ty,
                    bw,
                    bh,
                    &Text::literal(label),
                    can_toggle(m),
                    hover,
                );
                if has_settings(m) {
                    let [x, sy, bw, bh] = at.settings;
                    let hover = inside(cursor, at.settings);
                    ui::button(
                        batch,
                        atlas,
                        x,
                        sy,
                        bw,
                        bh,
                        &Text::literal("Settings"),
                        true,
                        hover,
                    );
                }
            }
        }
        let [x, y, bw, bh] = at.done;
        let hover = inside(cursor, at.done);
        let done = lang::get_or("gui.done", "Done");
        ui::button(
            batch,
            atlas,
            x,
            y,
            bw,
            bh,
            &Text::literal(done),
            true,
            hover,
        );
    }

    /// A press at `(x, y)` on a screen `w` x `h`.
    pub fn click(
        &mut self,
        mods: &ModList,
        atlas: &HudAtlas,
        (x, y): (f32, f32),
        (w, h): (f32, f32),
    ) -> Click {
        let at = Layout::new(w.ceil(), h.ceil());
        let installed = &mods.installed;
        let here = (x, y);
        self.search.focused = inside(here, at.search);
        if self.search.focused {
            self.search.click(atlas, x, false, false);
            return Click::Pressed;
        }
        if inside(here, at.done) {
            return Click::Done;
        }
        if inside(here, at.sort) {
            self.descending = !self.descending;
            self.first = 0;
            return Click::Pressed;
        }
        if inside(here, at.list) {
            let shown = visible(installed, &self.search.value, self.descending);
            let first = self.first_row(shown.len(), at.rows);
            let row = ((y - at.list[1]) / ROW).floor() as usize;
            return match shown.get(first + row) {
                Some(&i) if row < at.rows => {
                    self.selected = Some(i);
                    Click::Pressed
                }
                _ => Click::Nothing,
            };
        }
        let Some(i) = self.picked(installed) else {
            return Click::Nothing;
        };
        let m = &installed[i];
        if inside(here, at.toggle) && can_toggle(m) {
            return Click::Toggle(i);
        }
        if inside(here, at.settings) && has_settings(m) {
            return Click::Settings(i);
        }
        Click::Nothing
    }

    /// The wheel over the list (`amount` > 0 scrolls up), in rows of the screen `w` x `h`.
    pub fn wheel(&mut self, installed: &[Installed], size: (f32, f32), amount: f32) {
        let at = Layout::new(size.0.ceil(), size.1.ceil());
        let shown = visible(installed, &self.search.value, self.descending).len();
        let max = shown.saturating_sub(at.rows);
        let first = self.first_row(shown, at.rows) as isize;
        let by = if amount > 0.0 { -1 } else { 1 };
        self.first = (first + by).clamp(0, max as isize) as usize;
    }

    pub fn type_text(&mut self, text: &str) {
        if self.search.focused {
            let typed: String = text.chars().filter(|c| !c.is_control()).collect();
            self.search.insert_text(&typed);
            self.first = 0;
        }
    }

    /// A key: true when the page took it. Up and Down pick the mod before and after; typing goes to
    /// the search box while it has the focus; Escape unfocuses it, and then the options screen's
    /// closes the page.
    pub fn key(
        &mut self,
        installed: &[Installed],
        size: (f32, f32),
        key: winit::keyboard::KeyCode,
        text: Option<&str>,
    ) -> bool {
        use winit::keyboard::KeyCode as K;
        let rows = Layout::new(size.0.ceil(), size.1.ceil()).rows;
        match key {
            K::ArrowUp => self.step(installed, -1, rows),
            K::ArrowDown => self.step(installed, 1, rows),
            K::Escape if self.search.focused => self.search.focused = false,
            _ if self.search.focused => {
                if !self.search.key(key, false, false)
                    && let Some(t) = text
                {
                    self.type_text(t);
                } else {
                    self.first = 0;
                }
            }
            _ => return false,
        }
        true
    }

    /// Ctrl+A/C/V/X in the search box; false when it hasn't the focus.
    pub fn shortcut(
        &mut self,
        shortcut: crate::clipboard::EditShortcut,
        clipboard: &mut dyn crate::clipboard::Clipboard,
    ) -> bool {
        if !self.search.focused {
            return false;
        }
        self.search.shortcut(shortcut, clipboard);
        self.first = 0;
        true
    }
}

/// A native mod with something to set: its keys or buttons.
pub fn has_settings(m: &Installed) -> bool {
    m.native
        .as_ref()
        .is_some_and(|p| !p.keys.is_empty() || !p.buttons.is_empty())
}

/// `text` cut with an ellipsis to fit `max` pixels.
fn clip(atlas: &HudAtlas, text: &str, max: f32) -> String {
    let width = |t: &str| HudBatch::text_width(atlas, t);
    if width(text) <= max {
        return text.to_owned();
    }
    let mut out: String = text.to_owned();
    while !out.is_empty() && width(&format!("{out}...")) > max {
        out.pop();
    }
    format!("{out}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mod_(id: &str, name: &str, version: &str, authors: &[&str]) -> Installed {
        Installed {
            id: id.into(),
            title: format!("{name} {version}"),
            version: version.into(),
            authors: authors.iter().map(|a| a.to_string()).collect(),
            ..Default::default()
        }
    }

    fn sample() -> Vec<Installed> {
        vec![
            mod_("zeta", "Zeta Tools", "2.1", &["Almeida"]),
            mod_("alpha", "alpha Pack", "1.0", &["Ana", "Bruno"]),
            mod_("mid", "Middle", "0.3", &[]),
        ]
    }

    #[test]
    fn the_name_is_the_title_without_its_version() {
        let m = mod_("a", "Spectral Tools", "1.0", &[]);
        assert_eq!(name_of(&m), "Spectral Tools");
        // No version known: the title as it is.
        let bare = Installed {
            title: "broken.hmod".into(),
            ..Default::default()
        };
        assert_eq!(name_of(&bare), "broken.hmod");
    }

    #[test]
    fn sorts_by_name_whatever_the_case_and_reverses() {
        let list = sample();
        assert_eq!(visible(&list, "", false), vec![1, 2, 0]);
        assert_eq!(visible(&list, "", true), vec![0, 2, 1]);
    }

    #[test]
    fn the_search_matches_name_id_or_author_in_any_case() {
        let list = sample();
        assert_eq!(visible(&list, "PACK", false), vec![1]);
        assert_eq!(visible(&list, "mid", false), vec![2]);
        assert_eq!(visible(&list, " bruno ", false), vec![1]);
        assert_eq!(visible(&list, "nothing", false), Vec::<usize>::new());
        // A name never matches through its version.
        assert_eq!(visible(&list, "2.1", false), Vec::<usize>::new());
    }

    #[test]
    fn the_pick_falls_back_to_the_first_shown() {
        let list = sample();
        let mut menu = ModMenu::default();
        assert_eq!(menu.picked(&list), Some(1));
        menu.selected = Some(0);
        assert_eq!(menu.picked(&list), Some(0));
        // The search hides the pick: the first one it leaves.
        menu.search.set_value("middle");
        assert_eq!(menu.picked(&list), Some(2));
        menu.search.set_value("zzz");
        assert_eq!(menu.picked(&list), None);
        assert_eq!(menu.picked(&[]), None);
    }

    #[test]
    fn arrows_move_through_the_list_in_its_order_and_stop_at_the_ends() {
        let list = sample();
        let mut menu = ModMenu::default();
        // Order: alpha (1), Middle (2), Zeta (0).
        menu.step(&list, 1, 8);
        assert_eq!(menu.picked(&list), Some(2));
        menu.step(&list, 5, 8);
        assert_eq!(menu.picked(&list), Some(0));
        menu.step(&list, -9, 8);
        assert_eq!(menu.picked(&list), Some(1));
    }

    #[test]
    fn stepping_scrolls_the_picked_row_into_view() {
        let list: Vec<Installed> = (0..10)
            .map(|i| mod_(&format!("m{i}"), &format!("Mod {i}"), "1", &[]))
            .collect();
        let mut menu = ModMenu::default();
        for _ in 0..5 {
            menu.step(&list, 1, 3);
        }
        // Row 5 of 0..10 with 3 rows shown: rows 3, 4 and 5.
        assert_eq!(menu.picked(&list), Some(5));
        assert_eq!(menu.first, 3);
        for _ in 0..5 {
            menu.step(&list, -1, 3);
        }
        assert_eq!(menu.first, 0);
    }

    #[test]
    fn dev_mods_and_unreadable_files_cannot_be_switched() {
        let dev = Installed {
            id: "d".into(),
            dev: true,
            ..Default::default()
        };
        let unreadable = Installed::default();
        assert!(!can_toggle(&dev) && !can_toggle(&unreadable));
        assert!(can_toggle(&mod_("a", "A", "1", &[])));
    }

    #[test]
    fn a_page_a_little_taller_than_the_list_shows_at_least_one_row() {
        for (w, h) in [(200.0, 100.0), (320.0, 240.0), (854.0, 480.0)] {
            let at = Layout::new(w, h);
            assert!(at.rows >= 1);
            assert!(at.list[0] + at.list[2] <= at.detail[0]);
        }
    }
}
