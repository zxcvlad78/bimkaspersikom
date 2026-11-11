extends Node3D

## Автотест физики: разгон на полном газу, поворот руля, сверка визуала с симуляцией.
## Запуск: godot --headless demo/physics_test.tscn

var body: Node3D
var visual: Node3D
var elapsed := 0.0
var next_report := 1.0


func _ready() -> void:
    var catalog = ClassDB.instantiate("VehicleCatalog")
    var assembly = catalog.call("sample_awd_auto")
    body = ClassDB.instantiate("VbvCarBody")
    body.set("assembly", assembly)
    add_child(body)

    # Коллайдер кузова создаёт сам плагин (VbvCarBody.ensure_collider).

    var ground := StaticBody3D.new()
    var shape := CollisionShape3D.new()
    var box := BoxShape3D.new()
    box.size = Vector3(400.0, 0.5, 400.0)
    shape.shape = box
    ground.add_child(shape)
    ground.position = Vector3(0.0, -0.25, 0.0)
    add_child(ground)

    body.call("set_input_enabled", false)
    print("[test] car spawned, engine running=", body.call("is_engine_running"))

    # Визуал: проверяем, что колёса следуют за симуляцией.
    # CarVisual — дочерний узел машины (кузов наследует её движение).
    visual = preload("res://demo/car_visual.gd").new()
    body.add_child(visual)
    visual.call("setup", body)


func _physics_process(delta: float) -> void:
    if body == null:
        # Машина не создалась (например, расширение не загрузилось) — выходим,
        # чтобы не висеть бесконечно.
        print("[test] ERROR: машина не создана")
        get_tree().quit(1)
        return

    # Полный газ; после 4-й секунды добавляем поворот руля.
    var steer := 0.0
    if elapsed > 4.0:
        steer = 0.35
    body.call("set_controls", 1.0, 0.0, steer, 0.0)

    elapsed += delta

    # Проверка выхода — до любой диагностики, чтобы тест не мог зависнуть.
    if elapsed > 9.0:
        print("[test] RESULT final_speed_kmh=%.1f" % body.call("get_speed_kmh"))
        get_tree().quit()
        return

    if elapsed < next_report:
        return
    next_report += 1.0

    var kmh = body.call("get_speed_kmh")
    var rpm = body.call("get_rpm")
    var gear = body.call("get_gear")
    var pos = body.global_position
    print("[test] t=%.1fs speed=%.1f km/h rpm=%.0f gear=%d pos=(%.1f, %.2f, %.1f)"
        % [elapsed, kmh, rpm, gear, pos.x, pos.y, pos.z])

    var tel = body.call("get_telemetry")
    var eng = body.call("get_engine_state")
    print("       tel on_ground=%s long_g=%.2f | eng running=%s torque=%.0f kw=%.0f rpm=%.0f clutch=%.2f conv_lock=%s"
        % [tel.get("wheels_on_ground"), tel.get("longitudinal_g"),
           eng.get("running"), eng.get("engine_torque"), eng.get("power_kw"), eng.get("rpm"),
           eng.get("clutch_engagement"), eng.get("converter_locked")])

    var wheels = body.call("get_wheel_info")
    var wstr := ""
    for w in wheels:
        wstr += "[c=%s load=%.0f slip=%.2f] " % [w.get("contact"), w.get("load"), w.get("slip")]
    print("       wheels: " + wstr)

    var up = body.global_transform.basis.y
    print("       roll=%.2f deg lateral_g=%.3f yaw=%.3f"
        % [rad_to_deg(acos(clampf(up.y, -1.0, 1.0))), tel.get("lateral_g"), tel.get("yaw_rate")])

    # Сверяем мировые трансформации колёс симуляции и визуальных узлов.
    var sim_tf = body.call("get_wheel_transforms")
    var sim_pos := PackedVector3Array()
    for t in sim_tf:
        var tr: Transform3D = t
        sim_pos.append(tr.origin)
    var vpos := PackedVector3Array()
    for child in visual.get_children():
        if child is Node3D and str(child.name).begins_with("Wheel"):
            var n3: Node3D = child
            vpos.append(n3.global_position)
    var max_err := 0.0
    for i in range(mini(sim_pos.size(), vpos.size())):
        max_err = maxf(max_err, sim_pos[i].distance_to(vpos[i]))
    print("       wheels visual=%d sim=%d max_sync_error=%.4f m"
        % [vpos.size(), sim_pos.size(), max_err])

    # Кузов должен ехать вместе с машиной и находиться рядом с колёсами.
    var body_node: Node3D = visual.get_node_or_null("Body")
    if body_node != null:
        var car_pos: Vector3 = body.global_position
        var body_pos: Vector3 = body_node.global_position
        var dist_to_car: float = body_pos.distance_to(car_pos)
        var nearest_wheel := 999.0
        for p in vpos:
            nearest_wheel = minf(nearest_wheel, body_pos.distance_to(p))
        print("       chassis offset_from_car=%.3f m, nearest_wheel=%.3f m"
            % [dist_to_car, nearest_wheel])

    var susp = body.call("get_suspension_debug")
    var sstr := ""
    for s in susp:
        sstr += "[len=%.3f comp=%.3f vel=%.2f load=%.0f] " % [
            s.get("length"), s.get("compression"), s.get("velocity"), s.get("load")]
    print("       susp: " + sstr)
