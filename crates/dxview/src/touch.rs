//! Multi-touch controls and an on-device layout editor. Touches are translated into the same action layer as
//! keyboard/gamepad, so rebinding a game action also rebinds its touch button. Menus receive a virtual cursor.
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

static EDITOR: AtomicBool = AtomicBool::new(false);
static EDIT_REQUEST: AtomicBool = AtomicBool::new(false);
static LOOK_SENS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1.0f32.to_bits());
pub fn look_sensitivity() -> f32 { f32::from_bits(LOOK_SENS.load(Ordering::Relaxed)) }
pub fn editor_open() -> bool { EDITOR.load(Ordering::Relaxed) }
pub fn request_editor() { EDIT_REQUEST.store(true, Ordering::Relaxed); }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TouchButton {
    pub id: u32,
    pub action: String,
    /// Normalized centre in the full window; the editor clamps to the safe area.
    pub x: f32,
    pub y: f32,
    /// Radius as a fraction of min(window width, window height).
    pub r: f32,
    pub enabled: bool,
}
impl TouchButton {
    fn new(id: u32, action: &str, x: f32, y: f32, r: f32) -> Self { Self { id, action: action.into(), x, y, r, enabled: true } }
    pub fn label(&self) -> String { short_action(&self.action) }
}

#[derive(Clone, Debug, Serialize, Deserialize, Resource)]
#[serde(default)]
pub struct TouchLayout {
    pub version: u32,
    pub buttons: Vec<TouchButton>,
    pub stick_x: f32,
    pub stick_y: f32,
    pub stick_r: f32,
    pub opacity: f32,
    pub look_sensitivity: f32,
    pub left_handed: bool,
    pub tap_to_use: bool,
}
impl Default for TouchLayout {
    fn default() -> Self {
        Self {
            version: 1,
            buttons: vec![
                TouchButton::new(1, "Fire / throw", 0.88, 0.72, 0.075),
                TouchButton::new(2, "Scope", 0.77, 0.84, 0.052),
                TouchButton::new(3, "Jump / swim up", 0.94, 0.52, 0.052),
                TouchButton::new(4, "Crouch / swim down", 0.83, 0.55, 0.047),
                TouchButton::new(5, "Use / pick up", 0.70, 0.63, 0.047),
                TouchButton::new(6, "Reload", 0.70, 0.88, 0.042),
                TouchButton::new(7, "Inventory screen", 0.07, 0.08, 0.036),
                TouchButton::new(8, "Goals / notes screen", 0.15, 0.08, 0.036),
                TouchButton::new(9, "Augmentation F3", 0.23, 0.08, 0.036),
                TouchButton::new(10, "Augmentation F12 (light)", 0.31, 0.08, 0.036),
                TouchButton::new(11, "Menu", 0.95, 0.075, 0.036),
                TouchButton::new(12, "Edit controls", 0.87, 0.075, 0.032),
            ],
            stick_x: 0.14, stick_y: 0.73, stick_r: 0.115,
            opacity: 0.62, look_sensitivity: 1.0, left_handed: false, tap_to_use: true,
        }
    }
}
impl TouchLayout {
    fn path() -> std::path::PathBuf { crate::platform::data_dir().join("touch_layout.ron") }
    pub fn load() -> Self {
        let mut l: Self = std::fs::read_to_string(Self::path()).ok()
            .and_then(|s| ron::from_str(&s).ok()).unwrap_or_default();
        l.validate(); l
    }
    pub fn save(&self) {
        if let Ok(s) = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            let _ = crate::platform::atomic_write(&Self::path(), s.as_bytes());
        }
    }
    fn validate(&mut self) {
        self.version = 1;
        self.opacity = finite(self.opacity, 0.62).clamp(0.15, 1.0);
        self.look_sensitivity = finite(self.look_sensitivity, 1.0).clamp(0.25, 3.0);
        self.stick_x = finite(self.stick_x, 0.14).clamp(0.05, 0.95);
        self.stick_y = finite(self.stick_y, 0.73).clamp(0.05, 0.95);
        self.stick_r = finite(self.stick_r, 0.115).clamp(0.05, 0.22);
        let mut ids = std::collections::HashSet::new();
        self.buttons.retain_mut(|b| {
            if !ids.insert(b.id) || b.action.is_empty() { return false; }
            b.x = finite(b.x, 0.5).clamp(0.0, 1.0); b.y = finite(b.y, 0.5).clamp(0.0, 1.0);
            b.r = finite(b.r, 0.045).clamp(0.022, 0.14); true
        });
        if self.buttons.len() > 40 { self.buttons.truncate(40); }
        if !self.buttons.iter().any(|b| b.action == "Menu") { self.buttons.push(TouchButton::new(self.next_id(), "Menu", 0.95, 0.075, 0.036)); }
        if !self.buttons.iter().any(|b| b.action == "Edit controls") { self.buttons.push(TouchButton::new(self.next_id(), "Edit controls", 0.87, 0.075, 0.032)); }
    }
    fn next_id(&self) -> u32 { self.buttons.iter().map(|b| b.id).max().unwrap_or(0) + 1 }
}
fn finite(v: f32, d: f32) -> f32 { if v.is_finite() { v } else { d } }

