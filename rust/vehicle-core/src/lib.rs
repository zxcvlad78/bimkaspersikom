//! # vehicle-core
//!
//! Чистое ядро физики автомобиля без зависимости от движка. Всё считается в
//! `f32` и не аллоцирует память в горячем цикле.
//!
//! ## Состав
//! * [`tire`] — модель шины Pacejka ("Magic Formula") с комбинированным
//!   скольжением, длиной релаксации, чувствительностью к нагрузке и развалу.
//! * [`suspension`] — пружинно-демпферная подвеска с отбойником и стабилизатором.
//! * [`powertrain`] — полная симуляция двигателя (mean-value с физикой
//!   воздушного тракта, наддувом, охлаждением, смазкой, детонацией), сцепления,
//!   гидротрансформатора, коробки передач, дифференциалов и тормозов.
//! * [`parts`] — каталог деталей: спецификации компонентов автомобиля.
//! * [`assembly`] — сборка машины из деталей, проверка совместимости и вывод
//!   [`assembly::VehicleConfig`].
//! * [`vehicle`] — симуляция автомобиля: подвеска, шины, аэродинамика, руль.
//! * [`softbody`] — деформируемый кузов (решётка узлов и балок).
//! * [`catalog`] — готовые детали и примеры сборок.
//!
//! Хост (Godot) реализует [`ContactProvider`] для трассировки лучей.

pub mod assembly;
pub mod catalog;
pub mod parts;
pub mod powertrain;
pub mod softbody;
pub mod suspension;
pub mod tire;
pub mod vehicle;

pub use assembly::{CarAssembly, VehicleConfig, WheelAssembly, WheelConfig};
pub use parts::{
    AeroSpec, BodySpec, BrakeSpec, ChassisSpec, Corner, DifferentialSpec, DriveLayout, EngineSpec,
    GearboxSpec, PartMeta, RimSpec, SuspensionSpec, TireSpec,
};
pub use powertrain::{
    BrakeConfig, BrakeWheel, Clutch, ClutchConfig, DifferentialKind, Engine, EngineConfig,
    EngineOutput, FuelType, Powertrain, PowertrainConfig, PowertrainInputs, PowertrainOutput,
    Transmission, TransmissionConfig, WheelRole, WheelState,
};
pub use softbody::{SoftBeam, SoftBody, SoftNode};
pub use suspension::{Suspension, SuspensionConfig};
pub use tire::{TireConfig, TireOutput, TireState};
pub use vehicle::{ContactPoint, Telemetry, Vehicle, VehicleStep, Wheel};

use glam::Vec3;

/// Результат трассировки луча.
#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    /// Точка попадания в мировых координатах.
    pub point: Vec3,
    /// Нормаль поверхности в точке попадания.
    pub normal: Vec3,
    /// Коэффициент трения поверхности (обычно 1.0).
    pub friction: f32,
}

/// Источник контактов с миром. Реализуется хостом.
pub trait ContactProvider {
    /// Трассировка луча. `mask` — битовая маска слоёв физики.
    fn raycast(&self, from: Vec3, to: Vec3, mask: u32) -> Option<RayHit>;

    /// Множитель трения поверхности в точке (погода, грязь и т.п.).
    fn surface_friction(&self, _point: Vec3) -> f32 {
        1.0
    }
}

/// Бесконечная горизонтальная плоскость — удобно для тестов и фолбэка.
pub struct PlaneContact {
    pub height: f32,
    pub friction: f32,
}

impl PlaneContact {
    pub fn new(height: f32) -> Self {
        Self {
            height,
            friction: 1.0,
        }
    }
}

impl ContactProvider for PlaneContact {
    fn raycast(&self, from: Vec3, to: Vec3, _mask: u32) -> Option<RayHit> {
        let dy = to.y - from.y;
        if dy.abs() < 1e-6 {
            return None;
        }
        let t = (self.height - from.y) / dy;
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
        Some(RayHit {
            point: from.lerp(to, t),
            normal: Vec3::Y,
            friction: self.friction,
        })
    }

    fn surface_friction(&self, _point: Vec3) -> f32 {
        self.friction
    }
}

/// `sign` для f32, возвращает 0.0 при нуле.
#[inline]
pub fn sign(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Безопасная нормализация вектора.
#[inline]
pub fn normalized_or(v: Vec3, fallback: Vec3) -> Vec3 {
    let len = v.length();
    if len > 1e-6 {
        v / len
    } else {
        fallback
    }
}
