//! Узел `VehicleBody3D` — мост между Godot `RigidBody3D` и симуляцию
//! [`vehicle_core::Vehicle`].
//!
//! Узел не интегрирует положение сам: он лишь считает суммарную силу и момент
//! ядра и применяет их к `RigidBody3D` (`apply_central_force` / `apply_torque`).
//! Столкновения и общую динамику мира считает физический движок Godot.

use godot::classes::rigid_body_3d::CenterOfMassMode;
use godot::classes::{
    BoxShape3D, CollisionShape3D, IRigidBody3D, Input, InputMap, PhysicsDirectSpaceState3D,
    PhysicsRayQueryParameters3D, RigidBody3D, Shape3D,
};
use godot::obj::NewAlloc;
use godot::prelude::*;

use vehicle_core::powertrain::PowertrainInputs;
use vehicle_core::{ContactProvider, RayHit, Vehicle, VehicleConfig, VehicleStep};

use crate::assembly::CarAssemblyResource;
use crate::convert::{quat_to_glam, to_glam, to_godot};

/// Источник контактов для ядра: трассировка лучей по физическому миру Godot.
///
/// Держит клонированный указатель на `PhysicsDirectSpaceState3D` (валиден только
/// во время физического кадра), список исключаемых тел (сам кузов) и маску
/// слоёв столкновений.
pub struct WorldContacts {
    space: Option<Gd<PhysicsDirectSpaceState3D>>,
    exclude: Array<Rid>,
    mask: u32,
}

impl ContactProvider for WorldContacts {
    fn raycast(&self, from: glam::Vec3, to: glam::Vec3, mask: u32) -> Option<RayHit> {
        // Копируем указатель: `intersect_ray` требует `&mut`, а трейт даёт `&self`.
        let mut space = self.space.clone()?;

        let mut query = PhysicsRayQueryParameters3D::create(to_godot(from), to_godot(to))?;
        let effective_mask = if mask == 0 { self.mask } else { mask };
        query.set_collision_mask(effective_mask);
        query.set_collide_with_areas(false);
        query.set_collide_with_bodies(true);
        query.set_exclude(&self.exclude);

        let hit = space.intersect_ray(&query);
        if hit.is_empty() {
            return None;
        }
        let point = hit.get("position")?.try_to::<Vector3>().ok()?;
        let normal = hit.get("normal")?.try_to::<Vector3>().ok()?;
        Some(RayHit {
            point: to_glam(point),
            normal: to_glam(normal),
            friction: 1.0,
        })
    }
}

/// Автомобиль, управляемый симуляцией `vehicle-core`.
#[derive(GodotClass)]
#[class(base = RigidBody3D, rename = VbvCarBody)]
pub struct VehicleBody3D {
    /// Сборка автомобиля из деталей. Изменение требует вызова `rebuild()`.
    #[export]
    pub assembly: Option<Gd<CarAssemblyResource>>,
    /// Заводить двигатель сразу при появлении.
    #[export]
    pub auto_start_engine: bool,
    /// Читать управление из `InputMap`.
    #[export]
    pub read_input: bool,
    /// Маска слоёв для лучей подвески.
    #[export]
    pub suspension_collision_mask: i64,
    /// Число внутренних подшагов симуляции за физический кадр.
    #[export]
    pub substeps: i64,

    /// Действие «газ».
    #[export]
    pub throttle_action: GString,
    /// Действие «тормоз».
    #[export]
    pub brake_action: GString,
    /// Действие «ручной тормоз».
    #[export]
    pub handbrake_action: GString,
    /// Действие «руль влево».
    #[export]
    pub steer_left_action: GString,
    /// Действие «руль вправо».
    #[export]
    pub steer_right_action: GString,
    /// Действие «сцепление» (пусто — не используется).
    #[export]
    pub clutch_action: GString,
    /// Действие «передача вверх».
    #[export]
    pub shift_up_action: GString,
    /// Действие «передача вниз».
    #[export]
    pub shift_down_action: GString,
    /// Действие «зажигание» (пусто — всегда включено).
    #[export]
    pub ignition_action: GString,
    /// Действие «стартер».
    #[export]
    pub starter_action: GString,
    /// Зажигание включено. Переключается клавишей зажигания (тумблер).
    #[export]
    pub ignition_on: bool,

