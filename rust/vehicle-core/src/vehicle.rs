//! Симуляция автомобиля: подвеска (трассировка лучей), шины, аэродинамика,
//! рулевое управление с Аккерманом, стабилизаторы поперечной устойчивости,
//! ABS/TCS/ESC и трансмиссия.
//!
//! Ядро не интегрирует положение кузова само: оно считает суммарную силу и
//! момент в мировых координатах, которые хост (Godot `RigidBody3D`) применяет
//! к телу. Так столкновения и общую динамику мира считает физический движок.
//!
//! ## Система координат
//! Как в Godot: `+X` вправо, `+Y` вверх, `-Z` вперёд. Начало локальных
//! координат машины — на земле по центру базы.

use glam::{Quat, Vec3};

use crate::assembly::{VehicleConfig, WheelConfig};
use crate::parts::Corner;
use crate::powertrain::{Powertrain, PowertrainInputs, WheelRole, WheelState, FL, FR, RL, RR};
use crate::suspension::Suspension;
use crate::tire::{rolling_resistance, TireState};
use crate::{normalized_or, ContactProvider};

/// Индекс колеса в массивах `[FL, FR, RL, RR]`.
#[inline]
pub fn corner_index(c: Corner) -> usize {
    match c {
        Corner::FrontLeft => FL,
        Corner::FrontRight => FR,
        Corner::RearLeft => RL,
        Corner::RearRight => RR,
    }
}

/// Динамическое состояние одного колеса.
#[derive(Clone, Debug)]
pub struct Wheel {
    pub config: WheelConfig,
    pub suspension: Suspension,
    pub tire: TireState,
    /// Угловая скорость, рад/с (>0 — вперёд).
    pub omega: f32,
    /// Угол поворота (руль), рад.
    pub steer_angle: f32,
    /// Накопленный угол вращения (для отрисовки), рад.
    pub spin_angle: f32,
    /// Есть контакт с поверхностью.
    pub contact: bool,
    /// Нормальная нагрузка, Н.
    pub load: f32,
    /// Точка контакта (локально).
    pub contact_local: Vec3,
    /// Точка контакта (мировые).
    pub contact_world: Vec3,
    /// Нормаль контакта (мировые).
    pub contact_normal: Vec3,
    /// Центр колеса (локально).
    pub center_local: Vec3,
    /// Длина подвески на прошлом шаге.
    pub prev_length: f32,
    /// Сила шины (мировые), Н.
    pub tire_force: Vec3,
    /// Продольная сила шины (включая сопротивление качению), Н.
    pub last_long_force: f32,
    /// Продольная скорость в системе шины, м/с.
    pub last_vx: f32,
    /// Выравнивающий момент, Н·м.
    pub last_align: f32,
    /// Трение поверхности под колесом.
    pub surface: f32,
}

impl Wheel {
    fn new(config: WheelConfig) -> Self {
        let rest = config.suspension.rest_length;
        Self {
            suspension: Suspension::new(rest),
            tire: TireState::default(),
            omega: 0.0,
            steer_angle: 0.0,
            spin_angle: 0.0,
            contact: false,
            load: 0.0,
            contact_local: config.position,
            contact_world: Vec3::ZERO,
            contact_normal: Vec3::Y,
            center_local: config.position + Vec3::NEG_Y * rest,
            prev_length: rest,
            tire_force: Vec3::ZERO,
            last_long_force: 0.0,
            last_vx: 0.0,
            last_align: 0.0,
            surface: 1.0,
            config,
        }
    }

    /// Текущий развал, рад.
    #[inline]
    fn camber(&self) -> f32 {
        self.suspension
            .camber(&self.config.suspension, self.config.static_camber_rad)
    }

    /// Текущее схождение, рад.
    #[inline]
    fn toe(&self) -> f32 {
        self.suspension
            .toe(&self.config.suspension, self.config.static_toe_rad)
    }

    /// Скорость вращения для отрисовки, об/мин.
    #[inline]
    pub fn rpm(&self) -> f32 {
        self.omega * 60.0 / std::f32::consts::TAU
    }
}

