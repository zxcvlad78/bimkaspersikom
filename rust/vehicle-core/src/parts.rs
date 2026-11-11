//! Каталог деталей: спецификации компонентов автомобиля.
//!
//! Каждая деталь — это данные (спецификация), которые можно создавать,
//! менять и комбинировать. Совместимость проверяется в [`crate::assembly`].

use glam::Vec3;

use crate::powertrain::differential::DifferentialKind;
use crate::powertrain::engine::EngineConfig;
use crate::powertrain::transmission::TransmissionConfig;
use crate::powertrain::ClutchConfig;
use crate::suspension::SuspensionConfig;
use crate::tire::TireConfig;

/// Общие метаданные любой детали.
#[derive(Clone, Debug)]
pub struct PartMeta {
    /// Уникальный идентификатор модели, напр. "inline4_turbo_2.0".
    pub id: String,
    /// Отображаемое название.
    pub name: String,
    /// Производитель.
    pub manufacturer: String,
    /// Масса, кг.
    pub mass: f32,
    /// Цена (для магазина/экономики).
    pub price: f32,
    /// Прочность: 1.0 — новая, 0.0 — в хлам.
    pub condition: f32,
}

impl Default for PartMeta {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            manufacturer: String::new(),
            mass: 0.0,
            price: 0.0,
            condition: 1.0,
        }
    }
}

impl PartMeta {
    pub fn new(id: &str, name: &str, mass: f32) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            manufacturer: String::new(),
            mass,
            price: 0.0,
            condition: 1.0,
        }
    }
}

/// Угол машины.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Corner {
    FrontLeft,
    FrontRight,
    RearLeft,
    RearRight,
}

impl Corner {
    pub fn is_front(self) -> bool {
        matches!(self, Corner::FrontLeft | Corner::FrontRight)
    }
    pub fn is_left(self) -> bool {
        matches!(self, Corner::FrontLeft | Corner::RearLeft)
    }
    /// Боковой знак: -1 слева, +1 справа.
    pub fn side(self) -> f32 {
        if self.is_left() {
            -1.0
        } else {
            1.0
        }
    }
    pub fn all() -> [Corner; 4] {
        [
            Corner::FrontLeft,
            Corner::FrontRight,
            Corner::RearLeft,
            Corner::RearRight,
        ]
    }
}

/// Схема привода.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DriveLayout {
    Rwd,
    Fwd,
    /// Полный привод с долей момента на переднюю ось 0..1.
    Awd { front_bias: f32 },
}

impl DriveLayout {
    pub fn front_bias(self) -> f32 {
        match self {
            DriveLayout::Rwd => 0.0,
            DriveLayout::Fwd => 1.0,
            DriveLayout::Awd { front_bias } => front_bias.clamp(0.0, 1.0),
        }
    }
}

/// Тип подвески.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuspensionKind {
    MacPherson,
    DoubleWishbone,
    MultiLink,
    TrailingArm,
    SolidAxle,
    Pushrod,
}

/// Тип кузова (влияет на аэродинамику и жёсткость).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyMaterial {
    Steel,
    Aluminium,
    Fiberglass,
    Carbon,
}

/// Крепление колеса (разболтовка) — должно совпадать у диска и ступицы.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoltPattern {
    pub bolts: u8,
    /// Диаметр окружности центров, мм.
    pub pcd_mm: u32,
}

/// Шасси/кузовной каркас.
#[derive(Clone, Debug)]
pub struct ChassisSpec {
    pub meta: PartMeta,
    /// База (между осями), м.
    pub wheelbase: f32,
    /// Колея спереди, м.
    pub track_front: f32,
    /// Колея сзади, м.
    pub track_rear: f32,
    /// Высота центра масс над землёй, м.
    pub com_height: f32,
    /// Продольное смещение центра масс (+ назад), м.
    pub com_offset: f32,
    /// Радиус колёсной арки, м (максимальный радиус колеса).
    pub arch_radius: f32,
    /// Разболтовка ступиц.
    pub hub_bolt_pattern: BoltPattern,
    /// Максимальный диаметр тормозного диска, м.
    pub max_brake_disc: f32,
    /// Жёсткость кузова на кручение (условно), 1..10.
    pub torsional_stiffness: f32,
    /// Объём моторного отсека (ДxШxВ), м.
    pub engine_bay: Vec3,
    pub material: BodyMaterial,
}

