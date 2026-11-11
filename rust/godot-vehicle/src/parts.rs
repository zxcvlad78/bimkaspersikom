//! Ресурсы деталей автомобиля (`Resource`), редактируемые в инспекторе Godot.
//!
//! Каждая деталь ядра ([`vehicle_core::parts`]) представлена отдельным ресурсом
//! с экспортируемыми полями. Из деталей собирается [`crate::assembly::CarAssemblyResource`],
//! а затем — [`vehicle_core::assembly::VehicleConfig`] для симуляции.
//!
//! Соглашения:
//! * перечисления с данными из ядра «разложены» на плоский enum + числовые поля
//!   (Godot не умеет экспортировать Rust-перечисления с payload);
//! * объёмный КПД двигателя хранится как [`PackedFloat32Array`] вида
//!   `[rpm0, ve0, rpm1, ve1, ...]`.

use glam::Vec3;
use godot::prelude::*;

use vehicle_core::catalog;
use vehicle_core::parts::{
    AeroSpec, BodyMaterial, BodySpec, BoltPattern, BrakeSpec, ChassisSpec, DriveLayout, EngineSpec,
    GearboxSpec, PartMeta, RimSpec, SuspensionKind, SuspensionSpec, TireSpec,
};
use vehicle_core::powertrain::{
    Aspiration, ClutchConfig, CoolingSpec, DifferentialKind, EngineConfig, FrictionModel, FuelType,
    TorqueConverterConfig, TransmissionConfig,
};
use vehicle_core::suspension::SuspensionConfig;
use vehicle_core::tire::TireConfig;

use crate::convert::{to_glam, to_godot};

// ---------------------------------------------------------------------------
// Перечисления
// ---------------------------------------------------------------------------

/// Тип топлива.
#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, PartialEq, Eq)]
#[godot(via = i64)]
pub enum FuelTypeRes {
    Gasoline95 = 0,
    Gasoline98 = 1,
    E85 = 2,
    Diesel = 3,
    Lpg = 4,
}

impl Default for FuelTypeRes {
    fn default() -> Self {
        FuelTypeRes::Gasoline95
    }
}

impl FuelTypeRes {
    pub fn to_core(self) -> FuelType {
        match self {
            FuelTypeRes::Gasoline95 => FuelType::Gasoline95,
            FuelTypeRes::Gasoline98 => FuelType::Gasoline98,
            FuelTypeRes::E85 => FuelType::E85,
            FuelTypeRes::Diesel => FuelType::Diesel,
            FuelTypeRes::Lpg => FuelType::Lpg,
        }
    }
    pub fn from_core(f: FuelType) -> Self {
        match f {
            FuelType::Gasoline95 => FuelTypeRes::Gasoline95,
            FuelType::Gasoline98 => FuelTypeRes::Gasoline98,
            FuelType::E85 => FuelTypeRes::E85,
            FuelType::Diesel => FuelTypeRes::Diesel,
            FuelType::Lpg => FuelTypeRes::Lpg,
        }
    }
}

/// Тип наддува.
#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, PartialEq, Eq)]
#[godot(via = i64)]
pub enum AspirationRes {
    NaturallyAspirated = 0,
    Turbo = 1,
    Supercharged = 2,
}

impl Default for AspirationRes {
    fn default() -> Self {
        AspirationRes::NaturallyAspirated
    }
}

/// Тип подвески.
#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, PartialEq, Eq)]
#[godot(via = i64)]
pub enum SuspensionKindRes {
    MacPherson = 0,
    DoubleWishbone = 1,
    MultiLink = 2,
    TrailingArm = 3,
    SolidAxle = 4,
    Pushrod = 5,
}

impl Default for SuspensionKindRes {
    fn default() -> Self {
        SuspensionKindRes::MacPherson
    }
}

impl SuspensionKindRes {
    pub fn to_core(self) -> SuspensionKind {
        match self {
            SuspensionKindRes::MacPherson => SuspensionKind::MacPherson,
            SuspensionKindRes::DoubleWishbone => SuspensionKind::DoubleWishbone,
            SuspensionKindRes::MultiLink => SuspensionKind::MultiLink,
            SuspensionKindRes::TrailingArm => SuspensionKind::TrailingArm,
            SuspensionKindRes::SolidAxle => SuspensionKind::SolidAxle,
            SuspensionKindRes::Pushrod => SuspensionKind::Pushrod,
        }
    }
    pub fn from_core(k: SuspensionKind) -> Self {
        match k {
            SuspensionKind::MacPherson => SuspensionKindRes::MacPherson,
            SuspensionKind::DoubleWishbone => SuspensionKindRes::DoubleWishbone,
            SuspensionKind::MultiLink => SuspensionKindRes::MultiLink,
            SuspensionKind::TrailingArm => SuspensionKindRes::TrailingArm,
            SuspensionKind::SolidAxle => SuspensionKindRes::SolidAxle,
            SuspensionKind::Pushrod => SuspensionKindRes::Pushrod,
        }
    }
}

/// Материал кузова.
#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, PartialEq, Eq)]
#[godot(via = i64)]
pub enum BodyMaterialRes {
    Steel = 0,
    Aluminium = 1,
    Fiberglass = 2,
    Carbon = 3,
}

impl Default for BodyMaterialRes {
    fn default() -> Self {
        BodyMaterialRes::Steel
    }
}

