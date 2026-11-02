use crate::gamepad::PadBtn;
use bevy::prelude::*;

#[derive(Default, Clone)]
pub struct DsIn {
    pub connected: bool,
    pub usb: bool,
    pub ls: Vec2,
    pub rs: Vec2,
    pub lt: f32,
    pub rt: f32,
    pub held: Vec<PadBtn>,
    pub mic_btn: bool,
    pub battery: u8,
}
#[derive(Default, Clone)]
pub struct Touch { pub drag: Vec2, pub scroll: f32, pub swipe: i8, pub fingers: u8 }
#[derive(Clone)]
pub enum Haptic { Step(Option<String>), Land(f32), Shot(f32), Hurt(f32, f32), Boom(f32), Smash(f32) }
pub fn touch_click() -> bool { false }
pub fn input() -> Option<DsIn> { None }
pub fn connected_usb() -> bool { false }
pub fn take_touch() -> Touch { Touch::default() }
pub fn setup() {}
pub fn dualsense_system() {}
pub fn event(_h: Haptic) {}
