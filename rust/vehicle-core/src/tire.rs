//! Модель шины: Pacejka "Magic Formula" с комбинированным скольжением
//! (метод подобия), длиной релаксации, чувствительностью к нагрузке,
//! усилием от развала, выравнивающим моментом, износом и температурой.

use glam::Vec2;

use crate::sign;

/// Параметры шины. Значения по умолчанию — шоссейная спортивная шина.
#[derive(Clone, Copy, Debug)]
pub struct TireConfig {
    /// Базовый коэффициент трения (μ) по сухому асфальту.
    pub friction: f32,
    /// Продольная жёсткость (B в Magic Formula).
    pub long_stiffness: f32,
    /// Продольная форма кривой (C).
    pub long_shape: f32,
    /// Продольная кривизна (E).
    pub long_curvature: f32,
    /// Боковая жёсткость (B).
    pub lat_stiffness: f32,
    /// Боковая форма кривой (C).
    pub lat_shape: f32,
    /// Боковая кривизна (E).
    pub lat_curvature: f32,
    /// Падение μ с ростом нагрузки (0 — нет).
    pub load_sensitivity: f32,
    /// Номинальная нагрузка Fz0, Н.
    pub reference_load: f32,
    /// Длина релаксации скольжения, м (0 — мгновенно).
    pub relaxation_length: f32,
    /// Жёсткость усилия от развала (на градус и на Н нагрузки).
    pub camber_stiffness: f32,
    /// Коэффициент сопротивления качению.
    pub rolling_resistance: f32,
    /// Плечо выравнивающего момента, м.
    pub pneumatic_trail: f32,
    /// Радиус, м.
    pub radius: f32,
    /// Ширина, м.
    pub width: f32,
    /// Масса шины (с диском учитывается отдельно), кг.
    pub mass: f32,
    /// Скорость износа: доля за секунду при полном срыве.
    pub wear_rate: f32,
    /// Оптимальная температура, °C.
    pub optimal_temperature: f32,
}

impl Default for TireConfig {
    fn default() -> Self {
        Self {
            friction: 1.55,
            long_stiffness: 10.0,
            long_shape: 1.65,
            long_curvature: 0.6,
            lat_stiffness: 8.5,
            lat_shape: 1.45,
            lat_curvature: 0.75,
            load_sensitivity: 0.09,
            reference_load: 4000.0,
            relaxation_length: 0.35,
            camber_stiffness: 0.0016,
            rolling_resistance: 0.014,
            pneumatic_trail: 0.05,
            radius: 0.33,
            width: 0.225,
            mass: 10.0,
            wear_rate: 0.02,
            optimal_temperature: 90.0,
        }
    }
}

/// Результат работы модели шины.
#[derive(Clone, Copy, Debug, Default)]
pub struct TireOutput {
    /// Сила в системе шины: x — вперёд, y — вправо, Н.
    pub force: Vec2,
    /// Выравнивающий момент вокруг вертикали, Н·м.
    pub aligning_torque: f32,
    /// Коэффициент скольжения (для ABS/TCS), -1..+N.
    pub slip_ratio: f32,
    /// Угол скольжения, рад.
    pub slip_angle: f32,
    /// Насыщение сцепления: длина вектора / предел (>=1 — скольжение).
    pub utilization: f32,
}

/// Динамическое состояние шины (сглаженные скольжения, износ, температура).
#[derive(Clone, Copy, Debug)]
pub struct TireState {
    /// Отфильтрованный коэффициент скольжения.
    pub slip_ratio: f32,
    /// Отфильтрованный угол скольжения, рад.
    pub slip_angle: f32,
    /// Износ 0..1 (1 — лысая).
    pub wear: f32,
    /// Температура, °C.
    pub temperature: f32,
    /// Накопленное проскальзывание для эффектов.
    pub slip_distance: f32,
}

impl Default for TireState {
    fn default() -> Self {
        Self {
            slip_ratio: 0.0,
            slip_angle: 0.0,
            wear: 0.0,
            temperature: 20.0,
            slip_distance: 0.0,
        }
    }
}

#[inline]
fn magic_formula(b: f32, c: f32, d: f32, e: f32, slip: f32) -> f32 {
    let bs = b * slip;
    d * (c * (bs - e * (bs - bs.atan())).atan()).sin()
}

impl TireState {
    /// Сбрасывает динамику при снятии колеса.
    pub fn reset(&mut self) {
        self.slip_ratio = 0.0;
        self.slip_angle = 0.0;
        self.slip_distance = 0.0;
    }