impl BodyMaterialRes {
    pub fn to_core(self) -> BodyMaterial {
        match self {
            BodyMaterialRes::Steel => BodyMaterial::Steel,
            BodyMaterialRes::Aluminium => BodyMaterial::Aluminium,
            BodyMaterialRes::Fiberglass => BodyMaterial::Fiberglass,
            BodyMaterialRes::Carbon => BodyMaterial::Carbon,
        }
    }
    pub fn from_core(m: BodyMaterial) -> Self {
        match m {
            BodyMaterial::Steel => BodyMaterialRes::Steel,
            BodyMaterial::Aluminium => BodyMaterialRes::Aluminium,
            BodyMaterial::Fiberglass => BodyMaterialRes::Fiberglass,
            BodyMaterial::Carbon => BodyMaterialRes::Carbon,
        }
    }
}

/// Тип дифференциала.
#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, PartialEq, Eq)]
#[godot(via = i64)]
pub enum DifferentialKindRes {
    Open = 0,
    Locked = 1,
    Lsd = 2,
    Torsen = 3,
}

impl Default for DifferentialKindRes {
    fn default() -> Self {
        DifferentialKindRes::Open
    }
}

/// Схема привода.
#[derive(GodotConvert, Var, Export, Clone, Copy, Debug, PartialEq, Eq)]
#[godot(via = i64)]
pub enum DriveLayoutRes {
    Rwd = 0,
    Fwd = 1,
    Awd = 2,
}

impl Default for DriveLayoutRes {
    fn default() -> Self {
        DriveLayoutRes::Rwd
    }
}

// ---------------------------------------------------------------------------
// Вспомогательные функции
// ---------------------------------------------------------------------------

fn make_meta(
    id: &GString,
    name: &GString,
    manufacturer: &GString,
    mass: f32,
    price: f32,
    condition: f32,
) -> PartMeta {
    PartMeta {
        id: String::from(id),
        name: String::from(name),
        manufacturer: String::from(manufacturer),
        mass,
        price,
        condition,
    }
}

fn pfa(values: impl IntoIterator<Item = f32>) -> PackedFloat32Array {
    PackedFloat32Array::from(values.into_iter().collect::<Vec<f32>>())
}

// ---------------------------------------------------------------------------
// Шасси
// ---------------------------------------------------------------------------

/// Шасси/кузовной каркас.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct ChassisResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    /// База (между осями), м.
    #[export]
    pub wheelbase: f32,
    /// Колея спереди, м.
    #[export]
    pub track_front: f32,
    /// Колея сзади, м.
    #[export]
    pub track_rear: f32,
    /// Высота центра масс над землёй, м.
    #[export]
    pub com_height: f32,
    /// Продольное смещение центра масс (+ назад), м.
    #[export]
    pub com_offset: f32,
    /// Максимальный радиус колеса (арка), м.
    #[export]
    pub arch_radius: f32,
    /// Число болтов ступицы.
    #[export]
    pub hub_bolts: i64,
    /// Диаметр окружности центров, мм.
    #[export]
    pub hub_pcd_mm: i64,
    /// Максимальный диаметр тормозного диска, м.
    #[export]
    pub max_brake_disc: f32,
    /// Жёсткость кузова на кручение (условно 1..10).
    #[export]
    pub torsional_stiffness: f32,
    /// Объём моторного отсека (ДxШxВ), м.
    #[export]
    pub engine_bay: Vector3,
    #[export]
    pub material: BodyMaterialRes,

    base: Base<Resource>,
}

impl ChassisResource {
    fn from_spec(base: Base<Resource>, s: &ChassisSpec) -> Self {
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            wheelbase: s.wheelbase,
            track_front: s.track_front,
            track_rear: s.track_rear,
            com_height: s.com_height,
            com_offset: s.com_offset,
            arch_radius: s.arch_radius,
            hub_bolts: s.hub_bolt_pattern.bolts as i64,
            hub_pcd_mm: s.hub_bolt_pattern.pcd_mm as i64,
            max_brake_disc: s.max_brake_disc,
            torsional_stiffness: s.torsional_stiffness,
            engine_bay: to_godot(s.engine_bay),
            material: BodyMaterialRes::from_core(s.material),
            base,
        }
    }

    pub fn build_spec(&self) -> ChassisSpec {
        ChassisSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            wheelbase: self.wheelbase,
            track_front: self.track_front,
            track_rear: self.track_rear,
            com_height: self.com_height,
            com_offset: self.com_offset,
            arch_radius: self.arch_radius,
            hub_bolt_pattern: BoltPattern {
                bolts: self.hub_bolts.clamp(0, 255) as u8,
                pcd_mm: self.hub_pcd_mm.max(0) as u32,
            },
            max_brake_disc: self.max_brake_disc,
            torsional_stiffness: self.torsional_stiffness,
            engine_bay: to_glam(self.engine_bay),
            material: self.material.to_core(),
        }
    }
}

#[godot_api]
impl IResource for ChassisResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::chassis_coupe())
    }
}

// ---------------------------------------------------------------------------
// Двигатель
// ---------------------------------------------------------------------------

