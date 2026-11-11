//! Сборка автомобиля из деталей: проверка совместимости и вывод
//! [`VehicleConfig`] для симуляции.
//!
//! Локальная система координат машины: начало — на земле по центру базы,
//! `-Z` — вперёд, `+Y` — вверх (как в Godot).

use glam::Vec3;

use crate::parts::*;
use crate::powertrain::differential::DifferentialKind;
use crate::powertrain::PowertrainConfig;
use crate::suspension::SuspensionConfig;
use crate::tire::{wheel_inertia, TireConfig};

/// Установленное колесо в конкретном углу.
#[derive(Clone, Debug)]
pub struct WheelAssembly {
    pub corner: Corner,
    pub rim: Option<RimSpec>,
    pub tire: Option<TireSpec>,
    pub brake: Option<BrakeSpec>,
    pub suspension: Option<SuspensionSpec>,
}

impl WheelAssembly {
    pub fn new(corner: Corner) -> Self {
        Self {
            corner,
            rim: None,
            tire: None,
            brake: None,
            suspension: None,
        }
    }

    pub fn with(
        mut self,
        suspension: SuspensionSpec,
        rim: RimSpec,
        tire: TireSpec,
        brake: BrakeSpec,
    ) -> Self {
        self.suspension = Some(suspension);
        self.rim = Some(rim);
        self.tire = Some(tire);
        self.brake = Some(brake);
        self
    }
}

/// Полная сборка машины.
#[derive(Clone, Debug, Default)]
pub struct CarAssembly {
    pub name: String,
    pub chassis: Option<ChassisSpec>,
    pub engine: Option<EngineSpec>,
    pub gearbox: Option<GearboxSpec>,
    /// Межколёсный дифференциал ведущей оси.
    pub differential: Option<DifferentialSpec>,
    /// Центральный дифференциал (для полного привода).
    pub center_differential: Option<DifferentialSpec>,
    pub body: Option<BodySpec>,
    pub aero: Vec<AeroSpec>,
    pub wheels: Vec<WheelAssembly>,
}

/// Важность проблемы сборки.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Проблема совместимости деталей.
#[derive(Clone, Debug)]
pub struct AssemblyIssue {
    pub severity: Severity,
    pub message: String,
}

impl AssemblyIssue {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
        }
    }
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
        }
    }
}

/// Конфигурация одного колеса для симуляции.
#[derive(Clone, Debug)]
pub struct WheelConfig {
    pub corner: Corner,
    /// Позиция точки крепления подвески (локально).
    pub position: Vec3,
    pub steering: bool,
    pub driven: bool,
    pub handbrake: bool,
    pub max_steer_deg: f32,
    pub static_camber_rad: f32,
    pub static_toe_rad: f32,
    pub suspension: SuspensionConfig,
    pub tire: TireConfig,
    pub brake_torque: f32,
    pub wheel_mass: f32,
    pub inertia: f32,
}

impl WheelConfig {
    /// Роль колеса в трансмиссии.
    pub fn role(&self) -> crate::powertrain::WheelRole {
        crate::powertrain::WheelRole {
            driven: self.driven,
            handbrake: self.handbrake,
            front: self.corner.is_front(),
            brake: crate::powertrain::BrakeWheel {
                max_torque: self.brake_torque,
                handbrake: self.handbrake,
            },
        }
    }
}

/// Конфигурация аэродинамики.
#[derive(Clone, Debug, Default)]
pub struct AeroConfig {
    /// CdA (м²).
    pub drag_area: f32,
    /// ClA (м²).
    pub downforce_area: f32,
    pub frontal_area: f32,
    /// Точки приложения прижимной силы (локально) и её доли.
    pub downforce_points: Vec<(Vec3, f32)>,
}

/// Полная конфигурация симуляции, выведенная из сборки.
#[derive(Clone, Debug)]
pub struct VehicleConfig {
    pub mass: f32,
    /// Диагональ тензора инерции (локально).
    pub inertia: Vec3,
    /// Центр масс (локально).
    pub center_of_mass: Vec3,
    pub powertrain: PowertrainConfig,
    pub wheels: Vec<WheelConfig>,
    pub aero: AeroConfig,
    pub grip_multiplier: f32,
    pub front_drive_bias: f32,
}