pub fn short_action(a: &str) -> String {
    match a {
        "Fire / throw" => "FIRE", "Scope" => "AIM", "Jump / swim up" => "JUMP", "Crouch / swim down" => "DUCK",
        "Use / pick up" => "USE", "Reload" => "RLD", "Inventory screen" => "INV", "Goals / notes screen" => "GOALS",
        "Menu" => "MENU", "Edit controls" => "EDIT", "Augmentation F12 (light)" => "LIGHT",
        s if s.starts_with("Augmentation ") => s.trim_start_matches("Augmentation ").split(' ').next().unwrap_or("AUG"),
        s => s.split(' ').next().unwrap_or("ACT").to_uppercase(),
    }.to_string()
}
fn action_choices() -> Vec<String> {
    let mut v: Vec<String> = crate::keybinds::ACTIONS.iter().map(|a| a.0.to_string()).collect();
    v.push("Menu".into()); v.push("Edit controls".into()); v
}

#[derive(Clone, Copy, Debug)]
enum Owner { Button(u32), Stick, Look, Ui }
#[derive(Clone, Debug)]
struct Track { owner: Owner, start: Vec2, last: Vec2, moved: f32 }
#[derive(Default, Clone)]
pub struct TouchFrame { pub keys: Vec<KeyCode>, pub buttons: Vec<MouseButton>, pub cursor: Option<Vec2>, pub scroll: f32 }
#[derive(Resource, Default)]
pub struct TouchState {
    fingers: HashMap<u64, Track>,
    pub frame: TouchFrame,
    look: Vec2,
    move_axes: Vec2,
    ui_left: bool,
    ui_tap: bool,
    pointer: Option<Vec2>,
    pub pressed: Vec<u32>,
}
impl TouchState {
    pub fn reset(&mut self) { self.fingers.clear(); self.frame = default(); self.look = Vec2::ZERO; self.move_axes = Vec2::ZERO; self.ui_left = false; self.ui_tap = false; self.pressed.clear(); }
    pub fn take_look(&mut self) -> Vec2 { std::mem::take(&mut self.look) }
    pub fn move_axes(&self) -> Option<Vec2> { (self.move_axes.length_squared() > 0.0001).then_some(self.move_axes) }
}

#[derive(Resource, Default)]
struct Visuals {
    buttons: HashMap<u32, (Entity, Entity)>,
    stick: Option<(Entity, Entity)>,
    editor_root: Option<Entity>,
    editor_hits: Vec<(EditAction, Rect)>,
    editor_built: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum EditAction { Done, Reset, Add, Delete, ActionPrev, ActionNext, SizeDown, SizeUp, OpacityDown, OpacityUp, SensDown, SensUp, SwapHands, TapUse }
impl EditAction {
    fn label(self) -> &'static str { match self {
        Self::Done => "SAVE / PLAY", Self::Reset => "RESET", Self::Add => "+ BUTTON", Self::Delete => "DELETE",
        Self::ActionPrev => "< ACTION", Self::ActionNext => "ACTION >", Self::SizeDown => "SIZE -", Self::SizeUp => "SIZE +",
        Self::OpacityDown => "FADE -", Self::OpacityUp => "FADE +", Self::SensDown => "SENS -", Self::SensUp => "SENS +",
        Self::SwapHands => "SWAP HANDS", Self::TapUse => "TAP USE",
    }}
}
#[derive(Resource)]
pub struct EditorState { open: bool, selected: Option<u32>, stick_selected: bool, drag: Option<u64>, paused_before: bool }
impl Default for EditorState { fn default() -> Self { Self { open: false, selected: None, stick_selected: false, drag: None, paused_before: false } } }