/// Двигатель в сборе (с маховиком и сцеплением).
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct EngineResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    /// Рабочий объём, л.
    #[export]
    pub displacement_l: f32,
    /// Число цилиндров.
    #[export]
    pub cylinders: i64,
    /// Степень сжатия.
    #[export]
    pub compression_ratio: f32,
    /// Объём ресивера, л.
    #[export]
    pub plenum_volume_l: f32,
    /// Кривая объёмного КПД: `[rpm0, ve0, rpm1, ve1, ...]`.
    #[export]
    pub volumetric_efficiency: PackedFloat32Array,
    #[export]
    pub aspiration: AspirationRes,
    #[export]
    pub fuel: FuelTypeRes,
    /// Целевое AFR.
    #[export]
    pub afr_target: f32,
    #[export]
    pub idle_rpm: f32,
    #[export]
    pub redline_rpm: f32,
    #[export]
    pub limiter_rpm: f32,
    /// Момент инерции коленвала с маховиком, кг·м².
    #[export]
    pub inertia: f32,

    #[export]
    pub friction_constant: f32,
    #[export]
    pub friction_linear: f32,
    #[export]
    pub friction_quadratic: f32,

    #[export]
    pub coolant_mass: f32,
    #[export]
    pub radiator_rate: f32,
    #[export]
    pub thermostat_open_c: f32,
    #[export]
    pub fan_rate: f32,
    #[export]
    pub max_temp_c: f32,
    #[export]
    pub oil_mass: f32,
    #[export]
    pub oil_cooler_rate: f32,

    /// Момент навесного оборудования, Н·м.
    #[export]
    pub accessory_torque: f32,
    /// Производительность форсунок, кг/с.
    #[export]
    pub injector_max_flow: f32,
    /// Объём бензобака, л.
    #[export]
    pub fuel_tank_l: f32,
    /// Масса двигателя, кг.
    #[export]
    pub engine_mass: f32,

    // --- Турбонаддув ---
    #[export]
    pub turbo_max_boost: f32,
    #[export]
    pub turbo_wastegate: f32,
    #[export]
    pub turbo_lag: f32,
    #[export]
    pub turbo_efficiency: f32,
    #[export]
    pub turbo_intercooler: f32,

    // --- Механический нагнетатель ---
    #[export]
    pub super_max_boost: f32,
    #[export]
    pub super_intercooler: f32,
    #[export]
    pub super_drive_torque: f32,

    // --- Сцепление ---
    #[export]
    pub clutch_capacity: f32,
    #[export]
    pub clutch_stiffness: f32,
    #[export]
    pub clutch_thermal_mass: f32,
    #[export]
    pub clutch_fade_temp: f32,
    #[export]
    pub clutch_wear_rate: f32,

    /// Габариты (ДxШxВ), м.
    #[export]
    pub size: Vector3,
    /// Позиция крепления в кузове (локально).
    #[export]
    pub mount: Vector3,

    base: Base<Resource>,
}

impl EngineResource {
    fn from_spec(base: Base<Resource>, s: &EngineSpec) -> Self {
        let e = &s.engine;
        let (turbo_max_boost, turbo_wastegate, turbo_lag, turbo_efficiency, turbo_intercooler) =
            match e.aspiration {
                Aspiration::Turbo {
                    max_boost,
                    wastegate,
                    lag,
                    efficiency,
                    intercooler,
                } => (max_boost, wastegate, lag, efficiency, intercooler),
                _ => (0.0, 0.0, 0.0, 0.0, 0.0),
            };
        let (super_max_boost, super_intercooler, super_drive_torque) = match e.aspiration {
            Aspiration::Supercharged {
                max_boost,
                intercooler,
                drive_torque,
            } => (max_boost, intercooler, drive_torque),
            _ => (0.0, 0.0, 0.0),
        };
        let aspiration = match e.aspiration {
            Aspiration::NaturallyAspirated => AspirationRes::NaturallyAspirated,
            Aspiration::Turbo { .. } => AspirationRes::Turbo,
            Aspiration::Supercharged { .. } => AspirationRes::Supercharged,
        };
        let ve: Vec<f32> = e
            .volumetric_efficiency
            .iter()
            .flat_map(|(r, v)| [*r, *v])
            .collect();
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            displacement_l: e.displacement_l,
            cylinders: e.cylinders as i64,
            compression_ratio: e.compression_ratio,
            plenum_volume_l: e.plenum_volume_l,
            volumetric_efficiency: pfa(ve),
            aspiration,
            fuel: FuelTypeRes::from_core(e.fuel),
            afr_target: e.afr_target,
            idle_rpm: e.idle_rpm,
            redline_rpm: e.redline_rpm,
            limiter_rpm: e.limiter_rpm,
            inertia: e.inertia,
            friction_constant: e.friction.constant,
            friction_linear: e.friction.linear,
            friction_quadratic: e.friction.quadratic,
            coolant_mass: e.cooling.coolant_mass,
            radiator_rate: e.cooling.radiator_rate,
            thermostat_open_c: e.cooling.thermostat_open_c,
            fan_rate: e.cooling.fan_rate,
            max_temp_c: e.cooling.max_temp_c,
            oil_mass: e.cooling.oil_mass,
            oil_cooler_rate: e.cooling.oil_cooler_rate,
            accessory_torque: e.accessory_torque,
            injector_max_flow: e.injector_max_flow,
            fuel_tank_l: e.fuel_tank_l,
            engine_mass: e.mass,
            turbo_max_boost,
            turbo_wastegate,
            turbo_lag,
            turbo_efficiency,
            turbo_intercooler,
            super_max_boost,
            super_intercooler,
            super_drive_torque,
            clutch_capacity: s.clutch.capacity,
            clutch_stiffness: s.clutch.stiffness,
            clutch_thermal_mass: s.clutch.thermal_mass,
            clutch_fade_temp: s.clutch.fade_temp,
            clutch_wear_rate: s.clutch.wear_rate,
            size: to_godot(s.size),
            mount: to_godot(s.mount),
            base,
        }
    }

    pub fn build_aspiration(&self) -> Aspiration {
        match self.aspiration {
            AspirationRes::NaturallyAspirated => Aspiration::NaturallyAspirated,
            AspirationRes::Turbo => Aspiration::Turbo {
                max_boost: self.turbo_max_boost,
                wastegate: self.turbo_wastegate,
                lag: self.turbo_lag,
                efficiency: self.turbo_efficiency,
                intercooler: self.turbo_intercooler,
            },
            AspirationRes::Supercharged => Aspiration::Supercharged {
                max_boost: self.super_max_boost,
                intercooler: self.super_intercooler,
                drive_torque: self.super_drive_torque,
            },
        }
    }

    pub fn build_engine_config(&self) -> EngineConfig {
        let ve = self
            .volumetric_efficiency
            .as_slice()
            .chunks_exact(2)
            .map(|c| (c[0], c[1]))
            .collect::<Vec<(f32, f32)>>();
        EngineConfig {
            displacement_l: self.displacement_l,
            cylinders: self.cylinders.clamp(0, 255) as u8,
            compression_ratio: self.compression_ratio,
            plenum_volume_l: self.plenum_volume_l,
            volumetric_efficiency: if ve.is_empty() {
                EngineConfig::default().volumetric_efficiency
            } else {
                ve
            },
            aspiration: self.build_aspiration(),
            fuel: self.fuel.to_core(),
            afr_target: self.afr_target,
            idle_rpm: self.idle_rpm,
            redline_rpm: self.redline_rpm,
            limiter_rpm: self.limiter_rpm,
            inertia: self.inertia,
            friction: FrictionModel {
                constant: self.friction_constant,
                linear: self.friction_linear,
                quadratic: self.friction_quadratic,
            },
            cooling: CoolingSpec {
                coolant_mass: self.coolant_mass,
                radiator_rate: self.radiator_rate,
                thermostat_open_c: self.thermostat_open_c,
                fan_rate: self.fan_rate,
                max_temp_c: self.max_temp_c,
                oil_mass: self.oil_mass,
                oil_cooler_rate: self.oil_cooler_rate,
            },
            accessory_torque: self.accessory_torque,
            injector_max_flow: self.injector_max_flow,
            fuel_tank_l: self.fuel_tank_l,
            mass: self.engine_mass,
        }
    }

    pub fn build_clutch(&self) -> ClutchConfig {
        ClutchConfig {
            capacity: self.clutch_capacity,
            stiffness: self.clutch_stiffness,
            thermal_mass: self.clutch_thermal_mass,
            fade_temp: self.clutch_fade_temp,
            wear_rate: self.clutch_wear_rate,
        }
    }

    pub fn build_spec(&self) -> EngineSpec {
        EngineSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            engine: self.build_engine_config(),
            clutch: self.build_clutch(),
            size: to_glam(self.size),
            mount: to_glam(self.mount),
        }
    }
}

