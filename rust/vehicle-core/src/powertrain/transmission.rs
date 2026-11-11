//! Коробка передач: механическая и автоматическая, с гидротрансформатором.
//!
//! КПП хранит состояние переключения и передаточные числа. Динамика входного
//! вала определяется кинематически от скорости ведущих колёс (жёсткий вал);
//! вместе с моделью сцепления/гидротрансформатора это даёт корректные
//! переходные процессы без жёсткой системы уравнений.

use std::f32::consts::TAU;

/// Гидротрансформатор (для автоматических КПП).
#[derive(Clone, Copy, Debug)]
pub struct TorqueConverterConfig {
    /// Коэффициент трансформации момента при неподвижной турбине.
    pub stall_ratio: f32,
    /// Отношение скоростей турбина/насос, при котором трансформация исчезает.
    pub coupling_ratio: f32,
    /// Ёмкость насоса, Н·м·с² (T = capacity · ω²).
    pub capacity: f32,
    /// Есть ли блокировка гидротрансформатора.
    pub lockup: bool,
    /// Минимальная скорость турбины для блокировки, рад/с.
    pub lockup_speed: f32,
    /// Ёмкость блокировки, Н·м.
    pub lockup_capacity: f32,
}

impl Default for TorqueConverterConfig {
    fn default() -> Self {
        Self {
            stall_ratio: 2.1,
            coupling_ratio: 0.86,
            capacity: 0.007,
            lockup: true,
            lockup_speed: 55.0,
            lockup_capacity: 480.0,
        }
    }
}

/// Состояние гидротрансформатора.
#[derive(Clone, Copy, Debug, Default)]
pub struct TorqueConverter {
    /// Скорость насоса (двигателя), рад/с.
    pub pump_omega: f32,
    /// Скорость турбины (вход КПП), рад/с.
    pub turbine_omega: f32,
    /// Текущий коэффициент трансформации момента.
    pub torque_ratio: f32,
    /// Момент насоса (нагрузка на двигатель), Н·м.
    pub pump_torque: f32,
    /// Момент турбины (на выходе), Н·м.
    pub turbine_torque: f32,
    /// Включена ли блокировка.
    pub locked: bool,
}

impl TorqueConverter {
    /// Шаг гидротрансформатора. Возвращает момент на турбине (вход КПП), Н·м.
    ///
    /// * `pump_omega` — угловая скорость двигателя.
    /// * `turbine_omega` — угловая скорость входного вала КПП.
    pub fn step(
        &mut self,
        cfg: &TorqueConverterConfig,
        pump_omega: f32,
        turbine_omega: f32,
    ) -> f32 {
        let pump = pump_omega.max(0.0);
        self.pump_omega = pump;
        self.turbine_omega = turbine_omega;

        // Отношение скоростей 0..1.
        let sr = if pump > 1.0 {
            (turbine_omega / pump).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // Коэффициент трансформации: максимум при стопе, 1 при сцеплении.
        let tr = 1.0
            + (cfg.stall_ratio - 1.0)
                * (1.0 - (sr / cfg.coupling_ratio.max(1e-3))).clamp(0.0, 1.0);
        self.torque_ratio = tr;

        // Ёмкость насоса падает с ростом отношения скоростей.
        let capacity = cfg.capacity * (1.0 - 0.55 * sr);
        let pump_torque = (capacity * pump * pump).max(0.0);
        self.pump_torque = pump_torque;
        let mut turbine_torque = pump_torque * tr;

        // Блокировка: жёсткая связь насоса и турбины.
        self.locked = false;
        if cfg.lockup
            && sr > cfg.coupling_ratio * 0.97
            && turbine_omega.abs() > cfg.lockup_speed
        {
            let lock = ((pump - turbine_omega) * 80.0)
                .clamp(-cfg.lockup_capacity, cfg.lockup_capacity);
            self.locked = true;
            self.pump_torque += lock;
            turbine_torque = (turbine_torque + lock).max(0.0);
        }

        self.turbine_torque = turbine_torque;
        turbine_torque
    }
}

/// Конфигурация коробки передач.
#[derive(Clone, Debug)]
pub struct TransmissionConfig {
    /// Передаточные числа вперёд.
    pub ratios: Vec<f32>,
    /// Передаточное число заднего хода (отрицательное).
    pub reverse: f32,
    /// Главная пара.
    pub final_drive: f32,
    /// КПД.
    pub efficiency: f32,
    /// Время переключения, с.
    pub shift_time: f32,
    /// Автоматическая коробка.
    pub automatic: bool,
    /// Обороты переключения вверх.
    pub upshift_rpm: f32,
    /// Обороты переключения вниз.
    pub downshift_rpm: f32,
    /// Порог газа для кикдауна.
    pub kickdown_throttle: f32,
    /// Предельный момент, Н·м.
    pub max_torque: f32,
    /// Приведённая инерция валов, кг·м².
    pub inertia: f32,
    /// Гидротрансформатор (для автомата).
    pub converter: Option<TorqueConverterConfig>,
    /// Максимальная скорость включения заднего хода, м/с.
    pub reverse_max_speed: f32,
}

impl Default for TransmissionConfig {
    fn default() -> Self {
        Self {
            ratios: vec![3.4, 2.1, 1.5, 1.12, 0.9, 0.75],
            reverse: -3.3,
            final_drive: 3.9,
            efficiency: 0.92,
            shift_time: 0.25,
            automatic: false,
            upshift_rpm: 6400.0,
            downshift_rpm: 2400.0,
            kickdown_throttle: 0.7,
            max_torque: 450.0,
            inertia: 0.02,
            converter: None,
            reverse_max_speed: 3.0,
        }
    }
}

impl TransmissionConfig {
    /// Полное передаточное число для передачи (с главной парой).
    pub fn ratio_for_gear(&self, gear: i32) -> f32 {
        if gear < 0 {
            return self.reverse * self.final_drive;
        }
        if gear == 0 {
            return 0.0;
        }
        let idx = (gear - 1).clamp(0, self.ratios.len() as i32 - 1) as usize;
        self.ratios[idx] * self.final_drive
    }