    /// Готовый автомобиль (после `rebuild`).
    vehicle: Option<Vehicle>,
    /// Коллайдер кузова (создаётся плагином автоматически).
    collider: Option<Gd<CollisionShape3D>>,
    /// Текущие воздействия.
    inputs: PowertrainInputs,
    /// Сглаженный угол руля, -1..1.
    steer_value: f32,
    /// Скорость сглаживания руля, 1/с.
    steer_speed: f32,
    /// Результат последнего шага (для визуализации).
    last_step: VehicleStep,
    /// Передача на прошлом кадре (для сигнала).
    last_gear: i64,
    /// Работал ли двигатель на прошлом кадре (для сигнала).
    last_running: bool,

    base: Base<RigidBody3D>,
}

#[godot_api]
impl VehicleBody3D {
    /// Двигатель запущен/заглушен.
    #[signal]
    fn engine_state_changed(running: bool);

    /// Передача изменилась.
    #[signal]
    fn gear_changed(gear: i64);

    /// Пересобирает автомобиль из `assembly` и настраивает физическое тело.
    #[func]
    pub fn rebuild(&mut self) {
        let Some(assembly) = self.assembly.clone() else {
            self.vehicle = None;
            return;
        };

        let core_assembly = assembly.bind().to_core();
        let config = match core_assembly.build() {
            Ok(config) => config,
            Err(issues) => {
                for issue in issues {
                    godot_warn!("[vehicle] {}", issue.message);
                }
                self.vehicle = None;
                return;
            }
        };

        // Коллайдер кузова должен находиться НАД колёсами: иначе он упирается
        // в землю, подвеска не сжимается, колёса не нагружены и машина не едет.
        self.ensure_collider(&config);

        let mut vehicle = Vehicle::new(config);
        vehicle.substeps = self.substeps.clamp(1, 32) as u32;
        vehicle.collision_mask = self.suspension_collision_mask.max(0) as u32;

        if self.auto_start_engine {
            let cfg = vehicle.config.powertrain.clone();
            vehicle.powertrain.force_start(&cfg);
        }

        // Настройка тела: масса, центр масс, инерция.
        let mass = vehicle.config.mass.max(1.0);
        let com = to_godot(vehicle.config.center_of_mass);
        let inertia = to_godot(vehicle.config.inertia.max(glam::Vec3::splat(1.0)));
        {
            let mut base = self.base_mut();
            base.set_mass(mass);
            base.set_center_of_mass_mode(CenterOfMassMode::CUSTOM);
            base.set_center_of_mass(com);
            base.set_inertia(inertia);
            // Демпфирование считает ядро (аэродинамика, шины), не движок.
            base.set_linear_damp(0.0);
            base.set_angular_damp(0.0);
        }

        self.last_gear = vehicle.powertrain.output.gear as i64;
        self.last_running = vehicle.powertrain.output.running;
        self.vehicle = Some(vehicle);
    }

    /// Создаёт (или обновляет) коллайдер кузова над колёсами.
    fn ensure_collider(&mut self, config: &VehicleConfig) {
        let Some(first) = config.wheels.first() else {
            return;
        };

        // Высота точки крепления подвески над землёй (локальная Y).
        let attach_y = first.position.y;
        let half_track = config
            .wheels
            .iter()
            .map(|w| w.position.x.abs())
            .fold(0.0f32, f32::max);
        let front_z = config
            .wheels
            .iter()
            .map(|w| w.position.z)
            .fold(f32::MIN, f32::max);
        let rear_z = config
            .wheels
            .iter()
            .map(|w| w.position.z)
            .fold(f32::MAX, f32::min);
        let wheelbase = (front_z - rear_z).abs();

        let width = (half_track * 2.0 * 0.80).max(1.1);
        let length = (wheelbase + 1.0).min(6.0);
        let height = 0.45;
        // Нижняя кромка кузова: чуть ниже точки крепления подвески,
        // но заведомо выше земли — подвеска должна сжиматься свободно.
        let floor_y = (attach_y - 0.18).max(0.2);

        let mut shape = BoxShape3D::new_gd();
        shape.set_size(Vector3::new(width, height, length));

        let mut collider: Gd<CollisionShape3D> = CollisionShape3D::new_alloc();
        collider.set_shape(&shape.upcast::<Shape3D>());
        collider.set_position(Vector3::new(0.0, floor_y + height * 0.5, 0.0));

        if let Some(mut old) = self.collider.take() {
            if old.is_instance_valid() {
                old.queue_free();
            }
        }

        let mut owner: Gd<Node> = self.to_gd().upcast();
        owner.add_child(&collider.clone().upcast::<Node>());
        self.collider = Some(collider);
    }

