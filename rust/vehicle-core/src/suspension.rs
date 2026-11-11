//! Подвеска: пружина, амортизатор (раздельно сжатие/отбой), отбойник,
//! кинематика развала и схождения от хода.

/// Параметры подвески одного колеса.
#[derive(Clone, Copy, Debug)]
pub struct SuspensionConfig {
    /// Длина в покое (крепление -> центр колеса), м.
    pub rest_length: f32,
    /// Максимальная длина (полный отбой), м.
    pub max_length: f32,
    /// Минимальная длина (полное сжатие), м.
    pub min_length: f32,
    /// Жёсткость пружины, Н/м.
    pub spring_rate: f32,
    /// Демпфирование на сжатии, Н·с/м.
    pub damper_compression: f32,
    /// Демпфирование на отбое, Н·с/м.
    pub damper_rebound: f32,
    /// Жёсткость отбойника, Н/м.
    pub bump_stop_rate: f32,
    /// Жёсткость стабилизатора поперечной устойчивости, Н/м разницы сжатия.
    pub anti_roll: f32,
    /// Прирост развала, градусы на метр сжатия.
    pub camber_gain: f32,
    /// Прирост схождения, градусы на метр сжатия.
    pub toe_gain: f32,
}

impl Default for SuspensionConfig {
    fn default() -> Self {
        Self {
            rest_length: 0.32,
            max_length: 0.42,
            min_length: 0.12,
            spring_rate: 38000.0,
            damper_compression: 3200.0,
            damper_rebound: 4600.0,
            bump_stop_rate: 180000.0,
            anti_roll: 6000.0,
            camber_gain: -2.0,
            toe_gain: 0.0,
        }
    }
}

/// Динамическое состояние подвески.
#[derive(Clone, Copy, Debug, Default)]
pub struct Suspension {
    /// Текущая длина (крепление -> центр колеса), м.
    pub length: f32,
    /// Скорость изменения длины, м/с (>0 — разжатие).
    pub velocity: f32,
    /// Есть ли контакт колеса с поверхностью.
    pub contact: bool,
}

impl Suspension {
    pub fn new(rest_length: f32) -> Self {
        Self {
            length: rest_length,
            velocity: 0.0,
            contact: false,
        }
    }

    /// Сжатие относительно покоя, м (>=0).
    #[inline]
    pub fn compression(&self, cfg: &SuspensionConfig) -> f32 {
        (cfg.rest_length - self.length).max(0.0)
    }

    /// Сила, передаваемая кузову вдоль оси подвески (положительная — вверх), Н.
    pub fn force(&self, cfg: &SuspensionConfig) -> f32 {
        let compression = (cfg.rest_length - self.length).max(0.0);
        let mut force = cfg.spring_rate * compression;

        // Отбойник: резкий рост жёсткости в конце хода сжатия.
        if self.length < cfg.min_length {
            force += cfg.bump_stop_rate * (cfg.min_length - self.length);
        }

        // Амортизатор: разное демпфирование на сжатии и отбое.
        let damper = if self.velocity < 0.0 {
            cfg.damper_compression
        } else {
            cfg.damper_rebound
        };
        force -= damper * self.velocity;

        force.max(0.0)
    }

    /// Текущий развал, рад (статический + от хода подвески).
    #[inline]
    pub fn camber(&self, cfg: &SuspensionConfig, static_camber_rad: f32) -> f32 {
        static_camber_rad + (cfg.camber_gain * self.compression(cfg)).to_radians()
    }

    /// Текущее схождение, рад.
    #[inline]
    pub fn toe(&self, cfg: &SuspensionConfig, static_toe_rad: f32) -> f32 {
        static_toe_rad + (cfg.toe_gain * self.compression(cfg)).to_radians()
    }
}
