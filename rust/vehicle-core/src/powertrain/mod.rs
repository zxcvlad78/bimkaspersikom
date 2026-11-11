//! Трансмиссия целиком: двигатель, сцепление или гидротрансформатор, КПП,
//! дифференциалы и тормоза. На выходе — моменты по каждому колесу.
//!
//! ## Порядок колёс
//! Все массивы `[f32; 4]` индексируются как `[FL, FR, RL, RR]`
//! (переднее левое, переднее правое, заднее левое, заднее правое).
//!
//! ## Поток момента
//! `двигатель → сцепление/ГДТ → КПП → центральный диф. → межколёсные диф. →
//! колёса`, а крутящий момент двигателя определяется нагрузкой со стороны
//! сцепления, которая зависит от скорости ведущих колёс.

pub mod brakes;
pub mod clutch;
pub mod differential;
pub mod engine;
pub mod transmission;

pub use brakes::{BrakeConfig, BrakeWheel, Brakes};
pub use clutch::{Clutch, ClutchConfig};
pub use differential::{distribute, distribute_center, DifferentialKind};
pub use engine::{
    Aspiration, CoolingSpec, Engine, EngineConfig, EngineOutput, FrictionModel, FuelType,
};
pub use transmission::{
    omega_from_rpm, rpm_from_omega, TorqueConverter, TorqueConverterConfig, Transmission,
    TransmissionConfig,
};

use std::f32::consts::TAU;

/// Индекс переднего левого колеса.
pub const FL: usize = 0;
/// Индекс переднего правого колеса.
pub const FR: usize = 1;
/// Индекс заднего левого колеса.
pub const RL: usize = 2;
/// Индекс заднего правого колеса.
pub const RR: usize = 3;

/// Входные воздействия водителя/ИИ.
#[derive(Clone, Copy, Debug)]
pub struct PowertrainInputs {
    /// Газ: 0..1.
    pub throttle: f32,
    /// Тормоз: 0..1.
    pub brake: f32,
    /// Ручной тормоз: 0..1.
    pub handbrake: f32,
    /// Сцепление: 0 — выжато, 1 — отпущено.
    pub clutch: f32,
    /// Руль: -1 (влево) .. +1 (вправо).
    pub steer: f32,
    /// Переключение вверх (нажатие в этом кадре).
    pub shift_up: bool,
    /// Переключение вниз (нажатие в этом кадре).
    pub shift_down: bool,
    /// Стартер.
    pub starter: bool,
    /// Зажигание включено.
    pub ignition: bool,
}

impl Default for PowertrainInputs {
    fn default() -> Self {
        Self {
            throttle: 0.0,
            brake: 0.0,
            handbrake: 0.0,
            clutch: 1.0,
            steer: 0.0,
            shift_up: false,
            shift_down: false,
            starter: false,
            ignition: true,
        }
    }
}

/// Состояние колеса, необходимое трансмиссии.
#[derive(Clone, Copy, Debug, Default)]
pub struct WheelState {
    /// Угловая скорость, рад/с.
    pub omega: f32,
    /// Продольное скольжение (для TCS/ABS).
    pub slip: f32,
    /// Есть контакт с поверхностью.
    pub contact: bool,
}

/// Роль колеса в трансмиссии.
#[derive(Clone, Copy, Debug, Default)]
pub struct WheelRole {
    /// Колесо ведущее.
    pub driven: bool,
    /// Есть ручной тормоз.
    pub handbrake: bool,
    /// Передняя ось.
    pub front: bool,
    /// Параметры тормоза колеса.
    pub brake: BrakeWheel,
}

/// Полная конфигурация трансмиссии.
#[derive(Clone, Debug)]
pub struct PowertrainConfig {
    pub engine: EngineConfig,
    pub clutch: ClutchConfig,
    pub transmission: TransmissionConfig,
    /// Межколёсный дифференциал ведущей оси.
    pub differential: DifferentialKind,
    /// Центральный дифференциал (для полного привода).
    pub center: DifferentialKind,
    pub brakes: BrakeConfig,
    /// Доля момента на переднюю ось (0 — задний привод, 1 — передний).
    pub front_bias: f32,
    /// Противобуксовочная система.
    pub tcs: bool,
    /// Порог скольжения TCS.
    pub tcs_slip: f32,
    /// Курсовая устойчивость.
    pub esc: bool,
}