    /// Отношение передаточных чисел между двумя передачами.
    fn step_ratio(&self, from: i32, to: i32) -> f32 {
        let a = self.ratio_for_gear(from).abs();
        let b = self.ratio_for_gear(to).abs();
        if a > 1e-6 {
            b / a
        } else {
            1.0
        }
    }
}

/// Состояние коробки передач.
#[derive(Clone, Debug)]
pub struct Transmission {
    /// Передача: -1 задний ход, 0 нейтраль, 1..N вперёд.
    pub gear: i32,
    /// Оставшееся время переключения, с.
    pub shift_timer: f32,
    /// Скорость входного вала, рад/с.
    pub input_omega: f32,
    /// Момент на выходе, Н·м.
    pub output_torque: f32,
    /// Износ/повреждение 0..1.
    pub damage: f32,
    /// Коробка сломана.
    pub broken: bool,
    /// Гидротрансформатор.
    pub converter: TorqueConverter,
    /// Направление последнего переключения (-1 вниз, +1 вверх).
    pub last_shift: i32,
}

impl Default for Transmission {
    fn default() -> Self {
        Self {
            gear: 1,
            shift_timer: 0.0,
            input_omega: 0.0,
            output_torque: 0.0,
            damage: 0.0,
            broken: false,
            converter: TorqueConverter::default(),
            last_shift: 0,
        }
    }
}

impl Transmission {
    pub fn new() -> Self {
        Self::default()
    }

    /// Текущее полное передаточное число.
    #[inline]
    pub fn ratio(&self, cfg: &TransmissionConfig) -> f32 {
        cfg.ratio_for_gear(self.gear)
    }

    /// Идёт ли переключение.
    #[inline]
    pub fn shifting(&self) -> bool {
        self.shift_timer > 0.0
    }

    /// Обновляет передачу (механика/автомат).
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        cfg: &TransmissionConfig,
        shift_up: bool,
        shift_down: bool,
        throttle: f32,
        engine_rpm: f32,
        speed: f32,
        dt: f32,
    ) {
        self.shift_timer = (self.shift_timer - dt).max(0.0);
        if self.broken {
            self.gear = 0;
            return;
        }
        let max_gear = cfg.ratios.len() as i32;

        if !cfg.automatic {
            if self.shift_timer > 0.0 {
                return;
            }
            if shift_up && self.gear < max_gear {
                self.gear += 1;
                self.shift_timer = cfg.shift_time;
                self.last_shift = 1;
            } else if shift_down && self.gear > -1 {
                let target = self.gear - 1;
                // В задний ход нельзя на ходу вперёд.
                if target >= 0 || speed.abs() < cfg.reverse_max_speed {
                    self.gear = target;
                    self.shift_timer = cfg.shift_time;
                    self.last_shift = -1;
                }
            }
            return;
        }

        // --- Автоматическая КПП ---
        if self.gear < 0 {
            self.gear = 0;
        }
        if self.shift_timer > 0.0 {
            return;
        }
        if self.gear == 0 {
            if throttle > 0.02 || speed.abs() > 0.5 {
                self.gear = 1;
                self.shift_timer = cfg.shift_time * 0.5;
                self.last_shift = 1;
            }
            return;
        }

        let want_up = engine_rpm > cfg.upshift_rpm;
        let want_down = engine_rpm < cfg.downshift_rpm;
        let kickdown =
            throttle > cfg.kickdown_throttle && self.gear > 1 && engine_rpm < cfg.downshift_rpm * 1.7;

        if want_up && self.gear < max_gear {
            let post = engine_rpm * cfg.step_ratio(self.gear, self.gear + 1);
            if post > cfg.downshift_rpm * 0.9 {
                self.gear += 1;
                self.shift_timer = cfg.shift_time;
                self.last_shift = 1;
            }
        } else if (want_down || kickdown) && self.gear > 1 {
            let post = engine_rpm * cfg.step_ratio(self.gear, self.gear - 1);
            if post < cfg.upshift_rpm * 1.08 {
                self.gear -= 1;
                self.shift_timer = cfg.shift_time;
                self.last_shift = -1;
            }
        }
    }

    /// Обороты входного вала при данной скорости ведущих колёс.
    #[inline]
    pub fn input_omega_for(&self, cfg: &TransmissionConfig, driven_omega: f32) -> f32 {
        driven_omega * self.ratio(cfg)
    }
}

/// Обороты из рад/с.
#[inline]
pub fn rpm_from_omega(omega: f32) -> f32 {
    omega * 60.0 / TAU
}

/// Рад/с из оборотов.
#[inline]
pub fn omega_from_rpm(rpm: f32) -> f32 {
    rpm * TAU / 60.0
}