/// Двигатель в сборе (с маховиком и сцеплением).
#[derive(Clone, Debug)]
pub struct EngineSpec {
    pub meta: PartMeta,
    pub engine: EngineConfig,
    /// Сцепление, идущее в комплекте с двигателем.
    pub clutch: ClutchConfig,
    /// Габариты (ДxШxВ), м.
    pub size: Vec3,
    /// Позиция крепления в кузове (локально).
    pub mount: Vec3,
}

/// Коробка передач.
#[derive(Clone, Debug)]
pub struct GearboxSpec {
    pub meta: PartMeta,
    pub transmission: TransmissionConfig,
    pub layout: DriveLayout,
    /// Габариты, м.
    pub size: Vec3,
    pub mount: Vec3,
}

/// Дифференциал (межколёсный или центральный).
#[derive(Clone, Debug)]
pub struct DifferentialSpec {
    pub meta: PartMeta,
    pub kind: DifferentialKind,
    /// Предельный момент, Н·м.
    pub max_torque: f32,
}

/// Подвеска одного угла.
#[derive(Clone, Debug)]
pub struct SuspensionSpec {
    pub meta: PartMeta,
    pub kind: SuspensionKind,
    pub suspension: SuspensionConfig,
    /// Статический развал, градусы (отрицательный — верх внутрь).
    pub camber_deg: f32,
    /// Статическое схождение, градусы.
    pub toe_deg: f32,
    /// Максимальный угол поворота (для рулевых), градусы.
    pub max_steer_deg: f32,
}

/// Диск.
#[derive(Clone, Debug)]
pub struct RimSpec {
    pub meta: PartMeta,
    /// Диаметр, дюймы.
    pub diameter_in: f32,
    /// Ширина, дюймы.
    pub width_in: f32,
    /// Вылет (ET), мм.
    pub offset_mm: f32,
    pub bolt_pattern: BoltPattern,
}

impl RimSpec {
    /// Радиус диска в метрах.
    pub fn radius_m(&self) -> f32 {
        self.diameter_in * 0.0254 * 0.5
    }
}

/// Шина.
#[derive(Clone, Debug)]
pub struct TireSpec {
    pub meta: PartMeta,
    pub tire: TireConfig,
    /// Индекс нагрузки (справочно).
    pub load_index: u16,
    /// Индекс скорости, км/ч (справочно).
    pub speed_rating: f32,
}

/// Тормоз одного колеса.
#[derive(Clone, Debug)]
pub struct BrakeSpec {
    pub meta: PartMeta,
    /// Диаметр диска, м.
    pub disc_diameter: f32,
    /// Толщина, м.
    pub disc_thickness: f32,
    /// Число поршней.
    pub pistons: u8,
    /// Трение колодок.
    pub pad_mu: f32,
    /// Вентилируемый.
    pub vented: bool,
}

impl BrakeSpec {
    /// Оценка максимального тормозного момента, Н·м.
    pub fn max_torque(&self) -> f32 {
        // F = p * A * mu; приблизительно через геометрию и число поршней.
        let piston_area = std::f32::consts::PI * (0.02_f32).powi(2) * self.pistons as f32;
        let clamp_force = 6.0e6 * piston_area; // 60 бар * площадь
        let effective_radius = self.disc_diameter * 0.5 * 0.8;
        2.0 * clamp_force * self.pad_mu * effective_radius
    }
}

/// Аэродинамическая деталь (сплиттер/крыло/диффузор).
#[derive(Clone, Debug)]
pub struct AeroSpec {
    pub meta: PartMeta,
    /// Прибавка лобового сопротивления (ΔCdA).
    pub drag_delta: f32,
    /// Прибавка прижимной силы (ΔClA).
    pub downforce_delta: f32,
    /// Позиция (локально).
    pub mount: Vec3,
}

/// Кузовная панель / обвес.
#[derive(Clone, Debug)]
pub struct BodySpec {
    pub meta: PartMeta,
    /// Коэффициент лобового сопротивления.
    pub drag_cd: f32,
    /// Площадь миделя, м².
    pub frontal_area: f32,
    /// Прижимная сила кузова (ClA).
    pub downforce: f32,
    pub material: BodyMaterial,
}
