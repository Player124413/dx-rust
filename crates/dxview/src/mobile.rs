//! Android rendering budget: conservative presets, ETC2 texture cache, and dynamic 3D resolution.
//! The HUD and touch controls stay at native resolution; only the 3D cameras render into the scaled target.
use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::settings::{Backends, RenderCreation, WgpuSettings, WgpuSettingsPriority};
use bevy::render::RenderPlugin;
use bevy::ui::{IsDefaultUiCamera, UiTargetCamera};
use std::sync::atomic::{AtomicBool, Ordering};

pub static GPU_ETC2: AtomicBool = AtomicBool::new(false);

pub fn render_creation() -> RenderCreation {
    if cfg!(target_os = "android") {
        RenderCreation::Automatic(Box::new(WgpuSettings {
            device_label: Some("DX RUST mobile".into()),
            backends: Some(Backends::VULKAN | Backends::GL),
            power_preference: bevy::render::settings::PowerPreference::HighPerformance,
            // Enables supported compression features (including ETC2) without forcing unsupported ones.
            priority: WgpuSettingsPriority::Functionality,
            constrained_limits: Some(bevy::render::settings::WgpuLimits::downlevel_webgl2_defaults()),
            memory_hints: bevy::render::settings::MemoryHints::MemoryUsage,
            instance_flags: bevy::render::settings::InstanceFlags::empty(),
            ..default()
        }))
    } else {
        RenderCreation::Automatic(Box::new(WgpuSettings::default()))
    }
}

#[derive(Resource)]
pub struct MobileRender {
    pub scale: f32,
    pub min_scale: f32,
    pub max_scale: f32,
    pub size: UVec2,
    pub image: Option<Handle<Image>>,
    pub blit: Option<Entity>,
    pub fps_text: Option<Entity>,
    pub ema_ms: f32,
    pub slow: f32,
    pub fast: f32,
    pub cooldown: f32,
    pub warmup: f32,
    pub fps: f32,
    pub fps_frames: u32,
    pub fps_time: f32,
    pub backend: String,
    pub etc2: bool,
}
impl Default for MobileRender {
    fn default() -> Self {
        Self { scale: 0.7, min_scale: 0.4, max_scale: 0.85, size: UVec2::ZERO, image: None, blit: None,
            fps_text: None, ema_ms: 16.7, slow: 0.0, fast: 0.0, cooldown: 0.0, warmup: 0.0,
            fps: 0.0, fps_frames: 0, fps_time: 0.0, backend: String::new(), etc2: false }
    }
}

#[derive(Component)]
struct MobileBlit;

fn bounds(quality: u8, pixels: f32) -> (f32, f32, f32) {
    let (budget, max) = match quality { 1 => (640.0 * 360.0, 0.55), 2 => (960.0 * 540.0, 0.8), 3 => (1920.0 * 1080.0, 1.0), _ => (1280.0 * 720.0, 0.85) };
    let initial = (budget / pixels.max(1.0)).sqrt().min(max);
    (0.4, max, initial.clamp(0.4, max))
}

pub fn setup(
    mut commands: Commands,
    mut state: ResMut<MobileRender>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    device: Option<Res<bevy::render::renderer::RenderDevice>>,
    adapter: Option<Res<bevy::render::renderer::RenderAdapterInfo>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !cfg!(target_os = "android") { return; }
    let etc2 = device.as_ref().is_some_and(|d| d.features().contains(bevy::render::render_resource::WgpuFeatures::TEXTURE_COMPRESSION_ETC2));
    GPU_ETC2.store(etc2, Ordering::Relaxed);
    state.etc2 = etc2;
    state.backend = adapter.as_ref().map(|a| format!("{:?}", a.backend)).unwrap_or_else(|| "GPU".into());
    let Ok(w) = windows.single() else { return };
    let pixels = (w.physical_width().max(1) * w.physical_height().max(1)) as f32;
    let (min, max, initial) = bounds(crate::settings::mobile_quality(), pixels);
    state.min_scale = min; state.max_scale = max; state.scale = initial;
    state.size = scaled_size(w.physical_width(), w.physical_height(), initial);
    let img = target_image(&mut images, state.size);
    state.image = Some(img.clone());
    let blit = commands.spawn((
        Camera2d,
        Camera { order: 10, clear_color: bevy::camera::ClearColorConfig::Custom(Color::BLACK), ..default() },
        IsDefaultUiCamera,
        Msaa::Off,
    )).id();
    state.blit = Some(blit);
    commands.spawn((
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        ImageNode::new(img),
        UiTargetCamera(blit),
        MobileBlit,
        Pickable::IGNORE,
    ));
    let text = commands.spawn((
        Text::new(""), TextFont { font_size: bevy::text::FontSize::Px(13.0), ..default() },
        TextColor(Color::srgba(0.65, 1.0, 0.8, 0.9)),
        Node { position_type: PositionType::Absolute, left: Val::Px(8.0), top: Val::Px(6.0), ..default() },
        UiTargetCamera(blit), GlobalZIndex(90), Pickable::IGNORE,
    )).id();
    state.fps_text = Some(text);
    prune_cache();
    bevy::log::info!("mobile: {} ETC2={} scale={:.2}", state.backend, etc2, initial);
}