    /// Есть ли готовая симуляция.
    #[func]
    pub fn is_ready(&self) -> bool {
        self.vehicle.is_some()
    }

    /// Включает/выключает чтение управления из `InputMap`.
    #[func]
    pub fn set_input_enabled(&mut self, enabled: bool) {
        self.read_input = enabled;
    }

    /// Прямая установка управления (например, из ИИ или записи).
    #[func]
    pub fn set_controls(&mut self, throttle: f32, brake: f32, steer: f32, handbrake: f32) {
        self.read_input = false;
        self.inputs.throttle = throttle.clamp(0.0, 1.0);
        self.inputs.brake = brake.clamp(0.0, 1.0);
        self.inputs.handbrake = handbrake.clamp(0.0, 1.0);
        self.inputs.steer = steer.clamp(-1.0, 1.0);
        self.steer_value = self.inputs.steer;
    }

    /// Только газ, 0..1.
    #[func]
    pub fn set_throttle(&mut self, throttle: f32) {
        self.read_input = false;
        self.inputs.throttle = throttle.clamp(0.0, 1.0);
    }

    /// Только тормоз, 0..1.
    #[func]
    pub fn set_brake(&mut self, brake: f32) {
        self.read_input = false;
        self.inputs.brake = brake.clamp(0.0, 1.0);
    }

    /// Только руль, -1..1.
    #[func]
    pub fn set_steer(&mut self, steer: f32) {
        self.read_input = false;
        self.inputs.steer = steer.clamp(-1.0, 1.0);
        self.steer_value = self.inputs.steer;
    }

    /// Сцепление, 0 (выжато) .. 1 (отпущено).
    #[func]
    pub fn set_clutch(&mut self, clutch: f32) {
        self.inputs.clutch = clutch.clamp(0.0, 1.0);
    }

    /// Переключение на передачу вверх.
    #[func]
    pub fn shift_up(&mut self) {
        self.inputs.shift_up = true;
    }

    /// Переключение на передачу вниз.
    #[func]
    pub fn shift_down(&mut self) {
        self.inputs.shift_down = true;
    }

    /// Запускает двигатель.
    #[func]
    pub fn start_engine(&mut self) {
        self.ignition_on = true;
        if let Some(vehicle) = self.vehicle.as_mut() {
            let cfg = vehicle.config.powertrain.clone();
            vehicle.powertrain.force_start(&cfg);
        }
    }

    /// Глушит двигатель.
    #[func]
    pub fn stop_engine(&mut self) {
        self.ignition_on = false;
        if let Some(vehicle) = self.vehicle.as_mut() {
            vehicle.powertrain.stop_engine();
        }
    }

    /// Сбрасывает динамическое состояние (скорости колёс, передачи).
    #[func]
    pub fn reset_state(&mut self) {
        if let Some(vehicle) = self.vehicle.as_mut() {
            vehicle.reset();
        }
        let mut base = self.base_mut();
        base.set_linear_velocity(Vector3::ZERO);
        base.set_angular_velocity(Vector3::ZERO);
    }

    /// Продольная скорость, м/с.
    #[func]
    pub fn get_speed(&self) -> f32 {
        self.vehicle
            .as_ref()
            .map(|v| v.telemetry.speed)
            .unwrap_or(0.0)
    }

    /// Продольная скорость, км/ч.
    #[func]
    pub fn get_speed_kmh(&self) -> f32 {
        self.vehicle
            .as_ref()
            .map(|v| v.telemetry.speed_kmh)
            .unwrap_or(0.0)
    }

    /// Обороты двигателя.
    #[func]
    pub fn get_rpm(&self) -> f32 {
        self.vehicle
            .as_ref()
            .map(|v| v.powertrain.output.rpm)
            .unwrap_or(0.0)
    }

    /// Текущая передача (0 — нейтраль, -1 — задний ход).
    #[func]
    pub fn get_gear(&self) -> i64 {
        self.vehicle
            .as_ref()
            .map(|v| v.powertrain.output.gear as i64)
            .unwrap_or(0)
    }

    /// Работает ли двигатель.
    #[func]
    pub fn is_engine_running(&self) -> bool {
        self.vehicle
            .as_ref()
            .map(|v| v.powertrain.output.running)
            .unwrap_or(false)
    }

