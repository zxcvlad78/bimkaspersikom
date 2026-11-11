//! Ресурс сборки автомобиля: связывает детали в целостную машину и выводит
//! [`vehicle_core::assembly::VehicleConfig`] для симуляции.

use godot::prelude::*;

use vehicle_core::assembly::{CarAssembly, WheelAssembly};
use vehicle_core::catalog;
use vehicle_core::parts::Corner;

use crate::parts::{
    aero_from_spec, body_from_spec, chassis_from_spec, differential_from_spec, engine_from_spec,
    gearbox_from_spec, wheel_from_specs, AeroResource, BodyResource, BrakeResource, ChassisResource,
    DifferentialResource, EngineResource, GearboxResource, RimResource, SuspensionResource,
    TireResource, WheelResource,
};

/// Полная сборка машины из деталей.
///
/// Колесо можно задать для каждого угла отдельно (`wheel_fl` и т.д.); если
/// конкретное колесо не задано, берётся общее `front_wheels`/`rear_wheels`.
#[derive(GodotClass)]
#[class(base=Resource, tool)]
pub struct CarAssemblyResource {
    #[export]
    #[var(rename = assembly_name)]
    pub name: GString,

    #[export]
    pub chassis: Option<Gd<ChassisResource>>,
    #[export]
    pub engine: Option<Gd<EngineResource>>,
    #[export]
    pub gearbox: Option<Gd<GearboxResource>>,
    /// Межколёсный дифференциал ведущей оси.
    #[export]
    pub differential: Option<Gd<DifferentialResource>>,
    /// Центральный дифференциал (для полного привода).
    #[export]
    pub center_differential: Option<Gd<DifferentialResource>>,
    #[export]
    pub body: Option<Gd<BodyResource>>,
    #[export]
    pub aero: Array<Gd<AeroResource>>,

    #[export]
    pub front_wheels: Option<Gd<WheelResource>>,
    #[export]
    pub rear_wheels: Option<Gd<WheelResource>>,
    #[export]
    pub wheel_fl: Option<Gd<WheelResource>>,
    #[export]
    pub wheel_fr: Option<Gd<WheelResource>>,
    #[export]
    pub wheel_rl: Option<Gd<WheelResource>>,
    #[export]
    pub wheel_rr: Option<Gd<WheelResource>>,

    base: Base<Resource>,
}

impl CarAssemblyResource {
    /// Строит сборку ядра из ресурсов.
    pub fn to_core(&self) -> CarAssembly {
        let mut car = CarAssembly {
            name: String::from(&self.name),
            ..Default::default()
        };
        car.chassis = self.chassis.as_ref().map(|c| c.bind().build_spec());
        car.engine = self.engine.as_ref().map(|e| e.bind().build_spec());
        car.gearbox = self.gearbox.as_ref().map(|g| g.bind().build_spec());
        car.differential = self.differential.as_ref().map(|d| d.bind().build_spec());
        car.center_differential = self
            .center_differential
            .as_ref()
            .map(|d| d.bind().build_spec());
        car.body = self.body.as_ref().map(|b| b.bind().build_spec());
        car.aero = self
            .aero
            .iter_shared()
            .map(|a| a.bind().build_spec())
            .collect();

        for corner in Corner::all() {
            let Some(wheel) = self.wheel_for(corner) else {
                continue;
            };
            let w = wheel.bind();
            let (Some(susp), Some(rim), Some(tire), Some(brake)) =
                (&w.suspension, &w.rim, &w.tire, &w.brake)
            else {
                continue;
            };
            car.wheels.push(WheelAssembly::new(corner).with(
                susp.bind().build_spec(),
                rim.bind().build_spec(),
                tire.bind().build_spec(),
                brake.bind().build_spec(),
            ));
        }
        car
    }

    /// Ресурс колеса для угла с учётом общих передних/задних колёс.
    fn wheel_for(&self, corner: Corner) -> Option<Gd<WheelResource>> {
        let specific = match corner {
            Corner::FrontLeft => &self.wheel_fl,
            Corner::FrontRight => &self.wheel_fr,
            Corner::RearLeft => &self.wheel_rl,
            Corner::RearRight => &self.wheel_rr,
        };
        if let Some(w) = specific {
            return Some(w.clone());
        }
        if corner.is_front() {
            self.front_wheels.clone()
        } else {
            self.rear_wheels.clone()
        }
    }