impl Default for PowertrainConfig {
    fn default() -> Self {
        Self {
            engine: EngineConfig::default(),
            clutch: ClutchConfig::default(),
            transmission: TransmissionConfig::default(),
            differential: DifferentialKind::Lsd {
                preload: 40.0,
                coupling: 260.0,
                bias_ratio: 0.45,
            },
            center: DifferentialKind::Open,
            brakes: BrakeConfig::default(),
            front_bias: 0.0,
            tcs: true,
            tcs_slip: 0.22,
            esc: true,
        }
    }
}

/// Наблюдаемое состояние трансмиссии (для интерфейса и звука).
#[derive(Clone, Copy, Debug)]
pub struct PowertrainOutput {
    pub engine_torque: f32,
    pub rpm: f32,
    pub gear: i32,
    pub shifting: bool,
    pub clutch_engagement: f32,
    pub clutch_slip: f32,
    pub clutch_temperature: f32,
    pub converter_locked: bool,
    pub torque_ratio: f32,
    /// Момент, подаваемый на каждое колесо, Н·м.
    pub drive_torque: [f32; 4],
    /// Тормозной момент по колёсам, Н·м.
    pub brake_torque: [f32; 4],
    /// Сработал ли ABS на колесе.
    pub abs_active: [bool; 4],
    pub tcs_active: bool,
    pub boost: f32,
    pub manifold_pressure: f32,
    pub coolant_temp: f32,
    pub oil_temp: f32,
    pub afr: f32,
    /// Остаток топлива, л.
    pub fuel_l: f32,
    pub power_kw: f32,
    pub running: bool,
    pub fuel_cut: bool,
    pub knock: bool,
    /// Износ двигателя 0..1.
    pub damage: f32,
    /// Износ коробки 0..1.
    pub gearbox_damage: f32,
    /// Суммарный момент на колёсах, Н·м.
    pub drive_torque_total: f32,
}

impl Default for PowertrainOutput {
    fn default() -> Self {
        Self {
            engine_torque: 0.0,
            rpm: 0.0,
            gear: 1,
            shifting: false,
            clutch_engagement: 0.0,
            clutch_slip: 0.0,
            clutch_temperature: 20.0,
            converter_locked: false,
            torque_ratio: 1.0,
            drive_torque: [0.0; 4],
            brake_torque: [0.0; 4],
            abs_active: [false; 4],
            tcs_active: false,
            boost: 0.0,
            manifold_pressure: 101_325.0,
            coolant_temp: 20.0,
            oil_temp: 20.0,
            afr: 0.0,
            fuel_l: 0.0,
            power_kw: 0.0,
            running: false,
            fuel_cut: false,
            knock: false,
            damage: 0.0,
            gearbox_damage: 0.0,
            drive_torque_total: 0.0,
        }
    }
}

/// Трансмиссия целиком.
#[derive(Clone, Debug)]
pub struct Powertrain {
    pub engine: Engine,
    pub clutch: Clutch,
    pub transmission: Transmission,
    pub brakes: Brakes,
    pub output: PowertrainOutput,
    /// Сработала ли TCS в этом кадре.
    pub tcs_active: bool,
}

impl Powertrain {
    pub fn new(cfg: &PowertrainConfig) -> Self {
        let mut out = PowertrainOutput::default();
        out.fuel_l = cfg.engine.fuel_tank_l;
        Self {
            engine: Engine::new(&cfg.engine),
            clutch: Clutch::default(),
            transmission: Transmission::new(),
            brakes: Brakes::new(),
            output: out,
            tcs_active: false,
        }
    }