    /// Число колёс в сборке.
    #[func]
    pub fn get_wheel_count(&self) -> i64 {
        self.vehicle
            .as_ref()
            .map(|v| v.wheels.len() as i64)
            .unwrap_or(0)
    }

    /// Телеметрия автомобиля словарём.
    #[func]
    pub fn get_telemetry(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        if let Some(v) = self.vehicle.as_ref() {
            let t = v.telemetry;
            d.set("speed", t.speed);
            d.set("speed_kmh", t.speed_kmh);
            d.set("longitudinal_g", t.longitudinal_g);
            d.set("lateral_g", t.lateral_g);
            d.set("yaw_rate", t.yaw_rate);
            d.set("wheels_on_ground", t.wheels_on_ground as i64);
            d.set("slip_angle", t.slip_angle);
            d.set("understeer", t.understeer);
            d.set("esc_active", v.esc_active);
        }
        d
    }

    /// Состояние двигателя и трансмиссии словарём.
    #[func]
    pub fn get_engine_state(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        if let Some(v) = self.vehicle.as_ref() {
            let o = v.powertrain.output;
            d.set("rpm", o.rpm);
            d.set("gear", o.gear as i64);
            d.set("shifting", o.shifting);
            d.set("running", o.running);
            d.set("engine_torque", o.engine_torque);
            d.set("power_kw", o.power_kw);
            d.set("boost", o.boost);
            d.set("manifold_pressure", o.manifold_pressure);
            d.set("afr", o.afr);
            d.set("coolant_temp", o.coolant_temp);
            d.set("oil_temp", o.oil_temp);
            d.set("fuel_l", o.fuel_l);
            d.set("clutch_engagement", o.clutch_engagement);
            d.set("clutch_temperature", o.clutch_temperature);
            d.set("converter_locked", o.converter_locked);
            d.set("tcs_active", o.tcs_active);
            d.set("fuel_cut", o.fuel_cut);
            d.set("knock", o.knock);
            d.set("damage", o.damage);
            d.set("gearbox_damage", o.gearbox_damage);
        }
        d
    }

    /// Информация по каждому колесу (массив словарей в порядке `[FL, FR, RL, RR]`).
    #[func]
    pub fn get_wheel_info(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        for i in 0..4 {
            let mut d = VarDictionary::new();
            d.set("contact", self.last_step.wheel_contact[i]);
            d.set("load", self.last_step.wheel_load[i]);
            d.set("slip", self.last_step.wheel_slip[i]);
            d.set("steer", self.last_step.wheel_steer[i]);
            d.set("spin", self.last_step.wheel_spin[i]);
            let c = self.last_step.wheel_centers[i];
            d.set("center", to_godot(c));
            out.push(&d);
        }
        out
    }

    /// Мировые трансформации колёс (для визуализации) в порядке `[FL, FR, RL, RR]`.
    #[func]
    pub fn get_wheel_transforms(&self) -> Array<Transform3D> {
        let body_basis = self.base().get_global_basis();
        let mut out = Array::new();
        for i in 0..4 {
            let steer = Basis::from_axis_angle(Vector3::UP, self.last_step.wheel_steer[i]);
            let spin = Basis::from_axis_angle(Vector3::RIGHT, -self.last_step.wheel_spin[i]);
            let basis = body_basis * steer * spin;
            let origin = to_godot(self.last_step.wheel_centers[i]);
            let transform = Transform3D::new(basis, origin);
            out.push(transform);
        }
        out
    }