#[godot_api]
impl EngineResource {
    /// Оценочный максимальный момент, Н·м.
    #[func]
    pub fn max_torque(&self) -> f32 {
        self.build_engine_config().max_torque()
    }

    /// Оценочная максимальная мощность, кВт.
    #[func]
    pub fn max_power_kw(&self) -> f32 {
        self.build_engine_config().max_power_kw()
    }
}

#[godot_api]
impl IResource for EngineResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::engine_2l_turbo())
    }
}

// ---------------------------------------------------------------------------
// Коробка передач
// ---------------------------------------------------------------------------

/// Коробка передач.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct GearboxResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    /// Передаточные числа вперёд.
    #[export]
    pub ratios: PackedFloat32Array,
    /// Передаточное число заднего хода (отрицательное).
    #[export]
    pub reverse: f32,
    /// Главная пара.
    #[export]
    pub final_drive: f32,
    #[export]
    pub efficiency: f32,
    #[export]
    pub shift_time: f32,
    #[export]
    pub automatic: bool,
    #[export]
    pub upshift_rpm: f32,
    #[export]
    pub downshift_rpm: f32,
    #[export]
    pub kickdown_throttle: f32,
    /// Предельный момент, Н·м.
    #[export]
    pub max_torque: f32,
    /// Приведённая инерция валов, кг·м².
    #[export]
    pub inertia: f32,
    #[export]
    pub reverse_max_speed: f32,

    // --- Гидротрансформатор ---
    #[export]
    pub converter: bool,
    #[export]
    pub converter_stall_ratio: f32,
    #[export]
    pub converter_coupling_ratio: f32,
    #[export]
    pub converter_capacity: f32,
    #[export]
    pub converter_lockup: bool,
    #[export]
    pub converter_lockup_speed: f32,
    #[export]
    pub converter_lockup_capacity: f32,

    #[export]
    pub layout: DriveLayoutRes,
    /// Доля момента на переднюю ось для полного привода.
    #[export]
    pub front_bias: f32,

    #[export]
    pub size: Vector3,
    #[export]
    pub mount: Vector3,

    base: Base<Resource>,
}