    /// Строит ресурс из сборки ядра.
    pub fn from_core(base: Base<Resource>, core: &CarAssembly) -> Self {
        let mut res = Self {
            name: GString::from(core.name.as_str()),
            chassis: core.chassis.as_ref().map(chassis_from_spec),
            engine: core.engine.as_ref().map(engine_from_spec),
            gearbox: core.gearbox.as_ref().map(gearbox_from_spec),
            differential: core.differential.as_ref().map(differential_from_spec),
            center_differential: core
                .center_differential
                .as_ref()
                .map(differential_from_spec),
            body: core.body.as_ref().map(body_from_spec),
            aero: Array::new(),
            front_wheels: None,
            rear_wheels: None,
            wheel_fl: None,
            wheel_fr: None,
            wheel_rl: None,
            wheel_rr: None,
            base,
        };
        for a in &core.aero {
            let g = aero_from_spec(a);
            res.aero.push(&g);
        }
        for wheel in &core.wheels {
            let g = wheel_from_specs(
                wheel.suspension.as_ref(),
                wheel.rim.as_ref(),
                wheel.tire.as_ref(),
                wheel.brake.as_ref(),
            );
            match wheel.corner {
                Corner::FrontLeft => res.wheel_fl = Some(g),
                Corner::FrontRight => res.wheel_fr = Some(g),
                Corner::RearLeft => res.wheel_rl = Some(g),
                Corner::RearRight => res.wheel_rr = Some(g),
            }
        }
        res
    }
}

#[godot_api]
impl CarAssemblyResource {
    /// Проверяет совместимость деталей. Возвращает список сообщений
    /// (ошибки и предупреждения). Пустой список — всё в порядке.
    #[func]
    pub fn validate(&self) -> Array<GString> {
        let issues = self.to_core().validate();
        let mut out = Array::new();
        for issue in issues {
            let prefix = match issue.severity {
                vehicle_core::assembly::Severity::Error => "[ОШИБКА] ",
                vehicle_core::assembly::Severity::Warning => "[ВНИМАНИЕ] ",
            };
            let msg = GString::from(format!("{prefix}{}", issue.message).as_str());
            out.push(&msg);
        }
        out
    }

    /// Есть ли ошибки (а не только предупреждения).
    #[func]
    pub fn has_errors(&self) -> bool {
        CarAssembly::has_errors(&self.to_core().validate())
    }

    /// Полная масса автомобиля по текущей сборке, кг (0 — если сборка неполная).
    #[func]
    pub fn total_mass(&self) -> f32 {
        self.to_core().build().map(|c| c.mass).unwrap_or(0.0)
    }
}

#[godot_api]
impl IResource for CarAssemblyResource {
    fn init(base: Base<Resource>) -> Self {
        Self::from_core(base, &catalog::sample_rwd_coupe())
    }
}

/// Каталог готовых деталей и примеров сборок.
#[derive(GodotClass)]
#[class(base=RefCounted, init)]
pub struct VehicleCatalog {
    base: Base<RefCounted>,
}

#[godot_api]
impl VehicleCatalog {
    /// Собирает заднеприводное спорткупе.
    #[func]
    pub fn sample_rwd_coupe() -> Gd<CarAssemblyResource> {
        Gd::from_init_fn(|base| CarAssemblyResource::from_core(base, &catalog::sample_rwd_coupe()))
    }

    /// Собирает полноприводный автомобиль с автоматом.
    #[func]
    pub fn sample_awd_auto() -> Gd<CarAssemblyResource> {
        Gd::from_init_fn(|base| CarAssemblyResource::from_core(base, &catalog::sample_awd_auto()))
    }

    /// Пустой ресурс сборки.
    #[func]
    pub fn empty_assembly() -> Gd<CarAssemblyResource> {
        Gd::from_init_fn(|base| CarAssemblyResource {
            name: GString::from(""),
            chassis: None,
            engine: None,
            gearbox: None,
            differential: None,
            center_differential: None,
            body: None,
            aero: Array::new(),
            front_wheels: None,
            rear_wheels: None,
            wheel_fl: None,
            wheel_fr: None,
            wheel_rl: None,
            wheel_rr: None,
            base,
        })
    }

    /// Пустой ресурс колеса.
    #[func]
    pub fn empty_wheel() -> Gd<WheelResource> {
        Gd::from_init_fn(|base| WheelResource::empty(base))
    }

    /// Готовое колесо (подвеска + диск + шина + тормоз) по умолчанию.
    #[func]
    pub fn default_wheel() -> Gd<WheelResource> {
        wheel_from_specs(
            Some(&catalog::suspension_sport()),
            Some(&catalog::rim_18()),
            Some(&catalog::tire_235()),
            Some(&catalog::brake_340()),
        )
    }

    /// Ресурс подвески по умолчанию.
    #[func]
    pub fn default_suspension() -> Gd<SuspensionResource> {
        Gd::from_init_fn(|base| SuspensionResource::from_spec(base, &catalog::suspension_sport()))
    }

    /// Ресурс диска по умолчанию.
    #[func]
    pub fn default_rim() -> Gd<RimResource> {
        Gd::from_init_fn(|base| RimResource::from_spec(base, &catalog::rim_18()))
    }

    /// Ресурс шины по умолчанию.
    #[func]
    pub fn default_tire() -> Gd<TireResource> {
        Gd::from_init_fn(|base| TireResource::from_spec(base, &catalog::tire_235()))
    }

    /// Ресурс тормоза по умолчанию.
    #[func]
    pub fn default_brake() -> Gd<BrakeResource> {
        Gd::from_init_fn(|base| BrakeResource::from_spec(base, &catalog::brake_340()))
    }
}
