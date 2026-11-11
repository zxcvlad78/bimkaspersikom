//! Двигатель: mean-value модель с физикой воздушного тракта, динамикой
//! ресивера, впрыском топлива, турбонаддувом, охлаждением, смазкой, износом.
//!
//! Модель оперирует реальными величинами (Па, К, кг/с, Н·м) и даёт:
//! момент, обороты, давление во впуске, наддув, AFR, температуры, расход
//! топлива, срабатывание отсечки, детонацию и повреждения.

use std::f32::consts::{PI, TAU};

/// Универсальная газовая постоянная воздуха, Дж/(кг·К).
const R_AIR: f32 = 287.0;
/// Показатель адиабаты воздуха.
const GAMMA: f32 = 1.4;
/// Теплоёмкость воздуха при постоянном давлении.
#[allow(dead_code)]
const CP_AIR: f32 = 1005.0;
/// Атмосферное давление, Па.
const P_AMBIENT: f32 = 101_325.0;
/// Температура воздуха, К.
const T_AMBIENT: f32 = 293.15;

/// Тип топлива.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FuelType {
    Gasoline95,
    Gasoline98,
    E85,
    Diesel,
    Lpg,
}

impl FuelType {
    /// Низшая теплота сгорания, Дж/кг.
    pub fn lhv(self) -> f32 {
        match self {
            FuelType::Gasoline95 | FuelType::Gasoline98 => 44.0e6,
            FuelType::E85 => 29.2e6,
            FuelType::Diesel => 42.7e6,
            FuelType::Lpg => 46.0e6,
        }
    }
    /// Стехиометрическое соотношение воздух/топливо.
    pub fn stoich_afr(self) -> f32 {
        match self {
            FuelType::Gasoline95 | FuelType::Gasoline98 => 14.7,
            FuelType::E85 => 9.8,
            FuelType::Diesel => 14.5,
            FuelType::Lpg => 15.7,
        }
    }
    /// Октановое число (стойкость к детонации).
    pub fn octane(self) -> f32 {
        match self {
            FuelType::Gasoline95 => 95.0,
            FuelType::Gasoline98 => 98.0,
            FuelType::E85 => 105.0,
            FuelType::Diesel => 0.0, // детонация не так актуальна
            FuelType::Lpg => 110.0,
        }
    }
    /// Плотность, кг/л.
    pub fn density(self) -> f32 {
        match self {
            FuelType::Gasoline95 | FuelType::Gasoline98 => 0.745,
            FuelType::E85 => 0.785,
            FuelType::Diesel => 0.84,
            FuelType::Lpg => 0.51,
        }
    }
}

/// Тип наддува.
#[derive(Clone, Copy, Debug)]
pub enum Aspiration {
    NaturallyAspirated,
    /// Турбонаддув.
    Turbo {
        /// Максимальное избыточное давление, Па.
        max_boost: f32,
        /// Давление срабатывания вестгейта, Па.
        wastegate: f32,
        /// Инерция ротора (условная единица задержки), с.
        lag: f32,
        /// КПД компрессора.
        efficiency: f32,
        /// КПД интеркулера 0..1.
        intercooler: f32,
    },
    /// Механический нагнетатель.
    Supercharged {
        max_boost: f32,
        intercooler: f32,
        /// Момент, отбираемый приводом, Н·м при полном наддуве.
        drive_torque: f32,
    },
}

impl Default for Aspiration {
    fn default() -> Self {
        Aspiration::NaturallyAspirated
    }
}

/// Модель механических потерь: T = a + b·ω + c·ω².
#[derive(Clone, Copy, Debug)]
pub struct FrictionModel {
    pub constant: f32,
    pub linear: f32,
    pub quadratic: f32,
}

impl Default for FrictionModel {
    fn default() -> Self {
        Self {
            constant: 12.0,
            linear: 0.06,
            quadratic: 0.00022,
        }
    }
}

