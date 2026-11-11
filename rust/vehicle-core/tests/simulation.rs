//! Интеграционные тесты ядра: двигатель, коробка, сборка, автомобиль,
//! шины и мягкое тело.

use glam::{Quat, Vec3};

use vehicle_core::assembly::CarAssembly;
use vehicle_core::catalog;
use vehicle_core::powertrain::{
    omega_from_rpm, Engine, EngineConfig, Powertrain, PowertrainInputs, Transmission,
    TransmissionConfig, WheelRole, WheelState,
};
use vehicle_core::softbody::SoftBody;
use vehicle_core::tire::{TireConfig, TireState};
use vehicle_core::vehicle::corner_index;
use vehicle_core::{PlaneContact, Vehicle};

const DT: f32 = 1.0 / 60.0;

// ---------------------------------------------------------------------------
// Двигатель
// ---------------------------------------------------------------------------

#[test]
fn engine_settles_at_idle() {
    let cfg = EngineConfig::default();
    let mut engine = Engine::new(&cfg);
    engine.running = true;
    engine.omega = omega_from_rpm(cfg.idle_rpm);
    for _ in 0..(60 * 5) {
        engine.step(&cfg, 0.0, 0.0, DT);
    }
    let rpm = engine.output.rpm;
    assert!(engine.running, "двигатель заглох");
    assert!(
        (rpm - cfg.idle_rpm).abs() < 250.0,
        "обороты холостого хода {rpm:.0} далеки от {:.0}",
        cfg.idle_rpm
    );
}

#[test]
fn engine_revs_with_throttle() {
    let cfg = EngineConfig::default();
    let mut engine = Engine::new(&cfg);
    engine.running = true;
    engine.omega = omega_from_rpm(cfg.idle_rpm);
    let mut peak_power = 0.0f32;
    for _ in 0..(60 * 3) {
        engine.step(&cfg, 1.0, 0.0, DT);
        peak_power = peak_power.max(engine.output.power_kw);
    }
    assert!(
        engine.output.rpm > 3000.0,
        "двигатель не раскрутился: {:.0} об/мин",
        engine.output.rpm
    );
    assert!(peak_power > 20.0, "нулевая мощность: {peak_power:.1} кВт");
}

#[test]
fn engine_consumes_fuel() {
    let cfg = EngineConfig::default();
    let mut engine = Engine::new(&cfg);
    engine.running = true;
    engine.omega = omega_from_rpm(cfg.idle_rpm * 2.0);
    let start = engine.fuel_l;
    for _ in 0..(60 * 5) {
        engine.step(&cfg, 0.5, 0.0, DT);
    }
    assert!(engine.fuel_l < start, "топливо не расходуется");
}

// ---------------------------------------------------------------------------
// Коробка передач
// ---------------------------------------------------------------------------

#[test]
fn manual_gearbox_shifts_up() {
    let cfg = TransmissionConfig::default();
    let mut t = Transmission::new();
    assert_eq!(t.gear, 1);
    t.update(&cfg, true, false, 0.5, 3000.0, 20.0, DT);
    assert_eq!(t.gear, 2, "нет переключения вверх");
    assert!(t.shifting(), "не выставлен таймер переключения");
    // Во время переключения повторное не срабатывает.
    t.update(&cfg, true, false, 0.5, 3000.0, 20.0, DT);
    assert_eq!(t.gear, 2);
}

#[test]
fn manual_gearbox_blocks_reverse_while_moving() {
    let cfg = TransmissionConfig::default();
    let mut t = Transmission::new();
    t.gear = 0;
    // На скорости включать задний ход нельзя.
    t.update(&cfg, false, true, 0.0, 1000.0, 10.0, DT);
    assert_eq!(t.gear, 0, "включён задний ход на скорости");
    // На месте — можно.
    t.shift_timer = 0.0;
    t.update(&cfg, false, true, 0.0, 1000.0, 0.0, DT);
    assert_eq!(t.gear, -1, "не включается задний ход на месте");
}

#[test]
fn automatic_gearbox_upshifts() {
    let mut cfg = TransmissionConfig::default();
    cfg.automatic = true;
    cfg.upshift_rpm = 6000.0;
    let mut t = Transmission::new();
    t.update(&cfg, false, false, 0.5, 6600.0, 30.0, DT);
    assert_eq!(t.gear, 2, "автомат не переключился вверх");
}