impl GearboxResource {
    fn from_spec(base: Base<Resource>, s: &GearboxSpec) -> Self {
        let t = &s.transmission;
        let c = t.converter.unwrap_or_default();
        let (layout, front_bias) = match s.layout {
            DriveLayout::Rwd => (DriveLayoutRes::Rwd, 0.0),
            DriveLayout::Fwd => (DriveLayoutRes::Fwd, 1.0),
            DriveLayout::Awd { front_bias } => (DriveLayoutRes::Awd, front_bias),
        };
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            ratios: pfa(t.ratios.iter().copied()),
            reverse: t.reverse,
            final_drive: t.final_drive,
            efficiency: t.efficiency,
            shift_time: t.shift_time,
            automatic: t.automatic,
            upshift_rpm: t.upshift_rpm,
            downshift_rpm: t.downshift_rpm,
            kickdown_throttle: t.kickdown_throttle,
            max_torque: t.max_torque,
            inertia: t.inertia,
            reverse_max_speed: t.reverse_max_speed,
            converter: t.converter.is_some(),
            converter_stall_ratio: c.stall_ratio,
            converter_coupling_ratio: c.coupling_ratio,
            converter_capacity: c.capacity,
            converter_lockup: c.lockup,
            converter_lockup_speed: c.lockup_speed,
            converter_lockup_capacity: c.lockup_capacity,
            layout,
            front_bias,
            size: to_godot(s.size),
            mount: to_godot(s.mount),
            base,
        }
    }

    pub fn build_layout(&self) -> DriveLayout {
        match self.layout {
            DriveLayoutRes::Rwd => DriveLayout::Rwd,
            DriveLayoutRes::Fwd => DriveLayout::Fwd,
            DriveLayoutRes::Awd => DriveLayout::Awd {
                front_bias: self.front_bias.clamp(0.0, 1.0),
            },
        }
    }

    pub fn build_transmission(&self) -> TransmissionConfig {
        TransmissionConfig {
            ratios: self.ratios.as_slice().to_vec(),
            reverse: self.reverse,
            final_drive: self.final_drive,
            efficiency: self.efficiency,
            shift_time: self.shift_time,
            automatic: self.automatic,
            upshift_rpm: self.upshift_rpm,
            downshift_rpm: self.downshift_rpm,
            kickdown_throttle: self.kickdown_throttle,
            max_torque: self.max_torque,
            inertia: self.inertia,
            converter: if self.converter {
                Some(TorqueConverterConfig {
                    stall_ratio: self.converter_stall_ratio,
                    coupling_ratio: self.converter_coupling_ratio,
                    capacity: self.converter_capacity,
                    lockup: self.converter_lockup,
                    lockup_speed: self.converter_lockup_speed,
                    lockup_capacity: self.converter_lockup_capacity,
                })
            } else {
                None
            },
            reverse_max_speed: self.reverse_max_speed,
        }
    }

    pub fn build_spec(&self) -> GearboxSpec {
        GearboxSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            transmission: self.build_transmission(),
            layout: self.build_layout(),
            size: to_glam(self.size),
            mount: to_glam(self.mount),
        }
    }
}

#[godot_api]
impl IResource for GearboxResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::gearbox_6mt_rwd())
    }
}

// ---------------------------------------------------------------------------
// Дифференциал
// ---------------------------------------------------------------------------

/// Дифференциал (межколёсный или центральный).
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct DifferentialResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    #[export]
    pub kind: DifferentialKindRes,
    /// Преднатяжение, Н·м.
    #[export]
    pub preload: f32,
    /// Жёсткость связи, Н·м·с/рад.
    #[export]
    pub coupling: f32,
    /// Доля момента блокировки.
    #[export]
    pub bias_ratio: f32,
    /// Предельный момент, Н·м.
    #[export]
    pub max_torque: f32,

    base: Base<Resource>,
}

impl DifferentialResource {
    fn from_spec(base: Base<Resource>, s: &vehicle_core::parts::DifferentialSpec) -> Self {
        let (kind, preload, coupling, bias_ratio) = match s.kind {
            DifferentialKind::Open => (DifferentialKindRes::Open, 0.0, 0.0, 0.0),
            DifferentialKind::Locked => (DifferentialKindRes::Locked, 0.0, 0.0, 0.0),
            DifferentialKind::Lsd {
                preload,
                coupling,
                bias_ratio,
            } => (DifferentialKindRes::Lsd, preload, coupling, bias_ratio),
            DifferentialKind::Torsen {
                bias_ratio,
                preload,
            } => (DifferentialKindRes::Torsen, preload, 0.0, bias_ratio),
        };
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            kind,
            preload,
            coupling,
            bias_ratio,
            max_torque: s.max_torque,
            base,
        }
    }

    pub fn build_kind(&self) -> DifferentialKind {
        match self.kind {
            DifferentialKindRes::Open => DifferentialKind::Open,
            DifferentialKindRes::Locked => DifferentialKind::Locked,
            DifferentialKindRes::Lsd => DifferentialKind::Lsd {
                preload: self.preload,
                coupling: self.coupling,
                bias_ratio: self.bias_ratio,
            },
            DifferentialKindRes::Torsen => DifferentialKind::Torsen {
                bias_ratio: self.bias_ratio,
                preload: self.preload,
            },
        }
    }

    pub fn build_spec(&self) -> vehicle_core::parts::DifferentialSpec {
        vehicle_core::parts::DifferentialSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            kind: self.build_kind(),
            max_torque: self.max_torque,
        }
    }
}

#[godot_api]
impl IResource for DifferentialResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::diff_lsd())
    }
}

// ---------------------------------------------------------------------------
// Подвеска
// ---------------------------------------------------------------------------

