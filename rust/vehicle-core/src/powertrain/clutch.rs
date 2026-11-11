//! Сцепление: момент трения, проскальзывание, нагрев и фейд, износ.

/// Параметры сцепления.
#[derive(Clone, Copy, Debug)]
pub struct ClutchConfig {
    /// Максимальный передаваемый момент, Н·м.
    pub capacity: f32,
    /// Жёсткость (Н·м на рад/с) — как быстро выравниваются скорости.
    pub stiffness: f32,
    /// Теплоёмкость, Дж/К.
    pub thermal_mass: f32,
    /// Температура начала фейда, °C.
    pub fade_temp: f32,
    /// Скорость износа.
    pub wear_rate: f32,
}

impl Default for ClutchConfig {
    fn default() -> Self {
        Self {
            capacity: 420.0,
            stiffness: 60.0,
            thermal_mass: 25_000.0,
            fade_temp: 220.0,
            wear_rate: 0.02,
        }
    }
}

/// Состояние сцепления.
#[derive(Clone, Copy, Debug, Default)]
pub struct Clutch {
    /// Степень включения 0..1 (0 — выжато, 1 — отпущено).
    pub engagement: f32,
    pub temperature: f32,
    pub wear: f32,
    /// Проскальзывание, рад/с.
    pub slip: f32,
    /// Передаваемый момент, Н·м.
    pub transmitted: f32,
}

impl Clutch {
    /// Шаг сцепления. `pedal` — 0 (выжато) .. 1 (отпущено).
    /// `engine_omega` и `input_omega` — скорости двигателя и вала КПП.
    /// Возвращает передаваемый момент (Н·м) — нагрузку на двигатель.
    pub fn step(
        &mut self,
        cfg: &ClutchConfig,
        engine_omega: f32,
        input_omega: f32,
        pedal: f32,
        dt: f32,
    ) -> f32 {
        // Нелинейная характеристика: в конце хода педали схватывает резко.
        let pedal = pedal.clamp(0.0, 1.0);
        self.engagement = (pedal * pedal * (3.0 - 2.0 * pedal)).clamp(0.0, 1.0);

        // Фейд при перегреве.
        let fade = (1.0 - ((self.temperature - cfg.fade_temp) / 100.0).max(0.0)).clamp(0.25, 1.0);
        let capacity = cfg.capacity * self.engagement * fade * (1.0 - 0.6 * self.wear);

        let omega_diff = engine_omega - input_omega;
        self.slip = omega_diff;
        let torque = (omega_diff * cfg.stiffness).clamp(-capacity, capacity);
        self.transmitted = torque;

        // Нагрев от мощности проскальзывания.
        let power = (torque * omega_diff).abs();
        let cooling = (self.temperature - 20.0) * 0.6;
        self.temperature =
            (self.temperature + (power - cooling) / cfg.thermal_mass * dt).clamp(-40.0, 450.0);
        self.wear = (self.wear + power * dt * cfg.wear_rate * 1e-6).min(1.0);

        torque
    }

    /// Полностью выжато (используется при переключении).
    pub fn disengage(&mut self) {
        self.engagement = 0.0;
        self.transmitted = 0.0;
    }
}
