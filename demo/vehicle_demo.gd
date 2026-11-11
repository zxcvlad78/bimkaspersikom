extends Node3D

## Полноценное игровое демо: машина из каталога + земля + камера + звук + приборы + гараж.
## Управление: WASD — газ/тормоз/руль, Shift — ручной тормоз, Q — сцепление,
## E/R — переключение передач, F — зажигание, Space — стартер, G — гараж.

const ACTIONS := [
    "vehicle_throttle",
    "vehicle_brake",
    "vehicle_handbrake",
    "vehicle_steer_left",
    "vehicle_steer_right",
    "vehicle_clutch",
    "vehicle_shift_up",
    "vehicle_shift_down",
    "vehicle_ignition",
    "vehicle_starter",
    "vehicle_garage",
]

const KEYMAP := {
    "vehicle_throttle": KEY_W,
    "vehicle_brake": KEY_S,
    "vehicle_handbrake": KEY_SHIFT,
    "vehicle_steer_left": KEY_A,
    "vehicle_steer_right": KEY_D,
    "vehicle_clutch": KEY_Q,
    "vehicle_shift_up": KEY_E,
    "vehicle_shift_down": KEY_R,
    "vehicle_ignition": KEY_F,
    "vehicle_starter": KEY_SPACE,
    "vehicle_garage": KEY_G,
}

const CarVisualScript = preload("res://demo/car_visual.gd")
const EngineAudioScript = preload("res://demo/engine_audio.gd")
const VehicleHudScript = preload("res://demo/hud.gd")
const GarageMenuScript = preload("res://demo/garage.gd")

var body: Node3D
var cam: Camera3D
var visual: Node3D
var audio: Node
var hud: CanvasLayer
var garage: CanvasLayer
var cam_dir := Vector3(0.0, 0.0, 1.0)
var cam_init := false


func _ready() -> void:
    _setup_actions()
    _spawn_car()
    _setup_ground()
    _setup_camera()
    _setup_modules()
    # При закрытии окна сначала глушим звук и ждём микс-шаг аудиосервера,
    # иначе остаётся предупреждение об утечке playback при выходе.
    get_window().close_requested.connect(_on_close_requested)


func _on_close_requested() -> void:
    if audio != null and audio.has_method("stop"):
        audio.call("stop")
    await get_tree().create_timer(0.12).timeout
    get_tree().quit()


func _setup_actions() -> void:
    for action in ACTIONS:
        if not InputMap.has_action(action):
            InputMap.add_action(action)
    for action in ACTIONS:
        var ev := InputEventKey.new()
        ev.physical_keycode = KEYMAP[action]
        ev.pressed = true
        InputMap.action_add_event(action, ev)


func _spawn_car() -> void:
    var catalog = ClassDB.instantiate("VehicleCatalog")
    if catalog == null:
        push_error("[vehicle] Failed to instantiate VehicleCatalog")
        return
    var assembly = catalog.call("sample_awd_auto")
    body = ClassDB.instantiate("VbvCarBody")
    if body == null:
        push_error("[vehicle] Failed to instantiate VbvCarBody")
        return
    body.set("assembly", assembly)
    # Коллайдер кузова создаёт сам плагин: он должен быть НАД колёсами,
    # иначе подвеска не сжимается и машина не едет.
    add_child(body)


func _setup_ground() -> void:
    var ground := StaticBody3D.new()
    var shape := CollisionShape3D.new()
    var box := BoxShape3D.new()
    box.size = Vector3(400.0, 0.5, 400.0)
    shape.shape = box
    ground.add_child(shape)
    var ground_mesh := MeshInstance3D.new()
    var ground_box := BoxMesh.new()
    ground_box.size = Vector3(400.0, 0.5, 400.0)
    ground_mesh.mesh = ground_box
    var mat := StandardMaterial3D.new()
    mat.albedo_color = Color(0.16, 0.17, 0.19)
    mat.roughness = 0.95
    ground_mesh.material_override = mat
    ground.add_child(ground_mesh)
    ground.position = Vector3(0.0, -0.25, 0.0)
    add_child(ground)


func _setup_camera() -> void:
    cam = Camera3D.new()
    cam.position = Vector3(0.0, 2.4, 7.0)
    add_child(cam)
    cam.make_current()


func _setup_modules() -> void:
    # CarVisual должен быть дочерним узлом машины: кузов наследует её
    # движение, а колёса ставятся в мировые координаты (top_level).
    visual = CarVisualScript.new()
    body.add_child(visual)
    visual.call("setup", body)

    audio = EngineAudioScript.new()
    add_child(audio)
    audio.call("setup", body)

    hud = VehicleHudScript.new()
    add_child(hud)
    hud.call("setup", body)

    garage = GarageMenuScript.new()
    add_child(garage)
    garage.call("setup", body)


func _process(delta: float) -> void:
    if body == null:
        return

    # Гараж открывается/закрывается на G.
    if Input.is_action_just_pressed(StringName("vehicle_garage")) and garage != null:
        garage.toggle()

    # Камера едет за машиной. Направление берём прямо из базиса кузова
    # (вперёд — это −Z), без извлечения yaw: так камера не «прыгает» при крене.
    var fwd: Vector3 = -body.global_transform.basis.z
    var flat := Vector3(fwd.x, 0.0, fwd.z)
    if flat.length_squared() < 0.0001:
        flat = Vector3(0.0, 0.0, -1.0)
    flat = flat.normalized()

    var blend := clampf(delta * 5.0, 0.0, 1.0)
    cam_dir = cam_dir.lerp(flat, blend).normalized()
    if cam_dir.length_squared() < 0.0001:
        cam_dir = flat

    var focus: Vector3 = body.global_position + Vector3(0.0, 0.9, 0.0)
    var desired: Vector3 = focus - cam_dir * 7.5 + Vector3(0.0, 2.4, 0.0)
    if cam_init == false:
        cam.global_position = desired
        cam_init = true
    else:
        cam.global_position = cam.global_position.lerp(desired, clampf(delta * 8.0, 0.0, 1.0))
    cam.look_at(focus)