/// Подвеска одного угла.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct SuspensionResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    #[export]
    pub kind: SuspensionKindRes,
    #[export]
    pub rest_length: f32,
    #[export]
    pub max_length: f32,
    #[export]
    pub min_length: f32,
    #[export]
    pub spring_rate: f32,
    #[export]
    pub damper_compression: f32,
    #[export]
    pub damper_rebound: f32,
    #[export]
    pub bump_stop_rate: f32,
    #[export]
    pub anti_roll: f32,
    #[export]
    pub camber_gain: f32,
    #[export]
    pub toe_gain: f32,
    /// Статический развал, градусы.
    #[export]
    pub camber_deg: f32,
    /// Статическое схождение, градусы.
    #[export]
    pub toe_deg: f32,
    /// Максимальный угол поворота (для рулевых), градусы.
    #[export]
    pub max_steer_deg: f32,

    base: Base<Resource>,
}

impl SuspensionResource {
    pub(crate) fn from_spec(base: Base<Resource>, s: &SuspensionSpec) -> Self {
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            kind: SuspensionKindRes::from_core(s.kind),
            rest_length: s.suspension.rest_length,
            max_length: s.suspension.max_length,
            min_length: s.suspension.min_length,
            spring_rate: s.suspension.spring_rate,
            damper_compression: s.suspension.damper_compression,
            damper_rebound: s.suspension.damper_rebound,
            bump_stop_rate: s.suspension.bump_stop_rate,
            anti_roll: s.suspension.anti_roll,
            camber_gain: s.suspension.camber_gain,
            toe_gain: s.suspension.toe_gain,
            camber_deg: s.camber_deg,
            toe_deg: s.toe_deg,
            max_steer_deg: s.max_steer_deg,
            base,
        }
    }

    pub fn build_spec(&self) -> SuspensionSpec {
        SuspensionSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            kind: self.kind.to_core(),
            suspension: SuspensionConfig {
                rest_length: self.rest_length,
                max_length: self.max_length,
                min_length: self.min_length,
                spring_rate: self.spring_rate,
                damper_compression: self.damper_compression,
                damper_rebound: self.damper_rebound,
                bump_stop_rate: self.bump_stop_rate,
                anti_roll: self.anti_roll,
                camber_gain: self.camber_gain,
                toe_gain: self.toe_gain,
            },
            camber_deg: self.camber_deg,
            toe_deg: self.toe_deg,
            max_steer_deg: self.max_steer_deg,
        }
    }
}

#[godot_api]
impl IResource for SuspensionResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::suspension_sport())
    }
}

// ---------------------------------------------------------------------------
// Диск
// ---------------------------------------------------------------------------

/// Диск колеса.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct RimResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    /// Диаметр, дюймы.
    #[export]
    pub diameter_in: f32,
    /// Ширина, дюймы.
    #[export]
    pub width_in: f32,
    /// Вылет (ET), мм.
    #[export]
    pub offset_mm: f32,
    #[export]
    pub bolts: i64,
    #[export]
    pub pcd_mm: i64,

    base: Base<Resource>,
}

impl RimResource {
    pub(crate) fn from_spec(base: Base<Resource>, s: &RimSpec) -> Self {
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            diameter_in: s.diameter_in,
            width_in: s.width_in,
            offset_mm: s.offset_mm,
            bolts: s.bolt_pattern.bolts as i64,
            pcd_mm: s.bolt_pattern.pcd_mm as i64,
            base,
        }
    }

    pub fn build_spec(&self) -> RimSpec {
        RimSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            diameter_in: self.diameter_in,
            width_in: self.width_in,
            offset_mm: self.offset_mm,
            bolt_pattern: BoltPattern {
                bolts: self.bolts.clamp(0, 255) as u8,
                pcd_mm: self.pcd_mm.max(0) as u32,
            },
        }
    }
}

#[godot_api]
impl IResource for RimResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::rim_18())
    }
}

// ---------------------------------------------------------------------------
// Шина
// ---------------------------------------------------------------------------

/// Шина.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct TireResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    #[export]
    pub friction: f32,
    #[export]
    pub long_stiffness: f32,
    #[export]
    pub long_shape: f32,
    #[export]
    pub long_curvature: f32,
    #[export]
    pub lat_stiffness: f32,
    #[export]
    pub lat_shape: f32,
    #[export]
    pub lat_curvature: f32,
    #[export]
    pub load_sensitivity: f32,
    #[export]
    pub reference_load: f32,
    #[export]
    pub relaxation_length: f32,
    #[export]
    pub camber_stiffness: f32,
    #[export]
    pub rolling_resistance: f32,
    #[export]
    pub pneumatic_trail: f32,
    /// Радиус, м.
    #[export]
    pub radius: f32,
    /// Ширина, м.
    #[export]
    pub width: f32,
    /// Масса шины, кг.
    #[export]
    pub tire_mass: f32,
    #[export]
    pub wear_rate: f32,
    #[export]
    pub optimal_temperature: f32,
    /// Индекс нагрузки (справочно).
    #[export]
    pub load_index: i64,
    /// Индекс скорости, км/ч (справочно).
    #[export]
    pub speed_rating: f32,

    base: Base<Resource>,
}

