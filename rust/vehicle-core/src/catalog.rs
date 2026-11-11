//! Готовые детали и примеры сборок — для тестов, демо и как образец
//! заполнения каталога.

use glam::Vec3;

use crate::assembly::{CarAssembly, WheelAssembly};
use crate::parts::*;
use crate::powertrain::differential::DifferentialKind;
use crate::powertrain::engine::{Aspiration, CoolingSpec, EngineConfig, FrictionModel, FuelType};
use crate::powertrain::transmission::{TorqueConverterConfig, TransmissionConfig};
use crate::powertrain::ClutchConfig;
use crate::suspension::SuspensionConfig;
use crate::tire::TireConfig;

/// Спортивный рядный четырёхцилиндровый турбомотор 2.0 л.
pub fn engine_2l_turbo() -> EngineSpec {
    EngineSpec {
        meta: PartMeta::new("i4_2.0t", "2.0 Turbo I4", 160.0),
        engine: EngineConfig {
            displacement_l: 2.0,
            cylinders: 4,
            compression_ratio: 9.5,
            plenum_volume_l: 2.4,
            volumetric_efficiency: vec![
                (800.0, 0.70),
                (1500.0, 0.86),
                (2500.0, 0.94),
                (3500.0, 0.97),
                (4500.0, 0.96),
                (5500.0, 0.92),
                (6500.0, 0.84),
                (7000.0, 0.72),
            ],
            aspiration: Aspiration::Turbo {
                max_boost: 0.95e5,
                wastegate: 0.85e5,
                lag: 0.22,
                efficiency: 0.76,
                intercooler: 0.72,
            },
            fuel: FuelType::Gasoline98,
            afr_target: 12.4,
            idle_rpm: 850.0,
            redline_rpm: 6500.0,
            limiter_rpm: 6800.0,
            inertia: 0.22,
            friction: FrictionModel::default(),
            cooling: CoolingSpec::default(),
            accessory_torque: 6.0,
            injector_max_flow: 0.10,
            fuel_tank_l: 55.0,
            mass: 160.0,
        },
        clutch: ClutchConfig {
            capacity: 480.0,
            stiffness: 65.0,
            ..ClutchConfig::default()
        },
        size: Vec3::new(0.62, 0.68, 0.52),
        mount: Vec3::new(0.0, 0.45, -1.1),
    }
}

/// Атмосферный V8 4.0 л.
pub fn engine_v8_na() -> EngineSpec {
    EngineSpec {
        meta: PartMeta::new("v8_4.0", "4.0 V8", 210.0),
        engine: EngineConfig {
            displacement_l: 4.0,
            cylinders: 8,
            compression_ratio: 11.0,
            plenum_volume_l: 4.0,
            volumetric_efficiency: vec![
                (800.0, 0.72),
                (2000.0, 0.90),
                (3500.0, 0.96),
                (5000.0, 0.95),
                (6500.0, 0.88),
                (7500.0, 0.78),
            ],
            aspiration: Aspiration::NaturallyAspirated,
            fuel: FuelType::Gasoline98,
            afr_target: 12.8,
            idle_rpm: 750.0,
            redline_rpm: 7200.0,
            limiter_rpm: 7500.0,
            inertia: 0.32,
            friction: FrictionModel {
                constant: 16.0,
                linear: 0.08,
                quadratic: 0.00028,
            },
            cooling: CoolingSpec::default(),
            accessory_torque: 8.0,
            injector_max_flow: 0.14,
            fuel_tank_l: 70.0,
            mass: 210.0,
        },
        clutch: ClutchConfig {
            capacity: 650.0,
            stiffness: 80.0,
            ..ClutchConfig::default()
        },
        size: Vec3::new(0.72, 0.74, 0.60),
        mount: Vec3::new(0.0, 0.45, -1.1),
    }
}

/// Шестиступенчатая механика, задний привод.
pub fn gearbox_6mt_rwd() -> GearboxSpec {
    GearboxSpec {
        meta: PartMeta::new("6mt_rwd", "6MT RWD", 48.0),
        transmission: TransmissionConfig {
            ratios: vec![3.6, 2.19, 1.54, 1.19, 1.0, 0.85],
            reverse: -3.4,
            final_drive: 3.9,
            efficiency: 0.93,
            shift_time: 0.28,
            automatic: false,
            max_torque: 520.0,
            ..TransmissionConfig::default()
        },
        layout: DriveLayout::Rwd,
        size: Vec3::new(0.40, 0.45, 0.65),
        mount: Vec3::new(0.0, 0.35, -0.2),
    }
}

/// Шестиступенчатый автомат с гидротрансформатором, полный привод.
pub fn gearbox_6at_awd(front_bias: f32) -> GearboxSpec {
    GearboxSpec {
        meta: PartMeta::new("6at_awd", "6AT AWD", 78.0),
        transmission: TransmissionConfig {
            ratios: vec![4.2, 2.5, 1.7, 1.3, 1.0, 0.8],
            reverse: -3.8,
            final_drive: 3.7,
            efficiency: 0.90,
            shift_time: 0.35,
            automatic: true,
            upshift_rpm: 6300.0,
            downshift_rpm: 2200.0,
            kickdown_throttle: 0.7,
            max_torque: 800.0,
            converter: Some(TorqueConverterConfig::default()),
            ..TransmissionConfig::default()
        },
        layout: DriveLayout::Awd { front_bias },
        size: Vec3::new(0.46, 0.5, 0.72),
        mount: Vec3::new(0.0, 0.35, -0.1),
    }
}