pub fn setup(mut commands: Commands, mut layout: ResMut<TouchLayout>, mut editor: ResMut<EditorState>) {
    *layout = TouchLayout::load();
    LOOK_SENS.store(layout.look_sensitivity.to_bits(), Ordering::Relaxed);
    commands.init_resource::<Visuals>();
    if cfg!(target_os = "android") && crate::settings::get("TouchEditOnStart") > 0.5 {
        editor.open = true; EDITOR.store(true, Ordering::Relaxed);
        crate::settings::set("TouchEditOnStart", 0.0); crate::settings::save();
    }
}

fn safe_rect(w: &Window) -> Rect {
    let sf = w.scale_factor().max(0.1);
    let (l, t, r, b) = crate::platform::safe_insets();
    let l = l / sf; let t = t / sf; let r = r / sf; let b = b / sf;
    Rect::new(l.min(w.width() * 0.3), t.min(w.height() * 0.3), (w.width() - r).max(l + 1.0), (w.height() - b).max(t + 1.0))
}
fn center(rect: Rect, x: f32, y: f32) -> Vec2 { Vec2::new(rect.min.x + rect.size().x * x, rect.min.y + rect.size().y * y) }
fn radius(w: &Window, r: f32) -> f32 { w.width().min(w.height()) * r }
fn hit_button(layout: &TouchLayout, w: &Window, p: Vec2) -> Option<u32> {
    let rect = safe_rect(w);
    layout.buttons.iter().rev().filter(|b| b.enabled).find(|b| p.distance(center(rect, b.x, b.y)) <= radius(w, b.r) * 1.12).map(|b| b.id)
}
fn action_input(action: &str, binds: &crate::keybinds::Bindings) -> Option<crate::keybinds::Input> {
    use crate::keybinds::Input;
    match action { "Menu" => Some(Input::Key(KeyCode::Escape)), "Edit controls" => None, _ => binds.input_for(action) }
}
fn push_input(frame: &mut TouchFrame, input: crate::keybinds::Input) {
    use crate::keybinds::Input;
    match input { Input::Key(k) => { if !frame.keys.contains(&k) { frame.keys.push(k); } }, Input::Mouse(m) => { if !frame.buttons.contains(&m) { frame.buttons.push(m); } } }
}