/// Система охлаждения и смазки.
#[derive(Clone, Copy, Debug)]
pub struct CoolingSpec {
    /// Масса охлаждающей жидкости, кг.
    pub coolant_mass: f32,
    /// Эффективность радиатора, Вт/К.
    pub radiator_rate: f32,
    /// Температура открытия термостата, °C.
    pub thermostat_open_c: f32,
    /// Доп. поток от вентилятора, Вт/К.
    pub fan_rate: f32,
    /// Критическая температура, °C.
    pub max_temp_c: f32,
    /// Масса масла, кг.
    pub oil_mass: f32,
    /// Эффективность маслорадиатора, Вт/К.
    pub oil_cooler_rate: f32,
}

impl Default for CoolingSpec {
    fn default() -> Self {
        Self {
            coolant_mass: 6.0,
            radiator_rate: 900.0,
            thermostat_open_c: 88.0,
            fan_rate: 500.0,
            max_temp_c: 125.0,
            oil_mass: 4.5,
            oil_cooler_rate: 250.0,
        }
    }
}

/// Полная конфигурация двигателя.
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Рабочий объём, л.
    pub displacement_l: f32,
    /// Число цилиндров.
    pub cylinders: u8,
    /// Степень сжатия.
    pub compression_ratio: f32,
    /// Объём ресивера, л.
    pub plenum_volume_l: f32,
    /// Кривая объёмного КПД: (обороты, КПД).
    pub volumetric_efficiency: Vec<(f32, f32)>,
    pub aspiration: Aspiration,
    pub fuel: FuelType,
    /// Целевое AFR (лямбда-эквивалент).
    pub afr_target: f32,
    /// Обороты холостого хода.
    pub idle_rpm: f32,
    /// Обороты максимальной мощности.
    pub redline_rpm: f32,
    /// Отсечка.
    pub limiter_rpm: f32,
    /// Момент инерции коленвала с маховиком, кг·м².
    pub inertia: f32,
    pub friction: FrictionModel,
    pub cooling: CoolingSpec,
    /// Момент навесного оборудования, Н·м.
    pub accessory_torque: f32,
    /// Производительность форсунок, кг/с суммарно.
    pub injector_max_flow: f32,
    /// Объём бензобака, л.
    pub fuel_tank_l: f32,
    /// Масса, кг.
    pub mass: f32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            displacement_l: 2.0,
            cylinders: 4,
            compression_ratio: 10.5,
            plenum_volume_l: 2.4,
            volumetric_efficiency: vec![
                (800.0, 0.72),
                (1500.0, 0.86),
                (2500.0, 0.93),
                (3500.0, 0.95),
                (4500.0, 0.94),
                (5500.0, 0.90),
                (6500.0, 0.83),
                (7500.0, 0.72),
            ],
            aspiration: Aspiration::NaturallyAspirated,
            fuel: FuelType::Gasoline95,
            afr_target: 13.2,
            idle_rpm: 850.0,
            redline_rpm: 6800.0,
            limiter_rpm: 7200.0,
            inertia: 0.22,
            friction: FrictionModel::default(),
            cooling: CoolingSpec::default(),
            accessory_torque: 6.0,
            injector_max_flow: 0.09,
            fuel_tank_l: 55.0,
            mass: 160.0,
        }
    }
}

impl EngineConfig {
    /// Оценочный максимальный момент на маховике, Н·м.
    ///
    /// Считается из среднего эффективного давления (BMEP) и рабочего объёма:
    /// `T = BMEP · Vd / (4π)` для четырёхтактного двигателя.
    pub fn max_torque(&self) -> f32 {
        let bmep = match self.aspiration {
            Aspiration::NaturallyAspirated => 10.5e5,
            Aspiration::Turbo { .. } => 19.0e5,
            Aspiration::Supercharged { .. } => 16.0e5,
        };
        let vd = self.displacement_l * 1e-3;
        bmep * vd / (4.0 * PI)
    }

    /// Оценочная максимальная мощность, кВт.
    pub fn max_power_kw(&self) -> f32 {
        let omega = self.redline_rpm * TAU / 60.0;
        self.max_torque() * 0.85 * omega / 1000.0
    }