    /// Текущий множитель сцепления от износа и температуры.
    fn condition_factor(&self, cfg: &TireConfig) -> f32 {
        // Износ: до 30% сцепления теряется к полному износу.
        let wear_factor = 1.0 - 0.3 * self.wear.clamp(0.0, 1.0);
        // Температура: гауссово окно вокруг оптимума.
        let dt = (self.temperature - cfg.optimal_temperature) / 60.0;
        let temp_factor = (1.0 - 0.25 * dt * dt).clamp(0.6, 1.08);
        wear_factor * temp_factor
    }

    /// Считает силу в системе шины.
    ///
    /// * `vx`, `vy` — скорость точки контакта в системе шины (вперёд/вправо).
    /// * `omega` — угловая скорость колеса, рад/с.
    /// * `camber` — развал, рад (положительный — верх колеса наружу).
    /// * `fz` — нормальная нагрузка, Н.
    /// * `surface_friction` — трение поверхности.
    #[allow(clippy::too_many_arguments)]
    pub fn compute(
        &mut self,
        cfg: &TireConfig,
        dt: f32,
        vx: f32,
        vy: f32,
        omega: f32,
        camber: f32,
        fz: f32,
        surface_friction: f32,
    ) -> TireOutput {
        let out = TireOutput::default();
        if fz <= 1.0 {
            self.slip_ratio = 0.0;
            self.slip_angle = 0.0;
            return out;
        }

        // Мгновенные (steady-state) скольжения.
        let v_abs = vx.abs().max(1.0);
        let kappa = ((omega * cfg.radius - vx) / v_abs).clamp(-0.95, 20.0);
        let alpha = (vy / v_abs).atan();

        // Фильтр длины релаксации.
        let blend = if cfg.relaxation_length > 1e-3 {
            ((v_abs * dt / cfg.relaxation_length).min(1.0)).max(0.0)
        } else {
            1.0
        };
        self.slip_ratio += (kappa - self.slip_ratio) * blend;
        self.slip_angle += (alpha - self.slip_angle) * blend;

        let k = self.slip_ratio;
        let a = self.slip_angle;

        // Комбинированное скольжение (метод подобия Pacejka).
        let sx = k / (1.0 + k);
        let sy = -a.tan() / (1.0 + k);
        let s = (sx * sx + sy * sy).sqrt();

        // Пиковая сила с учётом нагрузки, поверхности, износа и температуры.
        let load_ratio = fz / cfg.reference_load.max(1.0);
        let load_factor = (1.0 - cfg.load_sensitivity * (load_ratio - 1.0)).clamp(0.4, 1.4);
        let mu = cfg.friction * load_factor * surface_friction * self.condition_factor(cfg);
        let peak = mu * fz;

        let mut force = Vec2::ZERO;
        let mut utilization = 0.0;
        if s > 1e-5 {
            let fx = (sx / s) * magic_formula(cfg.long_stiffness, cfg.long_shape, peak, cfg.long_curvature, s);
            let fy = (sy / s) * magic_formula(cfg.lat_stiffness, cfg.lat_shape, peak, cfg.lat_curvature, s);
            force = Vec2::new(fx, fy);
            utilization = force.length() / peak.max(1.0);
        }

        // Усилие от развала (camber thrust), действует вбок.
        force.y += cfg.camber_stiffness * camber.to_degrees() * fz;

        // Выравнивающий момент: падает с ростом угла скольжения.
        let trail_scale = (1.0 - (a.abs() / 0.35).min(1.0)).max(0.0);
        let aligning = -cfg.pneumatic_trail * trail_scale * force.y;

        // Износ и нагрев от проскальзывания.
        let slip_mag = (k.abs() + a.abs() * 2.0).min(3.0);
        let distance = v_abs * dt;
        self.slip_distance += distance * slip_mag;
        if cfg.wear_rate > 0.0 {
            self.wear = (self.wear + cfg.wear_rate * slip_mag * dt * 0.1).min(1.0);
        }
        let heating = slip_mag * distance * 0.02;
        let cooling = (self.temperature - 20.0) * 0.05 * dt;
        self.temperature = (self.temperature + heating - cooling).clamp(-40.0, 250.0);

        TireOutput {
            force,
            aligning_torque: aligning,
            slip_ratio: k,
            slip_angle: a,
            utilization,
        }
    }
}

/// Сила сопротивления качению, Н.
#[inline]
pub fn rolling_resistance(cfg: &TireConfig, fz: f32, vx: f32) -> f32 {
    -sign(vx) * cfg.rolling_resistance * fz * (vx.abs() / 0.5).min(1.0)
}

/// Инерция вращения колеса (шина + диск) для заданной массы, кг·м².
#[inline]
pub fn wheel_inertia(radius: f32, mass: f32) -> f32 {
    (0.5 * mass * radius * radius).max(0.05)
}