/// Точка контакта шины с поверхностью (для эффектов и звука).
#[derive(Clone, Copy, Debug, Default)]
pub struct ContactPoint {
    pub position: Vec3,
    pub normal: Vec3,
    pub load: f32,
    pub slip_ratio: f32,
    pub slip_angle: f32,
    pub surface: f32,
}

/// Результат шага симуляции.
#[derive(Clone, Debug)]
pub struct VehicleStep {
    /// Суммарная сила в мировых координатах, Н.
    pub force: Vec3,
    /// Суммарный момент в мировых координатах, Н·м.
    pub torque: Vec3,
    /// Контакты шин.
    pub contacts: Vec<ContactPoint>,
    /// Мировые центры колёс (для отрисовки).
    pub wheel_centers: [Vec3; 4],
    /// Углы вращения колёс, рад.
    pub wheel_spin: [f32; 4],
    /// Углы поворота колёс, рад.
    pub wheel_steer: [f32; 4],
    /// Контакт колеса.
    pub wheel_contact: [bool; 4],
    /// Нагрузка на колесо, Н.
    pub wheel_load: [f32; 4],
    /// Продольное скольжение колеса.
    pub wheel_slip: [f32; 4],
    /// Сработала ли ESC.
    pub esc_active: bool,
}

impl Default for VehicleStep {
    fn default() -> Self {
        Self {
            force: Vec3::ZERO,
            torque: Vec3::ZERO,
            contacts: Vec::new(),
            wheel_centers: [Vec3::ZERO; 4],
            wheel_spin: [0.0; 4],
            wheel_steer: [0.0; 4],
            wheel_contact: [false; 4],
            wheel_load: [0.0; 4],
            wheel_slip: [0.0; 4],
            esc_active: false,
        }
    }
}

/// Телеметрия автомобиля.
#[derive(Clone, Copy, Debug, Default)]
pub struct Telemetry {
    /// Продольная скорость, м/с.
    pub speed: f32,
    /// Продольная скорость, км/ч.
    pub speed_kmh: f32,
    /// Продольное ускорение, g.
    pub longitudinal_g: f32,
    /// Поперечное ускорение, g.
    pub lateral_g: f32,
    /// Скорость рыскания, рад/с.
    pub yaw_rate: f32,
    /// Колёс на земле.
    pub wheels_on_ground: u8,
    /// Угол скольжения кузова, рад.
    pub slip_angle: f32,
    /// Избыточная/недостаточная поворачиваемость (>0 — снос передней оси).
    pub understeer: f32,
}

/// Автомобиль целиком.
#[derive(Clone, Debug)]
pub struct Vehicle {
    pub config: VehicleConfig,
    pub wheels: Vec<Wheel>,
    pub powertrain: Powertrain,
    /// Индекс колеса по углу (None, если колеса нет).
    pub index_by_corner: [Option<usize>; 4],
    pub wheelbase: f32,
    pub track_front: f32,
    pub track_rear: f32,
    /// Текущий угол руля, рад.
    pub steer_angle: f32,
    /// Плотность воздуха, кг/м³.
    pub air_density: f32,
    /// Маска столкновений для трассировки лучей.
    pub collision_mask: u32,
    /// Число внутренних подшагов для колёс и трансмиссии.
    pub substeps: u32,
    /// Телеметрия.
    pub telemetry: Telemetry,
    /// Сработала ли ESC в последнем шаге.
    pub esc_active: bool,
    /// Прошлая продольная скорость (для расчёта ускорений).
    prev_speed: f32,
}