impl CarAssembly {
    /// Проверяет совместимость деталей.
    pub fn validate(&self) -> Vec<AssemblyIssue> {
        let mut issues = Vec::new();

        let Some(chassis) = &self.chassis else {
            issues.push(AssemblyIssue::error("Не выбрано шасси"));
            return issues;
        };
        if self.engine.is_none() {
            issues.push(AssemblyIssue::error("Не установлен двигатель"));
        }
        if self.gearbox.is_none() {
            issues.push(AssemblyIssue::error("Не установлена коробка передач"));
        }

        if let (Some(engine), Some(gearbox)) = (&self.engine, &self.gearbox) {
            let t = engine.engine.max_torque();
            if gearbox.transmission.max_torque < t {
                issues.push(AssemblyIssue::warning(format!(
                    "Коробка передач рассчитана на {:.0} Н·м, а двигатель даёт {:.0} Н·м — возможны поломки",
                    gearbox.transmission.max_torque, t
                )));
            }
            if engine.size.x > chassis.engine_bay.x
                || engine.size.y > chassis.engine_bay.y
                || engine.size.z > chassis.engine_bay.z
            {
                issues.push(AssemblyIssue::warning(format!(
                    "Двигатель {:.2}×{:.2}×{:.2} м не влезает в моторный отсек {:.2}×{:.2}×{:.2} м",
                    engine.size.x, engine.size.y, engine.size.z,
                    chassis.engine_bay.x, chassis.engine_bay.y, chassis.engine_bay.z
                )));
            }
        }

        if let Some(diff) = &self.differential {
            if let Some(engine) = &self.engine {
                if diff.max_torque < engine.engine.max_torque() {
                    issues.push(AssemblyIssue::warning(
                        "Дифференциал не рассчитан на момент двигателя",
                    ));
                }
            }
        }

        if self.wheels.is_empty() {
            issues.push(AssemblyIssue::error("Не установлено ни одного колеса"));
        }

        for wheel in &self.wheels {
            let corner = wheel.corner;
            if wheel.rim.is_none() {
                issues.push(AssemblyIssue::error(format!("{corner:?}: нет диска")));
            }
            if wheel.tire.is_none() {
                issues.push(AssemblyIssue::error(format!("{corner:?}: нет шины")));
            }
            if wheel.suspension.is_none() {
                issues.push(AssemblyIssue::error(format!("{corner:?}: нет подвески")));
            }

            if let (Some(rim), Some(tire)) = (&wheel.rim, &wheel.tire) {
                if rim.bolt_pattern != chassis.hub_bolt_pattern {
                    issues.push(AssemblyIssue::error(format!(
                        "{corner:?}: разболтовка диска ({}×{}) не совпадает со ступицей ({}×{})",
                        rim.bolt_pattern.bolts,
                        rim.bolt_pattern.pcd_mm,
                        chassis.hub_bolt_pattern.bolts,
                        chassis.hub_bolt_pattern.pcd_mm
                    )));
                }
                if tire.tire.radius > chassis.arch_radius {
                    issues.push(AssemblyIssue::error(format!(
                        "{corner:?}: радиус колеса {:.2} м больше арки {:.2} м",
                        tire.tire.radius, chassis.arch_radius
                    )));
                }
                if rim.radius_m() + 0.02 < tire.tire.radius - tire.tire.width * 0.0 {
                    // диск должен быть меньше шины; это мягкая проверка
                }
            }

            if let (Some(brake), Some(rim)) = (&wheel.brake, &wheel.rim) {
                if brake.disc_diameter > chassis.max_brake_disc {
                    issues.push(AssemblyIssue::error(format!(
                        "{corner:?}: тормозной диск {:.2} м больше допустимого {:.2} м",
                        brake.disc_diameter, chassis.max_brake_disc
                    )));
                }
                if brake.disc_diameter * 0.5 > rim.radius_m() {
                    issues.push(AssemblyIssue::error(format!(
                        "{corner:?}: тормозной диск не влезает в диск колеса"
                    )));
                }
            }
        }

        issues
    }

    /// Есть ли ошибки (а не только предупреждения).
    pub fn has_errors(issues: &[AssemblyIssue]) -> bool {
        issues.iter().any(|i| i.severity == Severity::Error)
    }

