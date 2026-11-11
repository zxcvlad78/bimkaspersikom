//! Дифференциалы: свободный, блокируемый, самоблокирующийся (дисковый),
//! червячный (Torsen) и центральный дифференциал (распределение между осями).

/// Тип дифференциала.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DifferentialKind {
    /// Свободный: момент делится поровну, скорости независимы.
    Open,
    /// Полностью заблокированный: колёса вращаются синхронно.
    Locked,
    /// Самоблокирующийся (дисковый): преднатяг + жёсткость связи.
    Lsd {
        /// Преднатяжение, Н·м.
        preload: f32,
        /// Жёсткость связи, Н·м·с/рад.
        coupling: f32,
        /// Максимальный момент блокировки как доля входного.
        bias_ratio: f32,
    },
    /// Червячный (Torsen): ограниченный коэффициент распределения.
    Torsen {
        /// Коэффициент блокировки (0.7 = 70/30).
        bias_ratio: f32,
        /// Преднатяжение, Н·м.
        preload: f32,
    },
}

impl Default for DifferentialKind {
    fn default() -> Self {
        DifferentialKind::Open
    }
}

/// Жёсткость связи полностью заблокированного дифференциала, Н·м·с/рад.
const LOCK_COUPLING: f32 = 8000.0;
/// Жёсткость связи Torsen, Н·м·с/рад.
const TORSEN_COUPLING: f32 = 2500.0;

/// Считает момент связи по разнице скоростей и её пределу.
#[inline]
fn coupling_torque(kind: &DifferentialKind, input_torque: f32, speed_diff: f32) -> f32 {
    let (raw, max) = match *kind {
        DifferentialKind::Open => (0.0, 0.0),
        DifferentialKind::Locked => (
            speed_diff * LOCK_COUPLING,
            input_torque.abs() * 4.0 + 500.0,
        ),
        DifferentialKind::Lsd {
            preload,
            coupling,
            bias_ratio,
        } => (
            speed_diff * coupling,
            preload + input_torque.abs() * bias_ratio,
        ),
        DifferentialKind::Torsen {
            bias_ratio,
            preload,
        } => (
            speed_diff * TORSEN_COUPLING,
            preload + input_torque.abs() * bias_ratio,
        ),
    };
    raw.clamp(-max, max)
}

/// Распределяет входной момент между двумя колёсами одной оси.
///
/// `omega_a`, `omega_b` — угловые скорости колёс. Возвращает моменты `(a, b)`.
/// Если колесо `a` вращается быстрее, дифференциал перебрасывает момент на `b`.
#[inline]
pub fn distribute(
    kind: &DifferentialKind,
    input_torque: f32,
    omega_a: f32,
    omega_b: f32,
) -> (f32, f32) {
    let half = input_torque * 0.5;
    let c = coupling_torque(kind, input_torque, omega_a - omega_b);
    (half - c, half + c)
}

/// Распределяет входной момент между осями (центральный дифференциал).
///
/// `front_bias` — доля момента на переднюю ось при свободном дифференциале.
/// Возвращает `(front, rear)`.
#[inline]
pub fn distribute_center(
    kind: &DifferentialKind,
    input_torque: f32,
    front_bias: f32,
    omega_front: f32,
    omega_rear: f32,
) -> (f32, f32) {
    let bias = front_bias.clamp(0.0, 1.0);
    let front = input_torque * bias;
    let rear = input_torque * (1.0 - bias);
    let c = coupling_torque(kind, input_torque, omega_front - omega_rear);
    (front - c, rear + c)
}