impl Vehicle {
    /// Создаёт автомобиль из собранной конфигурации.
    pub fn new(config: VehicleConfig) -> Self {
        let mut wheels = Vec::new();
        let mut index_by_corner: [Option<usize>; 4] = [None; 4];
        for wc in &config.wheels {
            let idx = wheels.len();
            index_by_corner[corner_index(wc.corner)] = Some(idx);
            wheels.push(Wheel::new(wc.clone()));
        }

        // База и колея из позиций колёс.
        let front: Vec<&WheelConfig> = config.wheels.iter().filter(|w| w.corner.is_front()).collect();
        let rear: Vec<&WheelConfig> = config.wheels.iter().filter(|w| !w.corner.is_front()).collect();
        let wheelbase = match (front.first(), rear.first()) {
            (Some(f), Some(r)) => (r.position.z - f.position.z).abs(),
            _ => 2.6,
        };
        let track_front = if front.len() >= 2 {
            (front[0].position.x - front[1].position.x).abs()
        } else {
            1.5
        };
        let track_rear = if rear.len() >= 2 {
            (rear[0].position.x - rear[1].position.x).abs()
        } else {
            1.5
        };

        let powertrain = Powertrain::new(&config.powertrain);

        Self {
            config,
            wheels,
            powertrain,
            index_by_corner,
            wheelbase,
            track_front,
            track_rear,
            steer_angle: 0.0,
            air_density: 1.225,
            collision_mask: u32::MAX,
            substeps: 2,
            telemetry: Telemetry::default(),
            esc_active: false,
            prev_speed: 0.0,
        }
    }