    /// Кривая объёмного КПД (линейная интерполяция).
    pub fn volumetric_efficiency_at(&self, rpm: f32) -> f32 {
        let c = &self.volumetric_efficiency;
        if c.is_empty() {
            return 0.9;
        }
        if rpm <= c[0].0 {
            return c[0].1;
        }
        if rpm >= c[c.len() - 1].0 {
            return c[c.len() - 1].1;
        }
        for w in c.windows(2) {
            if rpm <= w[1].0 {
                let f = (rpm - w[0].0) / (w[1].0 - w[0].0).max(1e-3);
                return w[0].1 + (w[1].1 - w[0].1) * f;
            }
        }
        c[c.len() - 1].1
    }
}

/// Наблюдаемое состояние двигателя.
#[derive(Clone, Copy, Debug)]
pub struct EngineOutput {
    pub torque: f32,
    pub rpm: f32,
    /// Абсолютное давление во впуске, Па.
    pub manifold_pressure: f32,
    /// Избыточный наддув, Па.
    pub boost: f32,
    pub coolant_temp: f32,
    pub oil_temp: f32,
    pub afr: f32,
    pub air_flow: f32,
    pub fuel_flow: f32,
    /// Мощность, кВт.
    pub power_kw: f32,
    pub running: bool,
    pub fuel_cut: bool,
    pub knock: bool,
    /// Износ 0..1.
    pub damage: f32,
}

/// Состояние двигателя (все интегрируемые величины).
#[derive(Clone, Debug)]
pub struct Engine {
    /// Угловая скорость коленвала, рад/с.
    pub omega: f32,
    /// Давление во впуске, Па.
    pub manifold_pressure: f32,
    /// Нормализованная скорость турбины 0..1.
    pub turbo_speed: f32,
    /// Температура впуска после интеркулера, К.
    pub intake_temp: f32,
    pub coolant_temp: f32,
    pub oil_temp: f32,
    /// Остаток топлива, л.
    pub fuel_l: f32,
    pub running: bool,
    pub starter: bool,
    pub fuel_cut: bool,
    pub knock: bool,
    /// Накопленный износ 0..1.
    pub damage: f32,
    pub output: EngineOutput,
}

impl Engine {
    pub fn new(cfg: &EngineConfig) -> Self {
        let idle_omega = cfg.idle_rpm * TAU / 60.0;
        let mut engine = Self {
            omega: 0.0,
            manifold_pressure: P_AMBIENT,
            turbo_speed: 0.0,
            intake_temp: T_AMBIENT,
            coolant_temp: 20.0,
            oil_temp: 20.0,
            fuel_l: cfg.fuel_tank_l,
            running: false,
            starter: false,
            fuel_cut: false,
            knock: false,
            damage: 0.0,
            output: EngineOutput {
                torque: 0.0,
                rpm: 0.0,
                manifold_pressure: P_AMBIENT,
                boost: 0.0,
                coolant_temp: 20.0,
                oil_temp: 20.0,
                afr: cfg.afr_target,
                air_flow: 0.0,
                fuel_flow: 0.0,
                power_kw: 0.0,
                running: false,
                fuel_cut: false,
                knock: false,
                damage: 0.0,
            },
        };
        engine.omega = 0.0;
        let _ = idle_omega;
        engine
    }

