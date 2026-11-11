//! # godot-vehicle
//!
//! GDExtension-обёртка вокруг [`vehicle_core`]: ресурсы деталей, ресурс сборки
//! и узел [`vehicle_body::VehicleBody3D`], который применяет силы ядра к
//! `RigidBody3D`.
//!
//! Классы регистрируются автоматически при загрузке библиотеки.

mod assembly;
mod convert;
mod parts;
mod vehicle_body;

use godot::prelude::*;

struct VehiclePhysicsExtension;

#[gdextension]
unsafe impl ExtensionLibrary for VehiclePhysicsExtension {}