impl TireResource {
    pub(crate) fn from_spec(base: Base<Resource>, s: &TireSpec) -> Self {
        let t = &s.tire;
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            friction: t.friction,
            long_stiffness: t.long_stiffness,
            long_shape: t.long_shape,
            long_curvature: t.long_curvature,
            lat_stiffness: t.lat_stiffness,
            lat_shape: t.lat_shape,
            lat_curvature: t.lat_curvature,
            load_sensitivity: t.load_sensitivity,
            reference_load: t.reference_load,
            relaxation_length: t.relaxation_length,
            camber_stiffness: t.camber_stiffness,
            rolling_resistance: t.rolling_resistance,
            pneumatic_trail: t.pneumatic_trail,
            radius: t.radius,
            width: t.width,
            tire_mass: t.mass,
            wear_rate: t.wear_rate,
            optimal_temperature: t.optimal_temperature,
            load_index: s.load_index as i64,
            speed_rating: s.speed_rating,
            base,
        }
    }

    pub fn build_config(&self) -> TireConfig {
        TireConfig {
            friction: self.friction,
            long_stiffness: self.long_stiffness,
            long_shape: self.long_shape,
            long_curvature: self.long_curvature,
            lat_stiffness: self.lat_stiffness,
            lat_shape: self.lat_shape,
            lat_curvature: self.lat_curvature,
            load_sensitivity: self.load_sensitivity,
            reference_load: self.reference_load,
            relaxation_length: self.relaxation_length,
            camber_stiffness: self.camber_stiffness,
            rolling_resistance: self.rolling_resistance,
            pneumatic_trail: self.pneumatic_trail,
            radius: self.radius,
            width: self.width,
            mass: self.tire_mass,
            wear_rate: self.wear_rate,
            optimal_temperature: self.optimal_temperature,
        }
    }

    pub fn build_spec(&self) -> TireSpec {
        TireSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            tire: self.build_config(),
            load_index: self.load_index.clamp(0, 65535) as u16,
            speed_rating: self.speed_rating,
        }
    }
}

#[godot_api]
impl IResource for TireResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::tire_235())
    }
}

// ---------------------------------------------------------------------------
// Тормоз
// ---------------------------------------------------------------------------

/// Тормоз одного колеса.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct BrakeResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    /// Диаметр диска, м.
    #[export]
    pub disc_diameter: f32,
    /// Толщина диска, м.
    #[export]
    pub disc_thickness: f32,
    /// Число поршней.
    #[export]
    pub pistons: i64,
    /// Трение колодок.
    #[export]
    pub pad_mu: f32,
    #[export]
    pub vented: bool,

    base: Base<Resource>,
}

impl BrakeResource {
    pub(crate) fn from_spec(base: Base<Resource>, s: &BrakeSpec) -> Self {
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            disc_diameter: s.disc_diameter,
            disc_thickness: s.disc_thickness,
            pistons: s.pistons as i64,
            pad_mu: s.pad_mu,
            vented: s.vented,
            base,
        }
    }

    pub fn build_spec(&self) -> BrakeSpec {
        BrakeSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            disc_diameter: self.disc_diameter,
            disc_thickness: self.disc_thickness,
            pistons: self.pistons.clamp(0, 255) as u8,
            pad_mu: self.pad_mu,
            vented: self.vented,
        }
    }
}

#[godot_api]
impl BrakeResource {
    /// Оценка максимального тормозного момента, Н·м.
    #[func]
    pub fn max_torque(&self) -> f32 {
        self.build_spec().max_torque()
    }
}

#[godot_api]
impl IResource for BrakeResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::brake_340())
    }
}

// ---------------------------------------------------------------------------
// Кузов
// ---------------------------------------------------------------------------

/// Кузовная панель / обвес.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct BodyResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    /// Коэффициент лобового сопротивления.
    #[export]
    pub drag_cd: f32,
    /// Площадь миделя, м².
    #[export]
    pub frontal_area: f32,
    /// Прижимная сила кузова (ClA).
    #[export]
    pub downforce: f32,
    #[export]
    pub material: BodyMaterialRes,

    base: Base<Resource>,
}

impl BodyResource {
    fn from_spec(base: Base<Resource>, s: &BodySpec) -> Self {
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            drag_cd: s.drag_cd,
            frontal_area: s.frontal_area,
            downforce: s.downforce,
            material: BodyMaterialRes::from_core(s.material),
            base,
        }
    }

    pub fn build_spec(&self) -> BodySpec {
        BodySpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            drag_cd: self.drag_cd,
            frontal_area: self.frontal_area,
            downforce: self.downforce,
            material: self.material.to_core(),
        }
    }
}

#[godot_api]
impl IResource for BodyResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_spec(base, &catalog::body_coupe())
    }
}

// ---------------------------------------------------------------------------
// Аэродинамика
// ---------------------------------------------------------------------------

/// Аэродинамическая деталь (сплиттер/крыло/диффузор).
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct AeroResource {
    #[export]
    pub part_id: GString,
    #[export]
    pub part_name: GString,
    #[export]
    pub manufacturer: GString,
    #[export]
    pub mass: f32,
    #[export]
    pub price: f32,
    #[export]
    pub condition: f32,

    /// Прибавка лобового сопротивления (ΔCdA).
    #[export]
    pub drag_delta: f32,
    /// Прибавка прижимной силы (ΔClA).
    #[export]
    pub downforce_delta: f32,
    /// Позиция (локально).
    #[export]
    pub mount: Vector3,

    base: Base<Resource>,
}

impl AeroResource {
    fn from_spec(base: Base<Resource>, s: &AeroSpec) -> Self {
        Self {
            part_id: GString::from(s.meta.id.as_str()),
            part_name: GString::from(s.meta.name.as_str()),
            manufacturer: GString::from(s.meta.manufacturer.as_str()),
            mass: s.meta.mass,
            price: s.meta.price,
            condition: s.meta.condition,
            drag_delta: s.drag_delta,
            downforce_delta: s.downforce_delta,
            mount: to_godot(s.mount),
            base,
        }
    }