    /// Полный шаг двигателя. `load_torque` — момент, который снимает сцепление
    /// (Н·м, положительный — двигатель отдаёт момент). Возвращает момент на
    /// маховике (Н·м), который нужно подать на сцепление.
    pub fn step(&mut self, cfg: &EngineConfig, throttle: f32, load_torque: f32, dt: f32) -> f32 {
        let rpm = self.omega * 60.0 / TAU;
        let idle_omega = cfg.idle_rpm * TAU / 60.0;

        // --- Управление холостым ходом и стартером ---
        let mut throttle = throttle.clamp(0.0, 1.0);
        if self.running && rpm < cfg.idle_rpm * 1.1 {
            let idle_demand = ((cfg.idle_rpm - rpm) / cfg.idle_rpm).clamp(0.0, 1.0) * 0.22;
            throttle = throttle.max(idle_demand);
        }

        // --- Воздух: расход через дроссель и через двигатель ---
        let plenum_v = (cfg.plenum_volume_l * 1e-3).max(1e-4);
        let t_manifold = self.intake_temp.max(200.0);
        let boost = (self.manifold_pressure - P_AMBIENT).max(0.0);

        // Дроссель: эффективная площадь растёт нелинейно.
        let throttle_area = 3.0e-3 * (1.0 - (1.0 - throttle).powi(2));
        let p_upstream = (P_AMBIENT + boost).max(P_AMBIENT);
        let m_dot_throttle = orifice_flow(throttle_area, p_upstream, T_AMBIENT, self.manifold_pressure);

        // Наддув.
        let (m_dot_compressor, boost_target) = self.turbo_step(cfg, throttle, dt);
        let m_dot_in = m_dot_throttle + m_dot_compressor;

        // Расход двигателем.
        let ve = cfg.volumetric_efficiency_at(rpm);
        let rho_man = self.manifold_pressure / (R_AIR * t_manifold);
        let v_dot = cfg.displacement_l * 1e-3 * self.omega.abs() / (4.0 * PI);
        let m_dot_engine = (ve * rho_man * v_dot).max(0.0);

        // Динамика ресивера: dP/dt = (R·T/V)·(m_in − m_out).
        let dp = (R_AIR * t_manifold / plenum_v) * (m_dot_in - m_dot_engine) * dt;
        self.manifold_pressure = (self.manifold_pressure + dp).clamp(2_000.0, 400_000.0);

        // --- Топливо и горение ---
        let limiter = self.omega > cfg.limiter_rpm * TAU / 60.0;
        self.fuel_cut = limiter || !self.running;
        let afr = cfg.afr_target.max(8.0);
        let mut m_dot_fuel = if self.fuel_cut { 0.0 } else { m_dot_engine / afr };
        m_dot_fuel = m_dot_fuel.min(cfg.injector_max_flow);

        // Ограничение по остатку топлива.
        let fuel_mass_available = self.fuel_l * cfg.fuel.density();
        if m_dot_fuel * dt > fuel_mass_available {
            m_dot_fuel = (fuel_mass_available / dt).max(0.0);
        }
        self.fuel_l = (self.fuel_l - m_dot_fuel * dt / cfg.fuel.density()).max(0.0);
        if self.fuel_l <= 0.0 && self.running {
            self.running = false;
        }

        // --- Индикаторный момент ---
        let eta = combustion_efficiency(cfg, afr, rpm, self.coolant_temp);
        let p_indicated = m_dot_fuel * cfg.fuel.lhv() * eta;
        let t_indicated = if self.omega.abs() > 1.0 {
            p_indicated / self.omega.abs()
        } else {
            0.0
        };

        // --- Потери ---
        let oil_factor = 1.0 + ((90.0 - self.oil_temp) / 90.0).clamp(0.0, 1.0) * 0.6;
        let t_friction = (cfg.friction.constant
            + cfg.friction.linear * self.omega.abs()
            + cfg.friction.quadratic * self.omega * self.omega)
            * oil_factor
            * if self.running { 1.0 } else { 0.7 };

        // Насосные потери: разница давлений выпуск/впуск.
        let p_exhaust = P_AMBIENT + 0.15 * boost + 1_000.0;
        let t_pumping = (p_exhaust - self.manifold_pressure) * cfg.displacement_l * 1e-3 / (4.0 * PI);

        // Навесное оборудование и нагнетатель.
        let mut accessory = cfg.accessory_torque;
        if let Aspiration::Supercharged { drive_torque, .. } = cfg.aspiration {
            accessory += drive_torque * throttle;
        }

        // --- Стартер ---
        let t_starter = if self.starter && !self.running {
            120.0
        } else {
            0.0
        };

        let t_engine = t_indicated - t_friction - t_pumping - accessory + t_starter;

        // --- Динамика коленвала ---
        let j = cfg.inertia.max(0.03);
        self.omega += (t_engine - load_torque) / j * dt;

        // Запуск/остановка.
        if !self.running {
            if self.starter && self.omega * 60.0 / TAU > cfg.idle_rpm * 0.6 {
                self.running = true;
            }
            if !self.starter {
                self.omega = self.omega.max(0.0) * (1.0 - 2.0 * dt).max(0.0);
            }
        }
        if self.running && self.omega * 60.0 / TAU < cfg.idle_rpm * 0.4 {
            self.running = false;
        }
        // Холостой ход не даёт заглохнуть на передаче.
        if self.running {
            self.omega = self.omega.max(idle_omega * 0.85);
        }
        self.omega = self.omega.clamp(0.0, cfg.limiter_rpm * TAU / 60.0 * 1.05);

        // --- Детонация ---
        self.knock = false;
        if self.running && !self.fuel_cut {
            let octane = cfg.fuel.octane();
            let pressure_ratio = self.manifold_pressure / P_AMBIENT;
            let knock_index = pressure_ratio * (cfg.compression_ratio / 10.0)
                * (self.coolant_temp / 100.0).max(0.8)
                * (1.0 - (octane - 95.0) / 40.0).max(0.3);
            if knock_index > 1.15 && throttle > 0.6 {
                self.knock = true;
                self.damage = (self.damage + dt * 0.15).min(1.0);
            }
        }

        // --- Перегрев и износ ---
        self.thermal_step(cfg, m_dot_fuel, t_indicated, dt);
        let overrev = (rpm / cfg.redline_rpm - 1.0).max(0.0);
        if overrev > 0.0 {
            self.damage = (self.damage + overrev * dt * 0.4).min(1.0);
        }

        // --- Публикация ---
        let new_rpm = self.omega * 60.0 / TAU;
        self.output = EngineOutput {
            torque: t_engine,
            rpm: new_rpm,
            manifold_pressure: self.manifold_pressure,
            boost: (self.manifold_pressure - P_AMBIENT).max(0.0),
            coolant_temp: self.coolant_temp,
            oil_temp: self.oil_temp,
            afr: if m_dot_fuel > 1e-6 { m_dot_engine / m_dot_fuel } else { 0.0 },
            air_flow: m_dot_engine,
            fuel_flow: m_dot_fuel,
            power_kw: t_engine * self.omega / 1000.0,
            running: self.running,
            fuel_cut: self.fuel_cut,
            knock: self.knock,
            damage: self.damage,
        };
        let _ = boost_target;
        t_engine
    }