/// Самоблокирующийся дифференциал.
pub fn diff_lsd() -> DifferentialSpec {
    DifferentialSpec {
        meta: PartMeta::new("lsd_1.5way", "LSD 1.5-way", 14.0),
        kind: DifferentialKind::Lsd {
            preload: 45.0,
            coupling: 280.0,
            bias_ratio: 0.5,
        },
        max_torque: 600.0,
    }
}

/// Центральный дифференциал для полного привода.
pub fn diff_center_lsd() -> DifferentialSpec {
    DifferentialSpec {
        meta: PartMeta::new("center_lsd", "Central LSD", 22.0),
        kind: DifferentialKind::Lsd {
            preload: 60.0,
            coupling: 200.0,
            bias_ratio: 0.4,
        },
        max_torque: 700.0,
    }
}

/// Шасси спортивного купе.
pub fn chassis_coupe() -> ChassisSpec {
    ChassisSpec {
        meta: PartMeta::new("coupe", "Coupe Chassis", 1150.0),
        wheelbase: 2.62,
        track_front: 1.57,
        track_rear: 1.59,
        com_height: 0.50,
        com_offset: 0.05,
        arch_radius: 0.35,
        hub_bolt_pattern: BoltPattern {
            bolts: 5,
            pcd_mm: 114,
        },
        max_brake_disc: 0.37,
        torsional_stiffness: 8.0,
        engine_bay: Vec3::new(0.90, 0.80, 0.70),
        material: BodyMaterial::Steel,
    }
}

/// Подвеска типа McPherson с отрицательным развалом.
pub fn suspension_sport() -> SuspensionSpec {
    SuspensionSpec {
        meta: PartMeta::new("sport_susp", "Sport Suspension", 32.0),
        kind: SuspensionKind::MacPherson,
        suspension: SuspensionConfig {
            rest_length: 0.34,
            max_length: 0.46,
            min_length: 0.12,
            spring_rate: 42000.0,
            damper_compression: 3600.0,
            damper_rebound: 5200.0,
            bump_stop_rate: 220000.0,
            anti_roll: 9000.0,
            camber_gain: -2.4,
            toe_gain: 0.0,
        },
        camber_deg: -1.5,
        toe_deg: 0.08,
        max_steer_deg: 38.0,
    }
}

/// Диск 18×8.5.
pub fn rim_18() -> RimSpec {
    RimSpec {
        meta: PartMeta::new("rim18", "18x8.5 Rim", 11.0),
        diameter_in: 18.0,
        width_in: 8.5,
        offset_mm: 45.0,
        bolt_pattern: BoltPattern {
            bolts: 5,
            pcd_mm: 114,
        },
    }
}

/// Шина 235/40R18.
pub fn tire_235() -> TireSpec {
    TireSpec {
        meta: PartMeta::new("tire235", "235/40R18 Sport", 11.0),
        tire: TireConfig {
            radius: 0.325,
            width: 0.235,
            ..TireConfig::default()
        },
        load_index: 95,
        speed_rating: 270.0,
    }
}

/// Тормоз 340 мм, 4 поршня.
pub fn brake_340() -> BrakeSpec {
    BrakeSpec {
        meta: PartMeta::new("brake340", "340mm 4-pot", 9.0),
        disc_diameter: 0.34,
        disc_thickness: 0.028,
        pistons: 4,
        pad_mu: 0.44,
        vented: true,
    }
}

/// Кузов купе.
pub fn body_coupe() -> BodySpec {
    BodySpec {
        meta: PartMeta::new("body_coupe", "Coupe Body", 320.0),
        drag_cd: 0.32,
        frontal_area: 2.05,
        downforce: 0.6,
        material: BodyMaterial::Steel,
    }
}

/// Собирает заднеприводное спорткупе.
pub fn sample_rwd_coupe() -> CarAssembly {
    let mut car = CarAssembly {
        name: "Sport Coupe RWD".to_string(),
        chassis: Some(chassis_coupe()),
        engine: Some(engine_2l_turbo()),
        gearbox: Some(gearbox_6mt_rwd()),
        differential: Some(diff_lsd()),
        body: Some(body_coupe()),
        ..Default::default()
    };
    for corner in Corner::all() {
        car.wheels.push(WheelAssembly::new(corner).with(
            suspension_sport(),
            rim_18(),
            tire_235(),
            brake_340(),
        ));
    }
    car
}

/// Собирает полноприводный автомобиль с автоматом.
pub fn sample_awd_auto() -> CarAssembly {
    let mut car = CarAssembly {
        name: "AWD Auto".to_string(),
        chassis: Some(chassis_coupe()),
        engine: Some(engine_v8_na()),
        gearbox: Some(gearbox_6at_awd(0.4)),
        differential: Some(diff_lsd()),
        center_differential: Some(diff_center_lsd()),
        body: Some(body_coupe()),
        ..Default::default()
    };
    for corner in Corner::all() {
        car.wheels.push(WheelAssembly::new(corner).with(
            suspension_sport(),
            rim_18(),
            tire_235(),
            brake_340(),
        ));
    }
    car
}