pub fn input_system(
    mut events: MessageReader<TouchInput>,
    mut state: ResMut<TouchState>,
    layout: Res<TouchLayout>,
    binds: Res<crate::keybinds::Bindings>,
    windows: Query<&Window, With<PrimaryWindow>>,
    editor: Res<EditorState>,
) {
    if !cfg!(target_os = "android") || crate::settings::get("TouchEnabled") < 0.5 { state.reset(); return; }
    let editor_open = editor.open || EDITOR.load(Ordering::Relaxed);
    if editor_open { state.frame = default(); state.look = Vec2::ZERO; state.move_axes = Vec2::ZERO; return; }
    let Ok(w) = windows.single() else { return };
    LOOK_SENS.store(layout.look_sensitivity.to_bits(), Ordering::Relaxed);
    let ui = crate::keybinds::UI_MODE.load(Ordering::Relaxed) || crate::menus::open() || crate::inventory_screen::screen_open() || crate::computers::open();
    state.frame = default(); state.ui_tap = false; state.pressed.clear();
    let rect = safe_rect(w);
    let stick_c = center(rect, layout.stick_x, layout.stick_y); let stick_r = radius(w, layout.stick_r);
    for e in events.read() {
        let p = e.position;
        match e.phase {
            TouchPhase::Started => {
                let owner = if ui { Owner::Ui }
                    else if let Some(id) = hit_button(&layout, w, p) { Owner::Button(id) }
                    else if p.distance(stick_c) <= stick_r * 1.25 { Owner::Stick }
                    else { Owner::Look };
                if matches!(owner, Owner::Ui) { state.ui_tap = true; state.pointer = Some(p); }
                if let Owner::Button(id) = owner { if layout.buttons.iter().find(|b| b.id == id).is_some_and(|b| b.action == "Edit controls") { request_editor(); } }
                state.fingers.insert(e.id, Track { owner, start: p, last: p, moved: 0.0 });
            }
            TouchPhase::Moved => {
                if let Some(t) = state.fingers.get_mut(&e.id) {
                    let d = p - t.last; t.moved += d.length(); t.last = p;
                    match t.owner {
                        Owner::Look => state.look += d,
                        Owner::Stick => { let v = (p - stick_c) / stick_r.max(1.0); let m = v.length(); state.move_axes = if m < 0.12 { Vec2::ZERO } else { v.clamp_length_max(1.0) }; }
                        Owner::Ui => { state.pointer = Some(p); }
                        _ => {}
                    }
                }
            }
            TouchPhase::Ended => {
                if let Some(t) = state.fingers.remove(&e.id) {
                    if matches!(t.owner, Owner::Look) && t.moved < 9.0 && layout.tap_to_use && !ui {
                        if let Some(inp) = binds.input_for("Use / pick up") { push_input(&mut state.frame, inp); }
                    }
                    if matches!(t.owner, Owner::Stick) { state.move_axes = Vec2::ZERO; }
                }
            }
            TouchPhase::Canceled => { state.fingers.remove(&e.id); state.move_axes = Vec2::ZERO; }
        }
    }
    state.ui_left = state.fingers.values().any(|t| matches!(t.owner, Owner::Ui));
    for t in state.fingers.values() {
        match t.owner {
            Owner::Button(id) => {
                state.pressed.push(id);
                if let Some(b) = layout.buttons.iter().find(|b| b.id == id) {
                    if let Some(inp) = action_input(&b.action, &binds) { push_input(&mut state.frame, inp); }
                }
            }
            Owner::Stick => {
                let a = state.move_axes;
                if a.y > 0.25 { state.frame.keys.push(KeyCode::KeyW); } if a.y < -0.25 { state.frame.keys.push(KeyCode::KeyS); }
                if a.x > 0.25 { state.frame.keys.push(KeyCode::KeyD); } if a.x < -0.25 { state.frame.keys.push(KeyCode::KeyA); }
                if a.length() > 0.05 && a.length() < 0.72 { state.frame.keys.push(KeyCode::ShiftLeft); }
            }
            Owner::Ui => { state.pointer = Some(t.last); }
            _ => {}
        }
    }
    if state.ui_left || state.ui_tap { state.frame.buttons.push(MouseButton::Left); }
    if let Some(p) = state.pointer { state.frame.cursor = Some(p); }
    // Two-finger vertical drag scrolls lists without fighting one-finger cursor/window dragging.
    let ui_tracks: Vec<&Track> = state.fingers.values().filter(|t| matches!(t.owner, Owner::Ui)).collect();
    if ui_tracks.len() >= 2 {
        let dy = ui_tracks.iter().map(|t| t.last.y - t.start.y).sum::<f32>() / ui_tracks.len() as f32;
        state.frame.scroll = -dy / 80.0;
    }
}