fn scaled_size(w: u32, h: u32, scale: f32) -> UVec2 {
    UVec2::new(((w as f32 * scale) as u32).max(4) & !3, ((h as f32 * scale) as u32).max(4) & !3)
}
fn target_image(images: &mut Assets<Image>, size: UVec2) -> Handle<Image> {
    let mut img = Image::new_uninit(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
        TextureDimension::D2, TextureFormat::Bgra8UnormSrgb, bevy::asset::RenderAssetUsages::RENDER_WORLD);
    img.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC;
    images.add(img)
}

/// Retarget every window-backed 3D camera to the scaled image. Reflection/drone/terminal cameras already
/// own their images and are left alone. The UI camera remains native-resolution.
fn retarget(state: &MobileRender, cameras: &mut Query<&mut RenderTarget, With<Camera3d>>) {
    let Some(img) = state.image.as_ref() else { return };
    for mut target in cameras.iter_mut() {
        if matches!(&*target, RenderTarget::Window(_)) { *target = img.clone().into(); }
    }
}

pub fn adaptive_system(
    time: Res<Time<Real>>,
    mut state: ResMut<MobileRender>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<&mut RenderTarget, With<Camera3d>>,
    mut blits: Query<&mut ImageNode, With<MobileBlit>>,
    lifecycle: Res<crate::platform::Lifecycle>,
) {
    if !cfg!(target_os = "android") { return; }
    retarget(&state, &mut cameras);
    let Ok(w) = windows.single() else { return };
    let dt = time.delta_secs().clamp(0.001, 0.1);
    state.fps_frames += 1; state.fps_time += dt;
    if state.fps_time >= 0.5 { state.fps = state.fps_frames as f32 / state.fps_time; state.fps_frames = 0; state.fps_time = 0.0; }
    if lifecycle.suspended { return; }
    state.warmup += dt;
    state.cooldown = (state.cooldown - dt).max(0.0);
    let ms = dt * 1000.0;
    state.ema_ms += (ms - state.ema_ms) * 0.06;
    let active = !crate::loader::busy() && !crate::menus::open() && !crate::touch::editor_open() && state.warmup > 4.0;
    if active {
        state.slow = if state.ema_ms > 18.5 { state.slow + dt } else { 0.0 };
        state.fast = if state.ema_ms < 14.2 { state.fast + dt } else { 0.0 };
        let mut next = state.scale;
        if state.slow > 1.2 && state.cooldown <= 0.0 { next = (state.scale - 0.08).max(state.min_scale); state.cooldown = 2.5; state.slow = 0.0; }
        else if state.fast > 5.0 && state.cooldown <= 0.0 { next = (state.scale + 0.04).min(state.max_scale); state.cooldown = 4.0; state.fast = 0.0; }
        let size = scaled_size(w.physical_width(), w.physical_height(), next);
        if size != state.size {
            state.scale = next; state.size = size;
            let img = target_image(&mut images, size);
            if let Some(old) = state.image.replace(img.clone()) { images.remove(old); }
            for mut node in &mut blits { node.image = img.clone(); }
            retarget(&state, &mut cameras);
        }
    }
    let _ = w;
}

pub fn overlay_system(state: Res<MobileRender>, mut texts: Query<&mut Text>, mut nodes: Query<&mut Node>) {
    if !cfg!(target_os = "android") { return; }
    let Some(e) = state.fps_text else { return };
    let show = crate::settings::get("MobileFps") > 0.5;
    if let Ok(mut node) = nodes.get_mut(e) { node.display = if show { Display::Flex } else { Display::None }; }
    if show {
        if let Ok(mut text) = texts.get_mut(e) {
            **text = format!("{:.0} FPS  |  3D {:.0}%  |  {}  |  {}", state.fps, state.scale * 100.0,
                if state.etc2 { "ETC2" } else { "RGBA" }, state.backend);
        }
    }
}

// ---- texture preparation and persistent ETC2 cache -------------------------------------------------------

fn hash_bytes(seed: u64, bytes: &[u8]) -> u64 {
    let mut h = seed; for b in bytes { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); } h
}
fn cache_dir() -> std::path::PathBuf { crate::platform::data_dir().join("cache").join(format!("etc2-v{}", dx_level::etc2::ENCODER_VERSION)) }
fn prune_cache() {
    let dir = cache_dir(); let Ok(rd) = std::fs::read_dir(&dir) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = rd.flatten().filter_map(|e| {
        let m = e.metadata().ok()?; Some((m.modified().ok()?, m.len(), e.path()))
    }).collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    for (_, len, path) in files { if total <= 384 * 1024 * 1024 { break; } if std::fs::remove_file(path).is_ok() { total = total.saturating_sub(len); } }
}