    pub fn build_spec(&self) -> AeroSpec {
        AeroSpec {
            meta: make_meta(
                &self.part_id,
                &self.part_name,
                &self.manufacturer,
                self.mass,
                self.price,
                self.condition,
            ),
            drag_delta: self.drag_delta,
            downforce_delta: self.downforce_delta,
            mount: to_glam(self.mount),
        }
    }
}

#[godot_api]
impl IResource for AeroResource {
    fn init(base: Base<Resource>) -> Self {
        let spec = AeroSpec {
            meta: PartMeta::new("aero_wing", "Rear Wing", 8.0),
            drag_delta: 0.08,
            downforce_delta: 0.9,
            mount: Vec3::new(0.0, 0.9, 1.6),
        };
        Self::from_spec(base, &spec)
    }
}

// ---------------------------------------------------------------------------
// Колесо в сборе
// ---------------------------------------------------------------------------

/// Колесо в сборе: подвеска + диск + шина + тормоз.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct WheelResource {
    #[export]
    pub suspension: Option<Gd<SuspensionResource>>,
    #[export]
    pub rim: Option<Gd<RimResource>>,
    #[export]
    pub tire: Option<Gd<TireResource>>,
    #[export]
    pub brake: Option<Gd<BrakeResource>>,

    base: Base<Resource>,
}

impl WheelResource {
    pub(crate) fn new_with(
        base: Base<Resource>,
        suspension: Option<Gd<SuspensionResource>>,
        rim: Option<Gd<RimResource>>,
        tire: Option<Gd<TireResource>>,
        brake: Option<Gd<BrakeResource>>,
    ) -> Self {
        Self {
            suspension,
            rim,
            tire,
            brake,
            base,
        }
    }

    /// Пустое колесо без компонентов.
    pub(crate) fn empty(base: Base<Resource>) -> Self {
        Self {
            suspension: None,
            rim: None,
            tire: None,
            brake: None,
            base,
        }
    }
}

#[godot_api]
impl IResource for WheelResource {
    fn init(base: Base<Resource>) -> Self {
        Self::new_with(base, None, None, None, None)
    }
}

// ---------------------------------------------------------------------------
// Конструкторы ресурсов из ядра (для каталога и сборок)
// ---------------------------------------------------------------------------

/// Создаёт ресурс шасси из спецификации ядра.
pub fn chassis_from_spec(s: &ChassisSpec) -> Gd<ChassisResource> {
    Gd::from_init_fn(|base| ChassisResource::from_spec(base, s))
}

/// Создаёт ресурс двигателя из спецификации ядра.
pub fn engine_from_spec(s: &EngineSpec) -> Gd<EngineResource> {
    Gd::from_init_fn(|base| EngineResource::from_spec(base, s))
}

/// Создаёт ресурс коробки передач из спецификации ядра.
pub fn gearbox_from_spec(s: &GearboxSpec) -> Gd<GearboxResource> {
    Gd::from_init_fn(|base| GearboxResource::from_spec(base, s))
}

/// Создаёт ресурс дифференциала из спецификации ядра.
pub fn differential_from_spec(s: &vehicle_core::parts::DifferentialSpec) -> Gd<DifferentialResource> {
    Gd::from_init_fn(|base| DifferentialResource::from_spec(base, s))
}

/// Создаёт ресурс подвески из спецификации ядра.
pub fn suspension_from_spec(s: &SuspensionSpec) -> Gd<SuspensionResource> {
    Gd::from_init_fn(|base| SuspensionResource::from_spec(base, s))
}

/// Создаёт ресурс диска из спецификации ядра.
pub fn rim_from_spec(s: &RimSpec) -> Gd<RimResource> {
    Gd::from_init_fn(|base| RimResource::from_spec(base, s))
}

/// Создаёт ресурс шины из спецификации ядра.
pub fn tire_from_spec(s: &TireSpec) -> Gd<TireResource> {
    Gd::from_init_fn(|base| TireResource::from_spec(base, s))
}

/// Создаёт ресурс тормоза из спецификации ядра.
pub fn brake_from_spec(s: &BrakeSpec) -> Gd<BrakeResource> {
    Gd::from_init_fn(|base| BrakeResource::from_spec(base, s))
}

/// Создаёт ресурс кузова из спецификации ядра.
pub fn body_from_spec(s: &BodySpec) -> Gd<BodyResource> {
    Gd::from_init_fn(|base| BodyResource::from_spec(base, s))
}

/// Создаёт ресурс аэродинамики из спецификации ядра.
pub fn aero_from_spec(s: &AeroSpec) -> Gd<AeroResource> {
    Gd::from_init_fn(|base| AeroResource::from_spec(base, s))
}

/// Создаёт ресурс колеса из спецификаций ядра.
pub fn wheel_from_specs(
    suspension: Option<&SuspensionSpec>,
    rim: Option<&RimSpec>,
    tire: Option<&TireSpec>,
    brake: Option<&BrakeSpec>,
) -> Gd<WheelResource> {
    Gd::from_init_fn(|base| {
        WheelResource::new_with(
            base,
            suspension.map(suspension_from_spec),
            rim.map(rim_from_spec),
            tire.map(tire_from_spec),
            brake.map(brake_from_spec),
        )
    })
}