    /// Габариты и параметры для визуализации (кузов, колёса).
    ///
    /// Возвращает словарь с ключами: `wheelbase`, `track_front`, `track_rear`,
    /// `wheel_radius`, `wheel_width`, `body_length`, `body_width`, `body_height`,
    /// `mass`, `cylinders`, `redline_rpm`, `wheel_count`, `wheel_positions`
    /// (массив локальных позиций колёс в порядке `[FL, FR, RL, RR]`).
    #[func]
    pub fn get_visual_spec(&self) -> VarDictionary {
        let mut d = VarDictionary::new();
        let Some(vehicle) = self.vehicle.as_ref() else {
            return d;
        };

        let cfg = &vehicle.config;
        let wheelbase = vehicle.wheelbase;
        let track_front = vehicle.track_front;
        let track_rear = vehicle.track_rear;

        let (radius, width) = cfg
            .wheels
            .first()
            .map(|w| (w.tire.radius, w.tire.width))
            .unwrap_or((0.33, 0.235));

        let track = track_front.max(track_rear);
        // Габарит кузова: колёсная база + свесы, ширина = колея + свесы,
        // высота кузова от земли.
        let body_length = wheelbase * 1.12 + 0.55;
        let body_width = track + 0.22;
        let body_height = 1.34;
        // Высота точки крепления подвески над землёй.
        let attach_y = cfg
            .wheels
            .first()
            .map(|w| w.position.y)
            .unwrap_or(0.65);
        // Нижняя кромка кузова (кузов стоит над колёсами).
        let body_floor_y = (attach_y - 0.18).max(0.2);

        let mut positions: Array<Vector3> = Array::new();
        for w in &cfg.wheels {
            positions.push(to_godot(w.position));
        }

        d.set("wheelbase", wheelbase);
        d.set("track_front", track_front);
        d.set("track_rear", track_rear);
        d.set("wheel_radius", radius);
        d.set("wheel_width", width);
        d.set("body_length", body_length);
        d.set("body_width", body_width);
        d.set("body_height", body_height);
        d.set("body_floor_y", body_floor_y);
        d.set("attach_y", attach_y);
        d.set("mass", cfg.mass);
        d.set("cylinders", cfg.powertrain.engine.cylinders as i64);
        d.set("redline_rpm", cfg.powertrain.engine.redline_rpm);
        d.set("wheel_count", cfg.wheels.len() as i64);
        let positions_variant = positions.to_variant();
        d.set("wheel_positions", &positions_variant);
        d
    }

    /// Отладочные данные подвески (для диагностики и тестов).
    #[func]
    pub fn get_suspension_debug(&self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(vehicle) = self.vehicle.as_ref() else {
            return out;
        };
        for w in &vehicle.wheels {
            let mut d = VarDictionary::new();
            d.set("attach_local", to_godot(w.config.position));
            d.set("center_local", to_godot(w.center_local));
            d.set("contact_local", to_godot(w.contact_local));
            d.set("length", w.suspension.length);
            d.set("compression", w.suspension.compression(&w.config.suspension));
            d.set("rest", w.config.suspension.rest_length);
            d.set("spring_rate", w.config.suspension.spring_rate);
            d.set("damper_c", w.config.suspension.damper_compression);
            d.set("damper_r", w.config.suspension.damper_rebound);
            d.set("anti_roll", w.config.suspension.anti_roll);
            d.set("velocity", w.suspension.velocity);
            d.set("radius", w.config.tire.radius);
            d.set("load", w.load);
            d.set("contact", w.contact);
            out.push(&d);
        }
        out
    }

    /// Список проблем сборки (пусто — всё совместимо).
    #[func]
    pub fn get_validation(&self) -> Array<GString> {
        match self.assembly.as_ref() {
            Some(assembly) => assembly.bind().validate(),
            None => {
                let mut out = Array::new();
                let msg = GString::from("[ОШИБКА] Не выбрана сборка");
                out.push(&msg);
                out
            }
        }
    }
}

impl VehicleBody3D {
    /// Считывает управление из `InputMap` с проверкой существования действий.
    fn read_actions(&mut self, dt: f32) {
        let input = Input::singleton();
        let map = InputMap::singleton();

        let strength = |name: &GString| -> f32 {
            if name.len() == 0 {
                return 0.0;
            }
            let action = StringName::from(name);
            if map.has_action(&action) {
                input.get_action_strength(&action)
            } else {
                0.0
            }
        };
        let pressed = |name: &GString| -> bool {
            if name.len() == 0 {
                return false;
            }
            let action = StringName::from(name);
            map.has_action(&action) && input.is_action_pressed(&action)
        };
        let just_pressed = |name: &GString| -> bool {
            if name.len() == 0 {
                return false;
            }
            let action = StringName::from(name);
            map.has_action(&action) && input.is_action_just_pressed(&action)
        };

        self.inputs.throttle = strength(&self.throttle_action).clamp(0.0, 1.0);
        self.inputs.brake = strength(&self.brake_action).clamp(0.0, 1.0);
        self.inputs.handbrake = strength(&self.handbrake_action).clamp(0.0, 1.0);

        let left = strength(&self.steer_left_action);
        let right = strength(&self.steer_right_action);
        let target = (right - left).clamp(-1.0, 1.0);
        let blend = (self.steer_speed * dt).clamp(0.0, 1.0);
        self.steer_value += (target - self.steer_value) * blend;
        self.inputs.steer = self.steer_value;

        if self.clutch_action.len() != 0 {
            self.inputs.clutch = (1.0 - strength(&self.clutch_action)).clamp(0.0, 1.0);
        }
        if self.ignition_action.len() != 0 {
            // Зажигание — тумблер, а не кнопка: иначе оно выключалось бы
            // сразу же, как только игрок отпустил клавишу, и мотор глох.
            if just_pressed(&self.ignition_action) {
                self.ignition_on = !self.ignition_on;
            }
            self.inputs.ignition = self.ignition_on;
        }
        self.inputs.starter = pressed(&self.starter_action);

        self.inputs.shift_up |= just_pressed(&self.shift_up_action);
        self.inputs.shift_down |= just_pressed(&self.shift_down_action);
    }