    /// Полный шаг трансмиссии. Возвращает наблюдаемое состояние.
    pub fn step(
        &mut self,
        cfg: &PowertrainConfig,
        inputs: &PowertrainInputs,
        wheels: &[WheelState; 4],
        roles: &[WheelRole; 4],
        dt: f32,
    ) -> PowertrainOutput {
        // --- 1. Скорости ведущих колёс по осям ---
        let mut front_sum = 0.0;
        let mut front_n = 0u32;
        let mut rear_sum = 0.0;
        let mut rear_n = 0u32;
        for i in 0..4 {
            if roles[i].driven {
                if roles[i].front {
                    front_sum += wheels[i].omega;
                    front_n += 1;
                } else {
                    rear_sum += wheels[i].omega;
                    rear_n += 1;
                }
            }
        }
        let front_omega = if front_n > 0 {
            front_sum / front_n as f32
        } else {
            0.0
        };
        let rear_omega = if rear_n > 0 {
            rear_sum / rear_n as f32
        } else {
            0.0
        };
        let front_driven = front_n > 0;
        let rear_driven = rear_n > 0;
        let driven_omega = match (front_driven, rear_driven) {
            (true, true) => {
                front_omega * cfg.front_bias.clamp(0.0, 1.0)
                    + rear_omega * (1.0 - cfg.front_bias.clamp(0.0, 1.0))
            }
            (true, false) => front_omega,
            (false, true) => rear_omega,
            _ => 0.0,
        };
        // Грубая оценка скорости для логики КПП (радиус ~0.33 м).
        let speed = driven_omega * 0.33;

        // --- 2. Переключение передач ---
        let engine_rpm = self.engine.omega * 60.0 / TAU;
        self.transmission.update(
            &cfg.transmission,
            inputs.shift_up,
            inputs.shift_down,
            inputs.throttle,
            engine_rpm,
            speed,
            dt,
        );
        let ratio = self.transmission.ratio(&cfg.transmission);

        // --- 3. Сцепление / гидротрансформатор ---
        self.engine.starter = inputs.starter && inputs.ignition;
        let (shaft_torque, load_torque) = if self.transmission.gear == 0 {
            // Нейтраль: двигатель свободен.
            (0.0, 0.0)
        } else if let Some(tc_cfg) = &cfg.transmission.converter {
            let turbine_omega = driven_omega * ratio;
            let turbine_torque =
                self.transmission
                    .converter
                    .step(tc_cfg, self.engine.omega, turbine_omega);
            let load = self.transmission.converter.pump_torque;
            (turbine_torque, load)
        } else {
            let input_omega = driven_omega * ratio;
            let pedal = if self.transmission.shift_timer > 0.0 {
                0.0
            } else if cfg.transmission.automatic {
                1.0
            } else {
                inputs.clutch
            };
            let ct = self
                .clutch
                .step(&cfg.clutch, self.engine.omega, input_omega, pedal, dt);
            (ct, ct)
        };

        // --- 4. Двигатель (с учётом TCS) ---
        let mut throttle = inputs.throttle.clamp(0.0, 1.0);
        self.tcs_active = false;
        if cfg.tcs && throttle > 0.0 {
            let max_slip = (0..4)
                .filter(|&i| roles[i].driven)
                .map(|i| wheels[i].slip.abs())
                .fold(0.0f32, f32::max);
            if max_slip > cfg.tcs_slip {
                let scale = (cfg.tcs_slip / max_slip.max(1e-3)).clamp(0.05, 1.0);
                throttle *= scale;
                self.tcs_active = true;
            }
        }
        if !inputs.ignition {
            self.engine.running = false;
        }
        self.engine.step(&cfg.engine, throttle, load_torque, dt);
        let engine_out = self.engine.output;

        // --- 5. Момент на выходе КПП ---
        let output_torque = if self.transmission.gear == 0 {
            0.0
        } else {
            shaft_torque * ratio * cfg.transmission.efficiency
        };

        // Износ коробки при перегрузке. Сравниваем входной момент (после
        // сцепления/гидротрансформатора) с паспортным пределом КПП, а не момент
        // на колёсах — он умножен на передаточное число.
        let overload = (shaft_torque.abs() / cfg.transmission.max_torque.max(1.0) - 1.0).max(0.0);
        if overload > 0.0 {
            self.transmission.damage =
                (self.transmission.damage + overload * dt * 0.6).min(1.0);
            if self.transmission.damage >= 1.0 {
                self.transmission.broken = true;
            }
        }

        // --- 6. Распределение момента по колёсам ---
        let mut drive = [0.0f32; 4];
        if front_driven && rear_driven {
            let (ft, rt) = distribute_center(
                &cfg.center,
                output_torque,
                cfg.front_bias,
                front_omega,
                rear_omega,
            );
            let (fl, fr) = distribute(
                &cfg.differential,
                ft,
                wheels[FL].omega,
                wheels[FR].omega,
            );
            let (rl, rr) = distribute(
                &cfg.differential,
                rt,
                wheels[RL].omega,
                wheels[RR].omega,
            );
            drive[FL] = fl;
            drive[FR] = fr;
            drive[RL] = rl;
            drive[RR] = rr;
        } else if front_driven {
            let (fl, fr) = distribute(
                &cfg.differential,
                output_torque,
                wheels[FL].omega,
                wheels[FR].omega,
            );
            drive[FL] = fl;
            drive[FR] = fr;
        } else if rear_driven {
            let (rl, rr) = distribute(
                &cfg.differential,
                output_torque,
                wheels[RL].omega,
                wheels[RR].omega,
            );
            drive[RL] = rl;
            drive[RR] = rr;
        }

        // --- 7. Тормоза ---
        let brake_wheels = [
            roles[FL].brake,
            roles[FR].brake,
            roles[RL].brake,
            roles[RR].brake,
        ];
        let omega_arr = [
            wheels[FL].omega,
            wheels[FR].omega,
            wheels[RL].omega,
            wheels[RR].omega,
        ];
        let slip_arr = [
            wheels[FL].slip,
            wheels[FR].slip,
            wheels[RL].slip,
            wheels[RR].slip,
        ];
        let brake_torque = self.brakes.step(
            &cfg.brakes,
            &brake_wheels,
            inputs.brake,
            inputs.handbrake,
            &omega_arr,
            &slip_arr,
            dt,
        );

        // --- 8. Публикация ---
        self.output = PowertrainOutput {
            engine_torque: engine_out.torque,
            rpm: engine_out.rpm,
            gear: self.transmission.gear,
            shifting: self.transmission.shifting(),
            clutch_engagement: self.clutch.engagement,
            clutch_slip: self.clutch.slip,
            clutch_temperature: self.clutch.temperature,
            converter_locked: self.transmission.converter.locked,
            torque_ratio: self.transmission.converter.torque_ratio,
            drive_torque: drive,
            brake_torque,
            abs_active: self.brakes.abs_active,
            tcs_active: self.tcs_active,
            boost: engine_out.boost,
            manifold_pressure: engine_out.manifold_pressure,
            coolant_temp: engine_out.coolant_temp,
            oil_temp: engine_out.oil_temp,
            afr: engine_out.afr,
            fuel_l: self.engine.fuel_l,
            power_kw: engine_out.power_kw,
            running: engine_out.running,
            fuel_cut: engine_out.fuel_cut,
            knock: engine_out.knock,
            damage: engine_out.damage,
            gearbox_damage: self.transmission.damage,
            drive_torque_total: drive.iter().sum(),
        };
        self.output
    }

    /// Запускает двигатель (мгновенно, для тестов и «телепорта» в машину).
    pub fn force_start(&mut self, cfg: &PowertrainConfig) {
        self.engine.running = true;
        self.engine.omega = omega_from_rpm(cfg.engine.idle_rpm * 1.2);
        self.engine.coolant_temp = 90.0;
        self.engine.oil_temp = 90.0;
    }

    /// Глушит двигатель.
    pub fn stop_engine(&mut self) {
        self.engine.running = false;
    }

    /// Обороты двигателя, рад/с.
    #[inline]
    pub fn engine_omega(&self) -> f32 {
        self.engine.omega
    }
}

/// Вспомогательный конструктор входов.
impl PowertrainInputs {
    pub fn driving(throttle: f32, brake: f32, steer: f32) -> Self {
        Self {
            throttle: throttle.clamp(0.0, 1.0),
            brake: brake.clamp(0.0, 1.0),
            steer: steer.clamp(-1.0, 1.0),
            ..Default::default()
        }
    }
}