fn rgba_mip(t: &dx_level::TextureImage, level: usize) -> Option<Vec<u8>> {
    let w = (t.width >> level).max(1); let h = (t.height >> level).max(1);
    let data = t.mips.get(level)?;
    if t.bc1 { dx_level::dds::decode_level(dx_level::dds::Format::Bc1, data, w, h) }
    else if t.bc3 { dx_level::dds::decode_level(dx_level::dds::Format::Bc3, data, w, h) }
    else { (data.len() == (w * h * 4) as usize).then(|| data.clone()) }
}
fn box_down(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let nw = (w / 2).max(1); let nh = (h / 2).max(1); let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh { for x in 0..nw {
        let mut sum = [0u32; 4]; let mut n = 0;
        for dy in 0..2 { for dx in 0..2 { let sx = (x * 2 + dx).min(w - 1); let sy = (y * 2 + dy).min(h - 1);
            let p = &src[((sy * w + sx) * 4) as usize..][..4]; for c in 0..4 { sum[c] += p[c] as u32; } n += 1;
        }}
        let o = ((y * nw + x) * 4) as usize; for c in 0..4 { out[o + c] = (sum[c] / n) as u8; }
    }}
    out
}

/// Decode/cap/transcode on the loading path, never in the frame loop. The cache key includes pixels, dimensions,
/// alpha policy, size cap and encoder version, so mod changes cannot reuse stale textures.
pub fn prepare_texture(t: &dx_level::TextureImage) -> crate::GpuImage {
    let cap = crate::settings::mobile_texture_cap();
    let mut level = 0usize;
    while level + 1 < t.mips.len() && t.width.max(t.height) >> level > cap { level += 1; }
    let Some(mut px) = rgba_mip(t, level) else { return crate::GpuImage::rgba(t.width.max(1), t.height.max(1), Vec::new(), 0); };
    let mut w = (t.width >> level).max(1); let mut h = (t.height >> level).max(1);
    while w.max(h) > cap { px = box_down(&px, w, h); w = (w / 2).max(1); h = (h / 2).max(1); }
    let (width, height) = (w, h);
    let alpha = px.chunks_exact(4).any(|p| p[3] < 250);
    let mut mips = vec![px];
    while mips.last().is_some_and(|m| m.len() > 4) && (w > 1 || h > 1) {
        let next = box_down(mips.last().unwrap(), w, h); mips.push(next); w = (w / 2).max(1); h = (h / 2).max(1);
    }
    let use_etc2 = GPU_ETC2.load(Ordering::Relaxed) && crate::settings::get("MobileTextureMode") < 1.5;
    if !use_etc2 {
        let mut data = Vec::new(); for m in &mips { data.extend_from_slice(m); }
        return crate::GpuImage { width, height, format: TextureFormat::Rgba8UnormSrgb, data, levels: mips.len() as u32 };
    }
    let mut key = hash_bytes(0xcbf29ce484222325, &width.to_le_bytes()); key = hash_bytes(key, &height.to_le_bytes());
    key = hash_bytes(key, &[alpha as u8, crate::settings::mobile_quality()]);
    for m in &mips { key = hash_bytes(key, m); }
    let path = cache_dir().join(format!("{key:016x}.bin"));
    let bytes_per_block = if alpha { 16 } else { 8 };
    let expected: usize = mips.iter().enumerate().map(|(i, _)| dx_level::etc2::encoded_len((width >> i).max(1), (height >> i).max(1), alpha)).sum();
    if let Ok(cached) = std::fs::read(&path) {
        if cached.len() == expected + 16 && &cached[0..4] == b"DXE2" && u32::from_le_bytes(cached[4..8].try_into().unwrap()) == dx_level::etc2::ENCODER_VERSION {
            return crate::GpuImage { width, height, format: if alpha { TextureFormat::Etc2Rgba8UnormSrgb } else { TextureFormat::Etc2Rgb8UnormSrgb }, data: cached[16..].to_vec(), levels: mips.len() as u32 };
        }
    }
    let mut data = Vec::with_capacity(expected);
    for (i, m) in mips.iter().enumerate() { data.extend_from_slice(&dx_level::etc2::encode(m, (width >> i).max(1), (height >> i).max(1), alpha)); }
    let mut file = Vec::with_capacity(16 + data.len()); file.extend_from_slice(b"DXE2"); file.extend_from_slice(&dx_level::etc2::ENCODER_VERSION.to_le_bytes());
    file.extend_from_slice(&width.to_le_bytes()); file.extend_from_slice(&height.to_le_bytes()); file.extend_from_slice(&data);
    let _ = crate::platform::atomic_write(&path, &file);
    let _ = bytes_per_block;
    crate::GpuImage { width, height, format: if alpha { TextureFormat::Etc2Rgba8UnormSrgb } else { TextureFormat::Etc2Rgb8UnormSrgb }, data, levels: mips.len() as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn preset_budgets_are_bounded() {
        for q in 0..=3 { let (min,max,init)=bounds(q,1920.0*1080.0); assert!(min<=init && init<=max && max<=1.0); }
        assert!(bounds(1,1920.0*1080.0).2 < bounds(3,1920.0*1080.0).2);
    }
    #[test] fn scale_sizes_are_four_aligned() { let s=scaled_size(2401,1081,0.67); assert_eq!(s.x%4,0); assert_eq!(s.y%4,0); assert!(s.x>0 && s.y>0); }
}
