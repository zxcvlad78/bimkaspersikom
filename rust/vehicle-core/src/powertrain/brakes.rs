//! Тормоза: тормозной момент, нагрев и фейд, ABS, ручной тормоз, износ.

/// Общая конфигурация тормозной системы.
#[derive(Clone, Copy, Debug)]
pub struct BrakeConfig {
    /// Теплоёмкость диска, Дж/К.
    pub thermal_mass: f32,
    /// Температура начала фейда, °C.
    pub fade_temp: f32,
    /// Максимальная рабочая температура, °C.
    pub max_temp: f32,
    /// Коэффициент охлаждения, Вт/К.
    pub cooling: f32,
    /// Порог продольного скольжения для ABS.
    pub abs_slip: f32,
    /// Включён ли ABS.
    pub abs: bool,
}

impl Default for BrakeConfig {
    fn default() -> Self {
        Self {
            thermal_mass: 42_000.0,
            fade_temp: 450.0,
            max_temp: 800.0,
            cooling: 24.0,
            abs_slip: 0.16,
            abs: true,
        }
    }
}

/// Параметры тормоза конкретного колеса.
#[derive(Clone, Copy, Debug)]
pub struct BrakeWheel {
    /// Максимальный тормозной момент, Н·м.
    pub max_torque: f32,
    /// Есть ли ручной тормоз (обычно задние колёса).
    pub handbrake: bool,
}

impl Default for BrakeWheel {
    fn default() -> Self {
        Self {
            max_torque: 3500.0,
            handbrake: false,
        }
    }
}

/// Состояние тормозов (по колёсам).
#[derive(Clone, Copy, Debug, Default)]
pub struct Brakes {
    /// Температура диска, °C.
    pub temperature: [f32; 4],
    /// Износ колодок 0..1.
    pub wear: [f32; 4],
    /// Сработал ли ABS на колесе.
    pub abs_active: [bool; 4],
}

impl Brakes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Шаг тормозов. Возвращает тормозной момент по колёсам, Н·м.
    ///
    /// * `brake`, `handbrake` — 0..1.
    /// * `omega` — угловые скорости колёс, рад/с.
    /// * `slip` — продольное скольжение колёс (для ABS).
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        cfg: &BrakeConfig,
        wheels: &[BrakeWheel; 4],
        brake: f32,
        handbrake: f32,
        omega: &[f32; 4],
        slip: &[f32; 4],
        dt: f32,
    ) -> [f32; 4] {
        let mut out = [0.0f32; 4];
        let brake = brake.clamp(0.0, 1.0);
        let handbrake = handbrake.clamp(0.0, 1.0);

        for i in 0..4 {
            let w = &wheels[i];

            // Фейд от температуры и износа.
            let fade_range = (cfg.max_temp - cfg.fade_temp).max(1.0);
            let overheat = ((self.temperature[i] - cfg.fade_temp) / fade_range).clamp(0.0, 1.0);
            let fade = (1.0 - overheat * 0.65) * (1.0 - 0.5 * self.wear[i]);

            let mut torque = w.max_torque * brake * fade;
            if w.handbrake {
                torque += w.max_torque * 0.6 * handbrake * fade;
            }

            // ABS: снижает момент, чтобы вернуть скольжение к порогу.
            self.abs_active[i] = false;
            if cfg.abs && brake > 0.05 {
                let s = slip[i].abs();
                if s > cfg.abs_slip {
                    let scale = (cfg.abs_slip / s).clamp(0.15, 1.0);
                    torque *= scale;
                    self.abs_active[i] = true;
                }
            }

            out[i] = torque;

            // Нагрев: мощность трения минус охлаждение.
            let power = torque * omega[i].abs();
            let cool = cfg.cooling * (self.temperature[i] - 25.0);
            self.temperature[i] = (self.temperature[i]
                + (power - cool) / cfg.thermal_mass.max(1.0) * dt)
                .clamp(-40.0, 1200.0);

            // Износ при перегреве.
            if self.temperature[i] > cfg.fade_temp {
                self.wear[i] = (self.wear[i] + dt * 1e-4).min(1.0);
            }
        }

        out
    }

    /// Максимальная температура дисков, °C.
    pub fn max_temperature(&self) -> f32 {
        self.temperature.iter().copied().fold(0.0f32, f32::max)
    }
}