pub fn editor_system(
    mut commands: Commands,
    mut events: MessageReader<TouchInput>,
    mut layout: ResMut<TouchLayout>,
    mut state: ResMut<TouchState>,
    mut visuals: ResMut<Visuals>,
    mut editor: ResMut<EditorState>,
    mut time: ResMut<Time<Virtual>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !cfg!(target_os = "android") { return; }
    let st = &mut *editor;
    if EDIT_REQUEST.swap(false, Ordering::Relaxed) && !st.open {
        st.open = true; st.paused_before = time.is_paused(); time.pause(); state.reset(); EDITOR.store(true, Ordering::Relaxed);
    }
    if !st.open { return; }
    time.pause();
    let Ok(w) = windows.single() else { return };
    let rect = safe_rect(w);
    for e in events.read() {
        let p = e.position;
        match e.phase {
            TouchPhase::Started => {
                if let Some((action, _)) = visuals.editor_hits.iter().rev().find(|(_, r)| r.contains(p)) {
                    do_edit(*action, &mut layout, st);
                    if !st.open { EDITOR.store(false, Ordering::Relaxed); if !st.paused_before && !crate::menus::open() { time.unpause(); } state.reset(); }
                    continue;
                }
                if let Some(id) = hit_button(&layout, w, p) { st.selected = Some(id); st.stick_selected = false; st.drag = Some(e.id); }
                else if p.distance(center(rect, layout.stick_x, layout.stick_y)) <= radius(w, layout.stick_r) * 1.25 { st.stick_selected = true; st.selected = None; st.drag = Some(e.id); }
                else { st.selected = None; st.stick_selected = false; }
            }
            TouchPhase::Moved => {
                if st.drag == Some(e.id) {
                    let x = ((p.x - rect.min.x) / rect.size().x).clamp(0.02, 0.98);
                    let y = ((p.y - rect.min.y) / rect.size().y).clamp(0.02, 0.98);
                    if st.stick_selected { layout.stick_x = x; layout.stick_y = y; }
                    else if let Some(id) = st.selected { if let Some(b) = layout.buttons.iter_mut().find(|b| b.id == id) { b.x = x; b.y = y; } }
                }
            }
            TouchPhase::Ended | TouchPhase::Canceled => { if st.drag == Some(e.id) { st.drag = None; } }
        }
    }
    let _ = &mut commands;
}

fn do_edit(action: EditAction, layout: &mut TouchLayout, st: &mut EditorState) {
    match action {
        EditAction::Done => { layout.validate(); layout.save(); st.open = false; }
        EditAction::Reset => { *layout = TouchLayout::default(); st.selected = None; st.stick_selected = false; }
        EditAction::Add => { let id = layout.next_id(); layout.buttons.push(TouchButton::new(id, "Fire / throw", 0.5, 0.5, 0.045)); st.selected = Some(id); st.stick_selected = false; }
        EditAction::Delete => { if let Some(id) = st.selected { layout.buttons.retain(|b| b.id != id || b.action == "Menu" || b.action == "Edit controls"); st.selected = None; } }
        EditAction::ActionPrev | EditAction::ActionNext => {
            if let Some(id) = st.selected { if let Some(b) = layout.buttons.iter_mut().find(|b| b.id == id) {
                let choices = action_choices(); let i = choices.iter().position(|a| a == &b.action).unwrap_or(0);
                let n = if action == EditAction::ActionNext { (i + 1) % choices.len() } else { (i + choices.len() - 1) % choices.len() };
                b.action = choices[n].clone();
            }}
        }
        EditAction::SizeDown | EditAction::SizeUp => {
            let d = if action == EditAction::SizeUp { 0.008 } else { -0.008 };
            if st.stick_selected { layout.stick_r = (layout.stick_r + d).clamp(0.05, 0.22); }
            else if let Some(id) = st.selected { if let Some(b) = layout.buttons.iter_mut().find(|b| b.id == id) { b.r = (b.r + d).clamp(0.022, 0.14); } }
        }
        EditAction::OpacityDown => layout.opacity = (layout.opacity - 0.08).max(0.15),
        EditAction::OpacityUp => layout.opacity = (layout.opacity + 0.08).min(1.0),
        EditAction::SensDown => layout.look_sensitivity = (layout.look_sensitivity - 0.1).max(0.25),
        EditAction::SensUp => layout.look_sensitivity = (layout.look_sensitivity + 0.1).min(3.0),
        EditAction::SwapHands => { for b in &mut layout.buttons { b.x = 1.0 - b.x; } layout.stick_x = 1.0 - layout.stick_x; layout.left_handed = !layout.left_handed; }
        EditAction::TapUse => layout.tap_to_use = !layout.tap_to_use,
    }
}