    /// Динамика наддува (mean-value турбина с задержкой раскрутки).
    fn turbo_step(&mut self, cfg: &EngineConfig, throttle: f32, dt: f32) -> (f32, f32) {
        match cfg.aspiration {
            Aspiration::NaturallyAspirated => {
                self.turbo_speed = 0.0;
                (0.0, 0.0)
            }
            Aspiration::Supercharged { max_boost, .. } => {
                // Нагнетатель даёт наддув практически мгновенно, пропорционально оборотам.
                let rpm = self.omega * 60.0 / TAU;
                let speed_factor = (rpm / cfg.redline_rpm).clamp(0.0, 1.0);
                let target = max_boost * speed_factor * throttle;
                let boost_now = (self.manifold_pressure - P_AMBIENT).max(0.0);
                let make_up = (target - boost_now) * 2.0;
                (make_up.max(0.0) * 0.05, target)
            }
            Aspiration::Turbo {
                max_boost,
                wastegate,
                lag,
                efficiency,
                intercooler,
            } => {
                let rpm = self.omega * 60.0 / TAU;
                let load = (throttle * 0.7 + 0.3).clamp(0.0, 1.0);
                let rpm_factor = (rpm / (cfg.redline_rpm * 0.6)).clamp(0.0, 1.0);
                let target_speed = (load * rpm_factor).clamp(0.0, 1.0);
                let tau = lag.max(0.05);
                self.turbo_speed += (target_speed - self.turbo_speed) * (dt / tau).min(1.0);

                let mut boost = max_boost * self.turbo_speed.powf(1.5) * load;
                boost = boost.min(wastegate);
                // Интеркулер охлаждает сжатый воздух.
                let t_compressed = T_AMBIENT * (1.0 + boost / P_AMBIENT * 0.6);
                self.intake_temp =
                    T_AMBIENT + (t_compressed - T_AMBIENT) * (1.0 - intercooler.clamp(0.0, 1.0));

                // Прирост массы воздуха за счёт компрессора.
                let extra_flow = boost / (R_AIR * self.intake_temp) * 0.0015 * efficiency;
                (extra_flow.max(0.0), boost)
            }
        }
    }