    /// Полный шаг симуляции.
    ///
    /// * `position` — мировое положение кузова (начало локальных координат).
    /// * `basis` — ориентация кузова.
    /// * `linear_velocity`, `angular_velocity` — скорости кузова.
    /// * `inputs` — воздействия водителя.
    /// * `dt` — шаг времени, с.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        contacts: &dyn ContactProvider,
        position: Vec3,
        basis: Quat,
        linear_velocity: Vec3,
        angular_velocity: Vec3,
        inputs: &PowertrainInputs,
        dt: f32,
    ) -> VehicleStep {
        if dt <= 1e-6 {
            return VehicleStep::default();
        }

        // Подвеска считается один раз на кадр (положение кузова фиксировано).
        self.update_suspension(contacts, position, basis, dt);

        // Рулевое управление.
        let forward = basis * Vec3::NEG_Z;
        let speed = linear_velocity.dot(forward);
        self.update_steering(inputs, speed);

        // Колёса и трансмиссия — несколькими подшагами.
        let substeps = self.substeps.max(1);
        let h = dt / substeps as f32;
        let mut result = VehicleStep::default();
        for _ in 0..substeps {
            result = self.integrate(
                contacts,
                position,
                basis,
                linear_velocity,
                angular_velocity,
                inputs,
                h,
            );
        }

        self.update_telemetry(basis, linear_velocity, angular_velocity, dt);
        result
    }

    /// Трассировка лучей подвески, расчёт нагрузок и стабилизаторов.
    fn update_suspension(
        &mut self,
        contacts: &dyn ContactProvider,
        position: Vec3,
        basis: Quat,
        dt: f32,
    ) {
        let mut loads = [0.0f32; 4];
        let inv_dt = 1.0 / dt.max(1e-4);

        for w in &mut self.wheels {
            let i = corner_index(w.config.corner);
            let attach_local = w.config.position;
            let attach_world = position + basis * attach_local;
            let down = basis * Vec3::NEG_Y;
            let max_len = w.config.suspension.max_length;
            let min_len = w.config.suspension.min_length;
            let radius = w.config.tire.radius;
            // Луч идёт от верхней точки крепления вниз на полный ход подвески
            // плюс радиус колеса: именно так он дотягивается до земли, когда
            // подвеска полностью разжата.
            let ray_end = attach_world + down * (max_len + radius);

            match contacts.raycast(attach_world, ray_end, self.collision_mask) {
                Some(hit) => {
                    // Длина подвески = расстояние до земли минус радиус колеса.
                    let length = ((hit.point - attach_world).dot(down) - radius)
                        .clamp(min_len, max_len)
                        .max(1e-3);
                    w.suspension.velocity = (length - w.prev_length) * inv_dt;
                    w.prev_length = length;
                    w.suspension.length = length;
                    w.suspension.contact = true;
                    w.contact = true;
                    w.contact_normal = hit.normal;
                    w.contact_world = hit.point;
                    w.contact_local = basis.inverse() * (hit.point - position);
                    w.center_local = attach_local + Vec3::NEG_Y * length;
                    loads[i] = w.suspension.force(&w.config.suspension).max(0.0);
                }
                None => {
                    let length = max_len;
                    w.suspension.velocity = (length - w.prev_length) * inv_dt;
                    w.prev_length = length;
                    w.suspension.length = length;
                    w.suspension.contact = false;
                    w.contact = false;
                    w.center_local = attach_local + Vec3::NEG_Y * length;
                    loads[i] = 0.0;
                }
            }
        }

        // Стабилизаторы поперечной устойчивости.
        for &(a, b) in &[(FL, FR), (RL, RR)] {
            let (Some(ia), Some(ib)) = (self.index_by_corner[a], self.index_by_corner[b]) else {
                continue;
            };
            let ka = self.wheels[ia].config.suspension.anti_roll;
            let kb = self.wheels[ib].config.suspension.anti_roll;
            let k = (ka + kb) * 0.5;
            if k <= 0.0 {
                continue;
            }
            let ca = self.wheels[ia]
                .suspension
                .compression(&self.wheels[ia].config.suspension);
            let cb = self.wheels[ib]
                .suspension
                .compression(&self.wheels[ib].config.suspension);
            let f = k * (ca - cb);
            loads[a] += f;
            loads[b] -= f;
        }

        for w in &mut self.wheels {
            let i = corner_index(w.config.corner);
            w.load = loads[i].max(0.0);
        }
    }

    /// Рулевое управление с геометрией Аккермана и снижением на скорости.
    fn update_steering(&mut self, inputs: &PowertrainInputs, speed: f32) {
        let max_steer = self
            .wheels
            .iter()
            .filter(|w| w.config.steering)
            .map(|w| w.config.max_steer_deg.to_radians())
            .fold(0.0f32, f32::max)
            .max(0.01);

        // На высокой скорости ограничиваем угол для устойчивости.
        let speed_factor = 1.0 / (1.0 + (speed.abs() * 0.02).powi(2));
        let steer = inputs.steer.clamp(-1.0, 1.0) * max_steer * speed_factor;
        self.steer_angle = steer;

        let wheelbase = self.wheelbase.max(0.1);
        let half_track = (self.track_front.max(0.1)) * 0.5;

        for w in &mut self.wheels {
            if !w.config.steering {
                w.steer_angle = 0.0;
                continue;
            }
            if steer.abs() < 1e-4 {
                w.steer_angle = 0.0;
                continue;
            }
            // Радиус поворота со знаком.
            let turn_radius = wheelbase / steer.tan();
            let side = if w.config.corner.is_left() { -1.0 } else { 1.0 };
            let wheel_radius = turn_radius - side * half_track;
            w.steer_angle = if wheel_radius.abs() > 1e-3 {
                (wheelbase / wheel_radius).atan()
            } else {
                steer
            };
        }
    }

    /// Внутренний подшаг: силы шин, трансмиссия, интеграция колёс, силы кузова.
    #[allow(clippy::too_many_arguments)]
    fn integrate(
        &mut self,
        contacts: &dyn ContactProvider,
        position: Vec3,
        basis: Quat,
        vel: Vec3,
        ang_vel: Vec3,
        inputs: &PowertrainInputs,
        dt: f32,
    ) -> VehicleStep {
        let com_world = position + basis * self.config.center_of_mass;
        let up = basis * Vec3::Y;
        let mut total_force = Vec3::ZERO;
        let mut total_torque = Vec3::ZERO;

        // --- Силы шин (по текущей угловой скорости колёс) ---
        for w in &mut self.wheels {
            if !w.contact || w.load <= 1.0 {
                w.tire_force = Vec3::ZERO;
                w.last_long_force = 0.0;
                w.last_vx = 0.0;
                w.last_align = 0.0;
                w.tire
                    .compute(&w.config.tire, dt, 0.0, 0.0, w.omega, 0.0, 0.0, 1.0);
                continue;
            }
            let center_world = position + basis * w.center_local;
            let r = center_world - com_world;
            let v = vel + ang_vel.cross(r);
            let (forward, right) = wheel_axes(basis, w.steer_angle, w.toe(), w.config.corner);
            let vx = v.dot(forward);
            let vy = v.dot(right);
            let camber = w.camber();
            let surface = contacts.surface_friction(w.contact_world);
            let out = w
                .tire
                .compute(&w.config.tire, dt, vx, vy, w.omega, camber, w.load, surface);
            let rr = rolling_resistance(&w.config.tire, w.load, vx);
            let fx = out.force.x + rr;
            w.tire_force = forward * fx + right * out.force.y;
            w.last_long_force = fx;
            w.last_vx = vx;
            w.last_align = out.aligning_torque;
            w.surface = surface;
        }

        // --- Трансмиссия ---
        let (states, roles) = self.powertrain_arrays();
        let mut out = self
            .powertrain
            .step(&self.config.powertrain, inputs, &states, &roles, dt);

        // --- Курсовая устойчивость (ESC) ---
        self.esc_active = false;
        if self.config.powertrain.esc {
            let forward = basis * Vec3::NEG_Z;
            let speed = vel.dot(forward);
            if speed.abs() > 5.0 && self.steer_angle.abs() > 1e-3 {
                // Желаемая скорость рыскания (велосипедная модель).
                let desired = -(speed / self.wheelbase.max(1.0)) * self.steer_angle;
                let actual = ang_vel.dot(up);
                let err = desired - actual;
                let threshold = 0.12;
                if err.abs() > threshold {
                    let intensity = ((err.abs() - threshold) * 3.0).clamp(0.0, 1.0);
                    let max_brake = self.max_brake_torque();
                    let t = max_brake * intensity * 0.6;
                    if err < 0.0 {
                        out.brake_torque[RR] += t;
                    } else {
                        out.brake_torque[RL] += t;
                    }
                    self.esc_active = true;
                }
            }
        }

        // --- Применение сил и интеграция колёс ---
        for w in &mut self.wheels {
            let i = corner_index(w.config.corner);
            let center_world = position + basis * w.center_local;

            // Сила подвески (вверх).
            let sf = up * w.load;
            total_force += sf;
            total_torque += (center_world - com_world).cross(sf);

            // Сила шины в точке контакта.
            if w.contact {
                let contact_world = position + basis * w.contact_local;
                total_force += w.tire_force;
                total_torque += (contact_world - com_world).cross(w.tire_force);
                total_torque += up * w.last_align;
            }

            // Динамика вращения колеса.
            let drive = out.drive_torque[i];
            let brake = out.brake_torque[i];
            let radius = w.config.tire.radius;
            let mut net = drive - w.last_long_force * radius;
            if w.omega.abs() > 0.5 {
                net -= brake * w.omega.signum();
            } else if net.abs() <= brake {
                net = 0.0;
                w.omega = 0.0;
            } else {
                net -= brake * net.signum();
            }
            w.omega += net / w.config.inertia.max(1e-3) * dt;
            w.spin_angle += w.omega * dt;
        }

        // --- Аэродинамика ---
        let speed_sq = vel.length_squared();
        if speed_sq > 0.25 {
            let q = 0.5 * self.air_density * speed_sq;
            let dir = normalized_or(vel, basis * Vec3::NEG_Z);
            total_force += -dir * (self.config.aero.drag_area * q);
            for (pt_local, share) in &self.config.aero.downforce_points {
                let pt_world = position + basis * *pt_local;
                let df = -up * (share * q);
                total_force += df;
                total_torque += (pt_world - com_world).cross(df);
            }
        }

        // --- Результат ---
        let mut step = VehicleStep {
            force: total_force,
            torque: total_torque,
            esc_active: self.esc_active,
            ..Default::default()
        };
        for w in &self.wheels {
            let i = corner_index(w.config.corner);
            step.wheel_centers[i] = position + basis * w.center_local;
            step.wheel_spin[i] = w.spin_angle;
            step.wheel_steer[i] = w.steer_angle;
            step.wheel_contact[i] = w.contact;
            step.wheel_load[i] = w.load;
            step.wheel_slip[i] = w.tire.slip_ratio;
            if w.contact {
                step.contacts.push(ContactPoint {
                    position: position + basis * w.contact_local,
                    normal: w.contact_normal,
                    load: w.load,
                    slip_ratio: w.tire.slip_ratio,
                    slip_angle: w.tire.slip_angle,
                    surface: w.surface,
                });
            }
        }
        step
    }

    /// Массивы состояния колёс для трансмиссии.
    fn powertrain_arrays(&self) -> ([WheelState; 4], [WheelRole; 4]) {
        let mut states = [WheelState::default(); 4];
        let mut roles = [WheelRole::default(); 4];
        for w in &self.wheels {
            let i = corner_index(w.config.corner);
            states[i] = WheelState {
                omega: w.omega,
                slip: w.tire.slip_ratio,
                contact: w.contact,
            };
            roles[i] = w.config.role();
        }
        (states, roles)
    }

    /// Максимальный тормозной момент среди колёс, Н·м.
    fn max_brake_torque(&self) -> f32 {
        self.wheels
            .iter()
            .map(|w| w.config.brake_torque)
            .fold(0.0f32, f32::max)
    }

    /// Обновляет телеметрию.
    fn update_telemetry(
        &mut self,
        basis: Quat,
        vel: Vec3,
        ang_vel: Vec3,
        dt: f32,
    ) {
        let forward = basis * Vec3::NEG_Z;
        let right = basis * Vec3::X;
        let up = basis * Vec3::Y;
        let v_forward = vel.dot(forward);
        let v_lateral = vel.dot(right);
        let long_acc = if dt > 1e-5 {
            (v_forward - self.prev_speed) / dt
        } else {
            0.0
        };
        self.prev_speed = v_forward;

        let wheels_on_ground = self.wheels.iter().filter(|w| w.contact).count() as u8;
        let slip_angle = if v_forward.abs() > 0.5 {
            (v_lateral / v_forward.abs()).atan()
        } else {
            0.0
        };

        // Недостаточная поворачиваемость: сравнение угла скольжения передней
        // и задней осей.
        let mut front_slip = 0.0;
        let mut front_n = 0.0;
        let mut rear_slip = 0.0;
        let mut rear_n = 0.0;
        for w in &self.wheels {
            if w.contact {
                if w.config.corner.is_front() {
                    front_slip += w.tire.slip_angle.abs();
                    front_n += 1.0;
                } else {
                    rear_slip += w.tire.slip_angle.abs();
                    rear_n += 1.0;
                }
            }
        }
        let understeer = if front_n > 0.0 && rear_n > 0.0 {
            front_slip / front_n - rear_slip / rear_n
        } else {
            0.0
        };

        self.telemetry = Telemetry {
            speed: v_forward,
            speed_kmh: v_forward * 3.6,
            longitudinal_g: long_acc / 9.81,
            lateral_g: (v_lateral * ang_vel.dot(up)) / 9.81,
            yaw_rate: ang_vel.dot(up),
            wheels_on_ground,
            slip_angle,
            understeer,
        };
    }

    /// Сбрасывает динамическое состояние (например, при пересборке).
    pub fn reset(&mut self) {
        for w in &mut self.wheels {
            w.omega = 0.0;
            w.spin_angle = 0.0;
            w.steer_angle = 0.0;
            w.tire = TireState::default();
            w.suspension = Suspension::new(w.config.suspension.rest_length);
            w.prev_length = w.config.suspension.rest_length;
        }
        self.steer_angle = 0.0;
        self.prev_speed = 0.0;
    }

    /// Суммарная масса автомобиля, кг.
    #[inline]
    pub fn mass(&self) -> f32 {
        self.config.mass
    }
}

/// Оси колеса с учётом поворота и схождения.
#[inline]
fn wheel_axes(basis: Quat, steer: f32, toe: f32, corner: Corner) -> (Vec3, Vec3) {
    let up = basis * Vec3::Y;
    let base_forward = basis * Vec3::NEG_Z;
    let toe_sign = if corner.is_left() { -1.0 } else { 1.0 };
    let angle = steer + toe * toe_sign;
    let forward = Quat::from_axis_angle(up, angle) * base_forward;
    let right = forward.cross(up).normalize_or_zero();
    (forward.normalize_or_zero(), right)
}