pub fn draw_system(
    mut commands: Commands,
    layout: Res<TouchLayout>,
    state: Res<TouchState>,
    mut visuals: ResMut<Visuals>,
    editor: Res<EditorState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut nodes: Query<&mut Node>,
    mut bgs: Query<&mut BackgroundColor>,
    mut texts: Query<&mut Text>,
    mut vis: Query<&mut Visibility>,
) {
    if !cfg!(target_os = "android") { return; }
    let Ok(w) = windows.single() else { return };
    let editor_open = editor.open;
    let show = crate::settings::get("TouchEnabled") > 0.5 && (editor_open || !crate::keybinds::UI_MODE.load(Ordering::Relaxed));
    let rect = safe_rect(w); let base = w.width().min(w.height());
    // Remove stale button entities after a reset/delete.
    let live: std::collections::HashSet<u32> = layout.buttons.iter().map(|b| b.id).collect();
    visuals.buttons.retain(|id, (e, child)| { if live.contains(id) { true } else { commands.entity(*e).despawn(); let _ = child; false } });
    for b in &layout.buttons {
        let (e, label) = *visuals.buttons.entry(b.id).or_insert_with(|| {
            let e = commands.spawn((Node { position_type: PositionType::Absolute, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
                BackgroundColor(Color::NONE), BorderColor(Color::NONE), BorderRadius::MAX, GlobalZIndex(40), Pickable::IGNORE)).id();
            let t = commands.spawn((Text::new(b.label()), TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() }, TextColor(Color::WHITE), Pickable::IGNORE)).id();
            commands.entity(e).add_child(t); (e, t)
        });
        let r = base * b.r; let c = center(rect, b.x, b.y); let pressed = state.pressed.contains(&b.id);
        let selected = editor_open && editor.selected == Some(b.id);
        if let Ok(mut n) = nodes.get_mut(e) { n.left = Val::Px(c.x - r); n.top = Val::Px(c.y - r); n.width = Val::Px(r * 2.0); n.height = Val::Px(r * 2.0); n.display = if show && b.enabled { Display::Flex } else { Display::None }; }
        if let Ok(mut bg) = bgs.get_mut(e) { *bg = Color::srgba(if selected { 0.18 } else { 0.04 }, if pressed { 0.35 } else { 0.12 }, if pressed { 0.38 } else { 0.15 }, layout.opacity * if pressed { 0.8 } else { 0.5 }).into(); }
        if let Ok(mut border) = commands.get_entity(e) { border.insert(BorderColor(Color::srgba(0.3, 0.95, 0.85, layout.opacity * if selected { 1.0 } else { 0.65 }))); }
        if let Ok(mut t) = texts.get_mut(label) { **t = b.label(); }
    }
    // Stick base + knob.
    let (stick, knob) = *visuals.stick.get_or_insert_with(|| {
        let s = commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, BackgroundColor(Color::NONE), BorderRadius::MAX, GlobalZIndex(39), Pickable::IGNORE)).id();
        let k = commands.spawn((Node { position_type: PositionType::Absolute, ..default() }, BackgroundColor(Color::NONE), BorderRadius::MAX, GlobalZIndex(41), Pickable::IGNORE)).id();
        (s, k)
    });
    let sr = base * layout.stick_r; let sc = center(rect, layout.stick_x, layout.stick_y);
    if let Ok(mut n) = nodes.get_mut(stick) { n.left = Val::Px(sc.x - sr); n.top = Val::Px(sc.y - sr); n.width = Val::Px(sr * 2.0); n.height = Val::Px(sr * 2.0); n.display = if show { Display::Flex } else { Display::None }; }
    if let Ok(mut bg) = bgs.get_mut(stick) { *bg = Color::srgba(0.03, 0.1, 0.12, layout.opacity * 0.35).into(); }
    let kr = sr * 0.38; let off = state.move_axes * sr * 0.62;
    if let Ok(mut n) = nodes.get_mut(knob) { n.left = Val::Px(sc.x + off.x - kr); n.top = Val::Px(sc.y - off.y - kr); n.width = Val::Px(kr * 2.0); n.height = Val::Px(kr * 2.0); n.display = if show { Display::Flex } else { Display::None }; }
    if let Ok(mut bg) = bgs.get_mut(knob) { *bg = Color::srgba(0.25, 0.9, 0.8, layout.opacity * 0.65).into(); }
    let _ = &mut vis;
    draw_editor(&mut commands, editor_open, &layout, Some(&*editor), w, &mut visuals, &mut nodes, &mut bgs);
}

