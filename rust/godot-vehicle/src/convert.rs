//! Преобразования между математикой ядра (`glam`) и встроенными типами Godot.
//!
//! Ядро не зависит от движка и использует `glam::Vec3`/`glam::Quat`. Здесь —
//! тонкий слой перевода в `Vector3`/`Quaternion` Godot (по компонентам, чтобы не
//! зависеть от версии `glam`, которую тянет godot-rust).

use glam::{Quat, Vec3};
use godot::builtin::{Quaternion, Vector3};

/// `glam::Vec3` → `Vector3`.
#[inline]
pub fn to_godot(v: Vec3) -> Vector3 {
    Vector3::new(v.x, v.y, v.z)
}

/// `Vector3` → `glam::Vec3`.
#[inline]
pub fn to_glam(v: Vector3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

/// `Quaternion` → `glam::Quat`.
#[inline]
pub fn quat_to_glam(q: Quaternion) -> Quat {
    Quat::from_xyzw(q.x, q.y, q.z, q.w)
}