    /// Тепловой баланс: охлаждение, масло, перегрев.
    fn thermal_step(&mut self, cfg: &EngineConfig, m_dot_fuel: f32, t_indicated: f32, dt: f32) {
        // Тепло, уходящее в стенки, ~ доля от подведённой энергии.
        let q_fuel = m_dot_fuel * cfg.fuel.lhv();
        let q_wall = q_fuel * 0.28 + t_indicated.abs() * self.omega.abs() * 0.05;

        let c = &cfg.cooling;
        let thermostat = if self.coolant_temp > c.thermostat_open_c {
            ((self.coolant_temp - c.thermostat_open_c) / 12.0).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q_radiator = (c.radiator_rate + c.fan_rate * thermostat) * (self.coolant_temp - 20.0) * thermostat;
        // c_water ≈ 4186 Дж/(кг·К)
        let coolant_dt = (q_wall - q_radiator) / (c.coolant_mass * 4186.0) * dt;
        self.coolant_temp = (self.coolant_temp + coolant_dt).clamp(-40.0, 200.0);

        // Масло прогревается от двигателя и охлаждается маслорадиатором.
        let q_oil_in = q_wall * 0.25;
        let q_oil_out = c.oil_cooler_rate * (self.oil_temp - 20.0);
        let oil_dt = (q_oil_in - q_oil_out) / (c.oil_mass * 1900.0) * dt;
        self.oil_temp = (self.oil_temp + oil_dt).clamp(-40.0, 220.0);

        if self.coolant_temp > c.max_temp_c {
            self.damage = (self.damage + dt * 0.2).min(1.0);
        }
        if self.oil_temp > 160.0 {
            self.damage = (self.damage + dt * 0.1).min(1.0);
        }
    }
}

/// Расход через дроссель (сжимаемое течение, с учётом запирания).
fn orifice_flow(cd_a: f32, p_up: f32, t_up: f32, p_down: f32) -> f32 {
    let pr = (p_down / p_up.max(1.0)).clamp(0.0, 1.0);
    let critical = (2.0 / (GAMMA + 1.0)).powf(GAMMA / (GAMMA - 1.0));
    let factor = if pr > critical {
        let term = (2.0 * GAMMA / (GAMMA - 1.0)) * (pr.powf(2.0 / GAMMA) - pr.powf((GAMMA + 1.0) / GAMMA));
        term.max(0.0).sqrt()
    } else {
        (GAMMA * (2.0 / (GAMMA + 1.0)).powf((GAMMA + 1.0) / (GAMMA - 1.0))).sqrt()
    };
    cd_a * p_up / (R_AIR * t_up).sqrt() * factor
}

/// Термический КПД как функция AFR, оборотов и температуры.
fn combustion_efficiency(cfg: &EngineConfig, afr: f32, rpm: f32, coolant: f32) -> f32 {
    // Максимум около стехиометрии, спад на богатых и бедных смесях.
    let rel = afr / cfg.fuel.stoich_afr();
    let afr_factor = (1.0 - 1.4 * (rel - 0.95).powi(2)).clamp(0.45, 1.0);
    // Спад КПД на высоких оборотах (потери на трение газа, время цикла).
    let rpm_factor = (1.0 - (rpm / (cfg.redline_rpm * 1.6)).powi(2)).clamp(0.6, 1.0);
    // Перегрев снижает КПД (детонация, позднее зажигание).
    let temp_factor = (1.0 - ((coolant - 95.0) / 120.0).max(0.0)).clamp(0.7, 1.0);
    0.38 * afr_factor * rpm_factor * temp_factor
}