fn draw_editor(
    commands: &mut Commands, open: bool, layout: &TouchLayout, editor: Option<&EditorState>, w: &Window, visuals: &mut Visuals,
    nodes: &mut Query<&mut Node>, _bgs: &mut Query<&mut BackgroundColor>,
) {
    if !open {
        if let Some(e) = visuals.editor_root.take() { commands.entity(e).despawn(); }
        visuals.editor_built = false; visuals.editor_hits.clear(); return;
    }
    if visuals.editor_built { return; }
    visuals.editor_built = true;
    let root = commands.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), right: Val::Px(0.0), bottom: Val::Px(0.0), height: Val::Px(118.0), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(6.0)), row_gap: Val::Px(4.0), ..default() }, BackgroundColor(Color::srgba(0.02, 0.04, 0.06, 0.94)), GlobalZIndex(60), Pickable::IGNORE)).id();
    visuals.editor_root = Some(root);
    let title = commands.spawn((Text::new("CONTROL EDITOR — drag a button or the stick; tap a tool below"), TextFont { font_size: bevy::text::FontSize::Px(14.0), ..default() }, TextColor(Color::srgb(0.7, 1.0, 0.9)), Pickable::IGNORE)).id();
    commands.entity(root).add_child(title);
    let actions = [EditAction::Done, EditAction::Reset, EditAction::Add, EditAction::Delete, EditAction::ActionPrev, EditAction::ActionNext, EditAction::SizeDown,
        EditAction::SizeUp, EditAction::OpacityDown, EditAction::OpacityUp, EditAction::SensDown, EditAction::SensUp, EditAction::SwapHands, EditAction::TapUse];
    let cols = 7; let cell_w = (w.width() - 16.0) / cols as f32; let cell_h = 38.0; let top = w.height() - 118.0 + 30.0;
    visuals.editor_hits.clear();
    for (i, a) in actions.iter().enumerate() {
        let col = i % cols; let row = i / cols; let x = 8.0 + col as f32 * cell_w; let y = 28.0 + row as f32 * (cell_h + 4.0); let gy = top + row as f32 * (cell_h + 4.0);
        let label = if *a == EditAction::TapUse { if layout.tap_to_use { "TAP USE: ON" } else { "TAP USE: OFF" } } else { a.label() };
        let e = commands.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(y), width: Val::Px(cell_w - 4.0), height: Val::Px(cell_h), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BackgroundColor(Color::srgba(0.1, 0.2, 0.24, 0.95)), GlobalZIndex(61), Pickable::IGNORE)).id();
        let t = commands.spawn((Text::new(label), TextFont { font_size: bevy::text::FontSize::Px(12.0), ..default() }, TextColor(Color::WHITE), Pickable::IGNORE)).id();
        commands.entity(e).add_child(t); commands.entity(root).add_child(e);
        visuals.editor_hits.push((*a, Rect::new(x, gy, x + cell_w - 4.0, gy + cell_h)));
    }
    let _ = editor; let _ = nodes;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn layout_roundtrip_and_validation() {
        let mut l = TouchLayout::default(); l.buttons[0].x = f32::NAN; l.buttons.push(l.buttons[0].clone()); l.validate();
        assert!(l.buttons[0].x.is_finite()); let s = ron::ser::to_string(&l).unwrap(); let back: TouchLayout = ron::from_str(&s).unwrap(); assert_eq!(back.buttons.len(), l.buttons.len());
    }
    #[test] fn reserved_controls_survive_validation() {
        let mut l = TouchLayout::default(); l.buttons.clear(); l.validate(); assert!(l.buttons.iter().any(|b| b.action == "Menu")); assert!(l.buttons.iter().any(|b| b.action == "Edit controls"));
    }
}
