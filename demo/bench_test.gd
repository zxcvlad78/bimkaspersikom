extends Node3D

## Бенчмарк динамики автомобиля (по мотивам реальных автотестов).
##
## Замеры:
##   * время разгона 0-100 км/ч;
##   * тормозной путь со 100 км/ч;
##   * максимальное боковое ускорение (skidpad) и радиус поворота;
##   * максимальная скорость.
##
## Запуск: godot --headless demo/bench_test.tscn

const TARGET_SPEED_KMH := 100.0

var body: Node3D
var ground: StaticBody3D
var phase := 0
var elapsed := 0.0
var t_start := 0.0
var t_100 := -1.0
var brake_start_pos := Vector3.ZERO
var brake_distance := 0.0
var max_lat_g := 0.0
var skid_radius := 0.0
var skid_center := Vector3.ZERO
var max_speed := 0.0
var phase_time := 0.0
var last_pos := Vector3.ZERO


func _ready() -> void:
    var catalog = ClassDB.instantiate("VehicleCatalog")
    var assembly = catalog.call("sample_awd_auto")
    body = ClassDB.instantiate("VbvCarBody")
    body.set("assembly", assembly)
    add_child(body)

    ground = StaticBody3D.new()
    var shape := CollisionShape3D.new()
    var box := BoxShape3D.new()
    box.size = Vector3(4000.0, 0.5, 4000.0)
    shape.shape = box
    ground.add_child(shape)
    ground.position = Vector3(0.0, -0.25, 0.0)
    add_child(ground)

    body.call("set_input_enabled", false)
    last_pos = body.global_position
    print("[bench] phase=ACCEL start")


func _physics_process(delta: float) -> void:
    if body == null:
        get_tree().quit(1)
        return
    elapsed += delta
    phase_time += delta

    var kmh: float = body.call("get_speed_kmh")
    max_speed = maxf(max_speed, kmh)
    var tel: Dictionary = body.call("get_telemetry")

    match phase:
        0:
            # Разгон на полном газу.
            body.call("set_controls", 1.0, 0.0, 0.0, 0.0)
            if kmh >= TARGET_SPEED_KMH and t_100 < 0.0:
                t_100 = elapsed - t_start
                print("[bench] 0-100 km/h = %.2f s" % t_100)
                brake_start_pos = body.global_position
                phase = 1
                phase_time = 0.0
        1:
            # Торможение со 100 км/ч.
            body.call("set_controls", 0.0, 1.0, 0.0, 0.0)
            if kmh <= 1.0:
                brake_distance = body.global_position.distance_to(brake_start_pos)
                print("[bench] braking 100-0 km/h = %.1f m" % brake_distance)
                phase = 2
                phase_time = 0.0
                body.call("reset_state")
                body.global_position = Vector3(0.0, 0.2, 0.0)
                body.global_rotation = Vector3.ZERO
                last_pos = body.global_position
                print("[bench] phase=SKIDPAD")
        2:
            # Skidpad: держим постоянную скорость регулятором газа и едем
            # по кругу с фиксированным рулём.
            var target_v: float = 60.0 / 3.6
            var err_v: float = target_v - float(tel.get("speed"))
            var thr: float = clampf(err_v * 0.5, 0.0, 0.6)
            body.call("set_controls", thr, 0.0, 0.35, 0.0)
            var lat_g: float = absf(float(tel.get("lateral_g")))
            max_lat_g = maxf(max_lat_g, lat_g)
            if phase_time > 2.0 and skid_radius <= 0.0:
                # Радиус = |v|² / (a_lat)
                var v: float = float(tel.get("speed"))
                if v > 5.0 and lat_g > 0.1:
                    skid_radius = v * v / (lat_g * 9.81)
                    skid_center = body.global_position
            if fmod(phase_time, 0.75) < delta:
                var wi = body.call("get_wheel_info")
                var wstr := ""
                for w in wi:
                    wstr += "[c=%s load=%.0f slip=%.3f] " % [w.get("contact"), w.get("load"), w.get("slip")]
                print("[bench] skid v=%.1f yaw=%.3f lat_g=%.3f slip_angle=%.3f %s"
                    % [kmh / 3.6, float(tel.get("yaw_rate")), lat_g,
                       float(tel.get("slip_angle")), wstr])
            if phase_time > 8.0:
                print("[bench] max lateral = %.2f g, skidpad radius = %.0f m" % [max_lat_g, skid_radius])
                print("[bench] top speed = %.1f km/h" % max_speed)
                print("[bench] RESULT accel_0_100=%.2f brake_m=%.1f lat_g=%.2f skid_m=%.0f vmax=%.1f"
                    % [t_100, brake_distance, max_lat_g, skid_radius, max_speed])
                get_tree().quit()
                return

    last_pos = body.global_position
    if elapsed > 60.0:
        print("[bench] RESULT timeout accel_0_100=%.2f brake_m=%.1f lat_g=%.2f vmax=%.1f"
            % [t_100, brake_distance, max_lat_g, max_speed])
        get_tree().quit()