// ---------------------------------------------------------------------------
// Сборка
// ---------------------------------------------------------------------------

#[test]
fn sample_assemblies_build() {
    for car in [catalog::sample_rwd_coupe(), catalog::sample_awd_auto()] {
        let issues = car.validate();
        assert!(
            !CarAssembly::has_errors(&issues),
            "{}: ошибки сборки: {issues:?}",
            car.name
        );
        let config = car.build().unwrap_or_else(|e| panic!("{}: {e:?}", car.name));
        assert_eq!(config.wheels.len(), 4);
        assert!(config.mass > 800.0, "подозрительно малая масса");
        assert!(config.center_of_mass.y > 0.0);
    }
}

#[test]
fn empty_assembly_is_invalid() {
    let car = CarAssembly::default();
    let issues = car.validate();
    assert!(CarAssembly::has_errors(&issues));
    assert!(car.build().is_err());
}

#[test]
fn mismatched_bolt_pattern_is_rejected() {
    let mut car = catalog::sample_rwd_coupe();
    // Ставим диск с другой разболтовкой.
    for wheel in &mut car.wheels {
        if let Some(rim) = &mut wheel.rim {
            rim.bolt_pattern.bolts = 4;
            rim.bolt_pattern.pcd_mm = 100;
        }
    }
    let issues = car.validate();
    assert!(CarAssembly::has_errors(&issues));
}

// ---------------------------------------------------------------------------
// Трансмиссия: полный привод
// ---------------------------------------------------------------------------

#[test]
fn awd_drives_all_four_wheels() {
    let car = catalog::sample_awd_auto();
    let config = car.build().unwrap();
    let mut powertrain = Powertrain::new(&config.powertrain);
    powertrain.force_start(&config.powertrain);

    let mut roles = [WheelRole::default(); 4];
    for wc in &config.wheels {
        roles[corner_index(wc.corner)] = wc.role();
    }
    let wheels = [WheelState {
        omega: 0.0,
        slip: 0.0,
        contact: true,
    }; 4];

    let mut inputs = PowertrainInputs::default();
    inputs.throttle = 1.0;
    let mut out = powertrain.output;
    for _ in 0..(60 * 2) {
        out = powertrain.step(&config.powertrain, &inputs, &wheels, &roles, DT);
    }

    let driven = out
        .drive_torque
        .iter()
        .filter(|t| t.abs() > 1.0)
        .count();
    assert_eq!(driven, 4, "при полном приводе момент не дошёл до всех колёс: {:?}", out.drive_torque);
}

// ---------------------------------------------------------------------------
// Шины
// ---------------------------------------------------------------------------

#[test]
fn tire_produces_lateral_force() {
    let cfg = TireConfig::default();
    let mut state = TireState::default();
    let v = 20.0f32;
    let alpha = 5.0f32.to_radians();
    let vy = v * alpha.tan();
    let out = state.compute(&cfg, DT, v, vy, v / cfg.radius, 0.0, 4000.0, 1.0);
    assert!(
        out.force.y.abs() > 1000.0,
        "боковая сила слишком мала: {:.0} Н",
        out.force.y
    );
    assert!(
        out.force.y.abs() > out.force.x.abs(),
        "боковая сила должна доминировать над продольной"
    );
}

#[test]
fn tire_grip_falls_with_load() {
    let cfg = TireConfig::default();
    let mut a = TireState::default();
    let mut b = TireState::default();
    let alpha = 5.0f32.to_radians();
    let va = a.compute(&cfg, DT, 20.0, 20.0 * alpha.tan(), 20.0 / cfg.radius, 0.0, 2000.0, 1.0);
    let vb = b.compute(&cfg, DT, 20.0, 20.0 * alpha.tan(), 20.0 / cfg.radius, 0.0, 8000.0, 1.0);
    // Сила растёт, но не в 4 раза (падение коэффициента сцепления с нагрузкой).
    assert!(vb.force.y.abs() > va.force.y.abs());
    assert!(vb.force.y.abs() < 4.0 * va.force.y.abs());
}

// ---------------------------------------------------------------------------
// Мягкое тело
// ---------------------------------------------------------------------------