    /// Собирает конфигурацию симуляции. Возвращает ошибки, если сборка неполная.
    pub fn build(&self) -> Result<VehicleConfig, Vec<AssemblyIssue>> {
        let issues = self.validate();
        if Self::has_errors(&issues) {
            return Err(issues);
        }

        let chassis = self.chassis.as_ref().unwrap();
        let engine = self.engine.as_ref().unwrap();
        let gearbox = self.gearbox.as_ref().unwrap();

        // --- Массы и точки масс ---
        let mut points: Vec<(Vec3, f32)> = Vec::new(); // (локальная позиция, масса)
        points.push((
            Vec3::new(0.0, chassis.com_height, chassis.com_offset),
            chassis.meta.mass,
        ));
        points.push((engine.mount, engine.meta.mass));
        points.push((gearbox.mount, gearbox.meta.mass));
        if let Some(diff) = &self.differential {
            points.push((gearbox.mount, diff.meta.mass));
        }
        if let Some(body) = &self.body {
            points.push((
                Vec3::new(0.0, chassis.com_height, chassis.com_offset),
                body.meta.mass,
            ));
        }
        for aero in &self.aero {
            points.push((aero.mount, aero.meta.mass));
        }

        // --- Колёса ---
        let mut wheels = Vec::new();
        let front_bias = gearbox.layout.front_bias();
        for corner in Corner::all() {
            let Some(assembly) = self.wheels.iter().find(|w| w.corner == corner) else {
                continue;
            };
            let (Some(rim), Some(tire), Some(brake), Some(susp)) = (
                assembly.rim.as_ref(),
                assembly.tire.as_ref(),
                assembly.brake.as_ref(),
                assembly.suspension.as_ref(),
            ) else {
                continue;
            };

            let track = if corner.is_front() {
                chassis.track_front
            } else {
                chassis.track_rear
            };
            let z = if corner.is_front() {
                -chassis.wheelbase * 0.5
            } else {
                chassis.wheelbase * 0.5
            };
            let position = Vec3::new(
                corner.side() * track * 0.5,
                tire.tire.radius + susp.suspension.rest_length,
                z,
            );

            let driven = match gearbox.layout {
                DriveLayout::Rwd => !corner.is_front(),
                DriveLayout::Fwd => corner.is_front(),
                DriveLayout::Awd { .. } => true,
            };

            let wheel_mass = rim.meta.mass + tire.meta.mass + brake.meta.mass;
            points.push((position, wheel_mass));

            wheels.push(WheelConfig {
                corner,
                position,
                steering: corner.is_front(),
                driven,
                handbrake: !corner.is_front(),
                max_steer_deg: susp.max_steer_deg,
                static_camber_rad: susp.camber_deg.to_radians(),
                static_toe_rad: susp.toe_deg.to_radians(),
                suspension: susp.suspension,
                tire: tire.tire,
                brake_torque: brake.max_torque(),
                wheel_mass,
                inertia: wheel_inertia(tire.tire.radius, wheel_mass),
            });
        }

        // --- Масса, центр масс, инерция ---
        let mass: f32 = points.iter().map(|(_, m)| *m).sum::<f32>().max(1.0);
        let mut com = Vec3::ZERO;
        for (pos, m) in &points {
            com += *pos * *m;
        }
        com /= mass;

        let mut inertia = Vec3::ZERO;
        for (pos, m) in &points {
            let d = *pos - com;
            inertia.x += m * (d.y * d.y + d.z * d.z);
            inertia.y += m * (d.x * d.x + d.z * d.z);
            inertia.z += m * (d.x * d.x + d.y * d.y);
        }
        // Минимальная инерция, чтобы не было деления на ноль.
        inertia = inertia.max(Vec3::splat(50.0));

        // --- Трансмиссия ---
        let powertrain = PowertrainConfig {
            engine: engine.engine.clone(),
            clutch: engine.clutch,
            transmission: gearbox.transmission.clone(),
            differential: self
                .differential
                .as_ref()
                .map(|d| d.kind)
                .unwrap_or(DifferentialKind::Open),
            center: self
                .center_differential
                .as_ref()
                .map(|d| d.kind)
                .unwrap_or(DifferentialKind::Open),
            front_bias,
            ..PowertrainConfig::default()
        };

        // --- Аэродинамика ---
        let mut aero = AeroConfig::default();
        if let Some(body) = &self.body {
            aero.drag_area = body.drag_cd * body.frontal_area;
            aero.frontal_area = body.frontal_area;
            aero.downforce_area = body.downforce;
        }
        let front_z = -chassis.wheelbase * 0.5;
        let rear_z = chassis.wheelbase * 0.5;
        let mut downforce_total = aero.downforce_area;
        for part in &self.aero {
            aero.drag_area += part.drag_delta;
            downforce_total += part.downforce_delta;
        }
        // Распределяем прижимную силу по точкам установки аэро-деталей.
        if self.aero.is_empty() {
            aero.downforce_points = vec![
                (Vec3::new(0.0, 0.0, front_z), downforce_total * 0.45),
                (Vec3::new(0.0, 0.0, rear_z), downforce_total * 0.55),
            ];
        } else {
            let share = downforce_total / self.aero.len() as f32;
            aero.downforce_points = self.aero.iter().map(|a| (a.mount, share)).collect();
        }
        aero.downforce_area = downforce_total;

        Ok(VehicleConfig {
            mass,
            inertia,
            center_of_mass: com,
            powertrain,
            wheels,
            aero,
            grip_multiplier: 1.0,
            front_drive_bias: front_bias,
        })
    }
}