    /// Создаёт источник контактов для текущего физического мира.
    fn make_contacts(&self) -> WorldContacts {
        let space = self
            .base()
            .get_world_3d()
            .and_then(|world| world.get_direct_space_state());

        let mut exclude = Array::new();
        exclude.push(self.base().get_rid());

        WorldContacts {
            space,
            exclude,
            mask: self.suspension_collision_mask.max(0) as u32,
        }
    }
}

#[godot_api]
impl IRigidBody3D for VehicleBody3D {
    fn init(base: Base<RigidBody3D>) -> Self {
        Self {
            assembly: None,
            auto_start_engine: true,
            read_input: true,
            suspension_collision_mask: 1,
            substeps: 8,
            throttle_action: GString::from("vehicle_throttle"),
            brake_action: GString::from("vehicle_brake"),
            handbrake_action: GString::from("vehicle_handbrake"),
            steer_left_action: GString::from("vehicle_steer_left"),
            steer_right_action: GString::from("vehicle_steer_right"),
            clutch_action: GString::from("vehicle_clutch"),
            shift_up_action: GString::from("vehicle_shift_up"),
            shift_down_action: GString::from("vehicle_shift_down"),
            ignition_action: GString::from("vehicle_ignition"),
            starter_action: GString::from("vehicle_starter"),
            ignition_on: true,
            vehicle: None,
            collider: None,
            inputs: PowertrainInputs::default(),
            steer_value: 0.0,
            steer_speed: 6.0,
            last_step: VehicleStep::default(),
            last_gear: 0,
            last_running: false,
            base,
        }
    }

    fn ready(&mut self) {
        if self.assembly.is_some() {
            self.rebuild();
        }
    }

    fn physics_process(&mut self, delta: f64) {
        if self.vehicle.is_none() {
            return;
        }
        let dt = delta as f32;
        if dt <= 1e-5 {
            return;
        }

        if self.read_input {
            self.read_actions(dt);
        }

        // 1. Состояние тела.
        let (position, basis, lin_vel, ang_vel) = {
            let base = self.base();
            (
                to_glam(base.get_global_position()),
                quat_to_glam(base.get_global_basis().get_quaternion()),
                to_glam(base.get_linear_velocity()),
                to_glam(base.get_angular_velocity()),
            )
        };

        // 2. Контакты мира.
        let contacts = self.make_contacts();

        // 3. Шаг симуляции.
        let inputs = self.inputs;
        let step = {
            let vehicle = self.vehicle.as_mut().unwrap();
            vehicle.step(&contacts, position, basis, lin_vel, ang_vel, &inputs, dt)
        };

        // 4. Применение сил.
        {
            let mut base = self.base_mut();
            base.apply_central_force(to_godot(step.force));
            base.apply_torque(to_godot(step.torque));
        }

        // 5. Сигналы об изменении состояния.
        let (gear, running) = {
            let out = &self.vehicle.as_ref().unwrap().powertrain.output;
            (out.gear as i64, out.running)
        };
        if gear != self.last_gear {
            self.last_gear = gear;
            self.signals().gear_changed().emit(gear);
        }
        if running != self.last_running {
            self.last_running = running;
            self.signals().engine_state_changed().emit(running);
        }

        // 6. Сброс импульсных воздействий.
        self.inputs.shift_up = false;
        self.inputs.shift_down = false;
        self.last_step = step;
    }
}