#[test]
fn softbody_deforms_and_repairs() {
    let mut body = SoftBody::box_lattice(Vec3::new(1.8, 1.2, 4.0), [3, 2, 4], 400.0, 15_000.0);
    assert!(body.node_count() > 0);
    // Закрепляем один угол, бьём по противоположному.
    body.pin_nearest(Vec3::new(-0.9, -0.6, -2.0));
    let target = body.nearest_node(Vec3::new(0.9, 0.6, 2.0)).unwrap();
    body.apply_impulse(target, Vec3::new(0.0, 0.0, -120.0));
    for _ in 0..(120 * 2) {
        body.step(1.0 / 120.0, Vec3::new(0.0, -9.81, 0.0));
    }
    assert!(
        body.max_deformation() > 0.005,
        "тело не деформировалось: {:.4}",
        body.max_deformation()
    );
    body.repair();
    assert!(body.max_deformation() < 1e-3, "ремонт не восстановил форму");
    assert_eq!(body.broken_beams(), 0);
}

// ---------------------------------------------------------------------------
// Автомобиль
// ---------------------------------------------------------------------------

/// Простейший интегратор кузова: только поступательное движение, без крена.
struct Harness {
    vehicle: Vehicle,
    position: Vec3,
    velocity: Vec3,
    ground: PlaneContact,
}

impl Harness {
    fn new(car: CarAssembly) -> Self {
        let config = car.build().expect("сборка не прошла");
        Self {
            vehicle: Vehicle::new(config),
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            ground: PlaneContact::new(0.0),
        }
    }

    fn step(&mut self, inputs: &PowertrainInputs, dt: f32) {
        let basis = Quat::IDENTITY;
        let out = self.vehicle.step(
            &self.ground,
            self.position,
            basis,
            self.velocity,
            Vec3::ZERO,
            inputs,
            dt,
        );
        let mass = self.vehicle.mass().max(1.0);
        let accel = out.force / mass + Vec3::new(0.0, -9.81, 0.0);
        self.velocity += accel * dt;
        self.position += self.velocity * dt;
    }

    fn forward_speed(&self) -> f32 {
        self.velocity.dot(Vec3::NEG_Z)
    }

    fn start(&mut self) {
        let cfg = self.vehicle.config.powertrain.clone();
        self.vehicle.powertrain.force_start(&cfg);
    }
}

#[test]
fn car_settles_on_ground() {
    let mut h = Harness::new(catalog::sample_rwd_coupe());
    let idle = PowertrainInputs::default();
    for _ in 0..(60 * 3) {
        h.step(&idle, DT);
    }
    // Машина должна осесть и держаться у земли, а не провалиться.
    assert!(
        h.position.y > -0.3 && h.position.y < 0.1,
        "машина не осела: y = {:.3}",
        h.position.y
    );
    let on_ground = h.vehicle.wheels.iter().filter(|w| w.contact).count();
    assert_eq!(on_ground, 4, "не все колёса на земле");
}

#[test]
fn car_accelerates_under_throttle() {
    let mut h = Harness::new(catalog::sample_rwd_coupe());
    h.start();
    let mut inputs = PowertrainInputs::default();
    inputs.throttle = 1.0;
    inputs.clutch = 0.0;
    // Плавный отпуск сцепления.
    for i in 0..(60 * 6) {
        let t = (i as f32 / (60.0 * 0.8)).min(1.0);
        inputs.clutch = t;
        h.step(&inputs, DT);
    }
    assert!(
        h.forward_speed() > 5.0,
        "машина не разогналась: {:.2} м/с",
        h.forward_speed()
    );
}

#[test]
fn car_brakes_to_stop() {
    let mut h = Harness::new(catalog::sample_rwd_coupe());
    h.start();
    let mut inputs = PowertrainInputs::default();
    inputs.throttle = 1.0;
    inputs.clutch = 1.0;
    for _ in 0..(60 * 5) {
        h.step(&inputs, DT);
    }
    let v0 = h.forward_speed();
    assert!(v0 > 5.0, "не удалось разогнаться для теста тормозов");

    let mut braking = PowertrainInputs::default();
    braking.brake = 1.0;
    braking.clutch = 0.0;
    for _ in 0..(60 * 3) {
        h.step(&braking, DT);
    }
    assert!(
        h.forward_speed() < v0 * 0.5,
        "тормоза слабые: {:.2} -> {:.2} м/с",
        v0,
        h.forward_speed()
    );
}
