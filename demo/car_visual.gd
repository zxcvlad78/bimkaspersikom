class_name CarVisual
extends Node3D

## Стилизованный визуализатор автомобиля.
##
## Кузов собирается из примитивов (BoxMesh / CylinderMesh / PrismMesh), колёса
## анимируются по мировым трансформациям, которые отдаёт физическое ядро.
##
## Использование:
## [codeblock]
## var visual := CarVisual.new()
## visual.setup(car)      # car — узел класса "VbvCarBody"
## car.add_child(visual)  # CarVisual становится дочерним узлом машины
## [/codeblock]
##
## Система координат машины: начало — на земле, в центре колёсной базы,
## вперёд — -Z, вправо — +X. Кузов — обычный дочерний узел (наследует
## трансформацию машины), колёса — узлы с [member Node3D.top_level] и
## мировыми трансформациями от physics-ядра.

# --- Габариты по умолчанию (если get_visual_spec недоступен) ----------------------

const DEF_WHEELBASE := 2.62
const DEF_TRACK_FRONT := 1.58
const DEF_TRACK_REAR := 1.58
const DEF_WHEEL_RADIUS := 0.33
const DEF_WHEEL_WIDTH := 0.235
const DEF_BODY_LENGTH := 4.4
const DEF_BODY_WIDTH := 1.85
const DEF_BODY_HEIGHT := 1.32
const DEF_MASS := 1350.0

# --- Пропорции кузова ------------------------------------------------------------

## Высота нижнего кузова (по ТЗ).
const LOWER_BODY_HEIGHT := 0.62
## Высота пояса кузова: от плеча до линии окон.
const BELT_HEIGHT := 0.22
## Толщина панели крыши.
const ROOF_HEIGHT := 0.06
## Зазор между бортом кузова и покрышкой по ширине (колёсная арка).
const ARCH_GAP := 0.02
## Сужение пояса относительно нижнего кузова — плечо машины.
const BELT_INSET := 0.04
## Толщина бокового порога.
const SKIRT_WIDTH := 0.05
## Число спиц на диске — делает вращение колеса заметным.
const SPOKE_COUNT := 5
## Насколько диск выступает наружу относительно бока покрышки.
const RIM_PROUD := 0.006

# --- Цвета ------------------------------------------------------------------------

const BODY_COLOR := Color(0.75, 0.12, 0.10)
const GLASS_COLOR := Color(0.04, 0.06, 0.11, 0.35)
const PLASTIC_COLOR := Color(0.09, 0.09, 0.10)
const TIRE_COLOR := Color(0.05, 0.05, 0.055)
const RIM_COLOR := Color(0.78, 0.79, 0.82)
const SPOKE_COLOR := Color(0.18, 0.18, 0.19)
const HEADLIGHT_COLOR := Color(1.0, 0.95, 0.8)
const TAILLIGHT_COLOR := Color(1.0, 0.12, 0.05)
const CHROME_COLOR := Color(0.60, 0.60, 0.62)

## Фары: угол конуса прожектора, энергия, дальность.
const SPOT_ANGLE := 35.0
const SPOT_ENERGY := 3.0
const SPOT_RANGE := 25.0
## Базовая сила свечения задних фар.
const TAIL_EMISSION := 1.6

# --- Внутреннее состояние ---------------------------------------------------------

var _car: Node = null
var _body: Node3D = null
var _wheel_roots: Array[Node3D] = []
var _tail_material: StandardMaterial3D = null
var _spec: Dictionary = {}


# --- Публичный API ----------------------------------------------------------------

## Привязывает визуализацию к автомобилю и строит кузов с колёсами.
## [param car] — узел класса "VbvCarBody" (или любой узел с теми же методами).
## Вызывать можно как до, так и после добавления узла в дерево.
func setup(car: Node) -> void:
    _car = car
    _spec = _read_spec(car)
    _build_body()
    _build_wheels()


# --- Чтение параметров ------------------------------------------------------------

## Габариты машины из get_visual_spec(); при отсутствии метода — дефолты.
func _read_spec(car: Node) -> Dictionary:
    var spec: Dictionary = {
        "wheelbase": DEF_WHEELBASE,
        "track_front": DEF_TRACK_FRONT,
        "track_rear": DEF_TRACK_REAR,
        "wheel_radius": DEF_WHEEL_RADIUS,
        "wheel_width": DEF_WHEEL_WIDTH,
        "body_length": DEF_BODY_LENGTH,
        "body_width": DEF_BODY_WIDTH,
        "body_height": DEF_BODY_HEIGHT,
        "mass": DEF_MASS,
    }
    if car == null or not is_instance_valid(car):
        return spec
    if not car.has_method(&"get_visual_spec"):
        return spec
    var raw: Variant = car.call(&"get_visual_spec")
    if not (raw is Dictionary):
        return spec
    var data: Dictionary = raw
    for key in spec.keys():
        if not data.has(key):
            continue
        var value: Variant = data[key]
        if value is float or value is int:
            spec[key] = float(value)
    return spec


# --- Построение кузова ------------------------------------------------------------

## Собирает кузов: нижнюю часть с колёсными арками, пояс, крылья над колёсами,
## капот, багажник, салон, бампера, пороги, антикрыло, фары, зеркала и выхлоп.
func _build_body() -> void:
    _clear()

    var body_w: float = _spec["body_width"]
    var body_h: float = _spec["body_height"]
    var wheelbase: float = _spec["wheelbase"]
    var wheel_r: float = _spec["wheel_radius"]
    var wheel_w: float = _spec["wheel_width"]
    var track_min: float = minf(_spec["track_front"], _spec["track_rear"])

    var sides: Array[float] = [-1.0, 1.0]
    var half_w: float = body_w * 0.5
    var axle_f: float = -wheelbase * 0.5
    var axle_r: float = wheelbase * 0.5

    # Длина кузова: из спецификации, но не меньше, чем нужно, чтобы колёса
    # поместились внутрь силуэта вместе с передним и задним свесами.
    var body_l: float = maxf(_spec["body_length"], wheelbase + 2.0 * (wheel_r + 0.28))
    var half_l: float = body_l * 0.5

    # Вертикальная раскладка: нижний кузов стоит на земле (низ — y = 0), дальше
    # идут пояс, стеклянный салон и панель крыши.
    var lower_top: float = LOWER_BODY_HEIGHT
    var belt_top: float = lower_top + BELT_HEIGHT
    var cabin_h: float = clampf(body_h - belt_top - ROOF_HEIGHT, 0.25, 0.60)
    var cabin_top: float = belt_top + cabin_h
    var tire_top: float = wheel_r * 2.0

    # Салон: уже кузова и смещён назад — «островок» на поясе.
    var cabin_w: float = maxf(body_w - 0.36, 0.6)
    var cabin_len: float = clampf((axle_r - wheel_r - 0.30) - (axle_f + wheel_r + 0.30),
        0.8, 2.6)
    var cabin_z: float = 0.12
    var cabin_front: float = cabin_z - cabin_len * 0.5
    var cabin_rear: float = cabin_z + cabin_len * 0.5

    # Кузов — дочерний узел CarVisual без top_level: наследует движение машины.
    _body = Node3D.new()
    _body.name = "Body"
    add_child(_body)

    var body_mat: StandardMaterial3D = _make_material(BODY_COLOR, 0.65, 0.25)
    var dark_mat: StandardMaterial3D = _make_material(PLASTIC_COLOR, 0.10, 0.60)
    var glass_mat: StandardMaterial3D = _make_glass_material()

    # --- Нижний кузов: body_length x 0.62 x body_width ---
    # Покрышки почти вплотную прилегают к бортам (body_width = колея + 0.22),
    # поэтому в зонах арок борт убирается — иначе колёса не было бы видно.
    var inner_x: float = maxf(track_min * 0.5 - wheel_w * 0.5 - ARCH_GAP, 0.2)
    inner_x = minf(inner_x, half_w - 0.10)
    var arch_half: float = wheel_r + 0.02
    var skin_w: float = half_w - inner_x
    var belt_w: float = half_w - BELT_INSET

    # Центральная «тумба»: днище и полка под салоном на всю длину.
    _add_box(_body, "LowerSpine",
        Vector3(inner_x * 2.0, belt_top, body_l),
        Vector3(0.0, belt_top * 0.5, 0.0), body_mat)

    # Борта и пояс: только там, где нет колёс.
    var spans: Array[Vector2] = [
        Vector2(-half_l, axle_f - arch_half),
        Vector2(axle_f + arch_half, axle_r - arch_half),
        Vector2(axle_r + arch_half, half_l),
    ]
    for i in spans.size():
        var span: Vector2 = spans[i]
        var length: float = span.y - span.x
        if length <= 0.02:
            continue
        var z_mid: float = (span.x + span.y) * 0.5
        for side in sides:
            var tag: String = "L" if side < 0.0 else "R"
            _add_box(_body, "LowerSkin%d%s" % [i, tag],
                Vector3(skin_w, LOWER_BODY_HEIGHT, length),
                Vector3(side * (inner_x + skin_w * 0.5), lower_top * 0.5, z_mid),
                body_mat)
        _add_box(_body, "Belt%d" % i,
            Vector3(belt_w * 2.0, BELT_HEIGHT, length),
            Vector3(0.0, lower_top + BELT_HEIGHT * 0.5, z_mid), body_mat)

    # Крылья над колёсами: верх покрышки уходит под крыло — получается арка.
    for i in 2:
        var axle_z: float = axle_f if i == 0 else axle_r
        _add_box(_body, "Fender%d" % i,
            Vector3(belt_w * 2.0, belt_top - tire_top, arch_half * 2.0),
            Vector3(0.0, (belt_top + tire_top) * 0.5, axle_z), body_mat)

    # --- Капот: плоская панель перед салоном ---
    var hood_front: float = -half_l + 0.05
    var hood_rear: float = cabin_front - 0.02
    _add_box(_body, "Hood",
        Vector3(belt_w * 2.0 - 0.12, 0.03, maxf(hood_rear - hood_front, 0.1)),
        Vector3(0.0, belt_top + 0.005, (hood_front + hood_rear) * 0.5), body_mat)

    # --- Багажник: плоская панель за салоном ---
    var trunk_front: float = cabin_rear + 0.02
    var trunk_rear: float = half_l - 0.05
    _add_box(_body, "Trunk",
        Vector3(belt_w * 2.0 - 0.12, 0.03, maxf(trunk_rear - trunk_front, 0.1)),
        Vector3(0.0, belt_top + 0.005, (trunk_front + trunk_rear) * 0.5), body_mat)

    # --- Салон: тёмное стекло, смещён назад и вверх ---
    _add_box(_body, "Cabin",
        Vector3(cabin_w, cabin_h, cabin_len),
        Vector3(0.0, belt_top + cabin_h * 0.5 - 0.01, cabin_z), glass_mat)

    # --- Крыша: тонкая панель цвета кузова над стеклом ---
    _add_box(_body, "Roof",
        Vector3(cabin_w + 0.06, ROOF_HEIGHT, maxf(cabin_len - 0.10, 0.1)),
        Vector3(0.0, cabin_top + ROOF_HEIGHT * 0.5 - 0.01, cabin_z), body_mat)

    # --- Бампера: тёмный пластик по краям кузова ---
    _add_box(_body, "FrontBumper",
        Vector3(body_w, 0.26, 0.16),
        Vector3(0.0, 0.30, -half_l - 0.04), dark_mat)
    _add_box(_body, "RearBumper",
        Vector3(body_w, 0.26, 0.16),
        Vector3(0.0, 0.30, half_l + 0.04), dark_mat)

    # --- Боковые пороги / юбки между колёсами ---
    var skirt_len: float = (axle_r - arch_half) - (axle_f + arch_half)
    for side in sides:
        var tag: String = "L" if side < 0.0 else "R"
        _add_box(_body, "Skirt%s" % tag,
            Vector3(SKIRT_WIDTH, 0.16, maxf(skirt_len, 0.1)),
            Vector3(side * (half_w + SKIRT_WIDTH * 0.5 - 0.01), 0.14,
                (axle_f + axle_r) * 0.5), dark_mat)

    # --- Крыло (spoiler) на заднем бампере: клин, приподнятый к хвосту ---
    var wing_z: float = half_l - 0.18
    var wing_y: float = belt_top + 0.07
    _add_prism(_body, "Spoiler",
        Vector3(0.34, 0.09, belt_w * 1.35),
        Vector3(0.0, wing_y, wing_z), 0.0, dark_mat)
    for side in sides:
        var tag: String = "L" if side < 0.0 else "R"
        _add_box(_body, "SpoilerStand%s" % tag,
            Vector3(0.05, 0.12, 0.10),
            Vector3(side * belt_w * 0.42, wing_y - 0.07, wing_z + 0.08), dark_mat)

    # --- Фары: эмиссия + прожектор на каждую ---
    var head_mat: StandardMaterial3D = _make_emissive(HEADLIGHT_COLOR, 2.0)
    var lamp_x: float = body_w * 0.30
    var lamp_y: float = belt_top - 0.16
    for side in sides:
        var tag: String = "L" if side < 0.0 else "R"
        _add_box(_body, "Headlight%s" % tag,
            Vector3(0.32, 0.12, 0.10),
            Vector3(side * lamp_x, lamp_y, -half_l + 0.02), head_mat)

        # Прожектор светит вдоль -Z своей основы — это направление движения.
        var spot := SpotLight3D.new()
        spot.name = "HeadlightSpot%s" % tag
        spot.position = Vector3(side * lamp_x, lamp_y, -half_l - 0.14)
        spot.spot_angle = SPOT_ANGLE
        spot.light_energy = SPOT_ENERGY
        spot.spot_range = SPOT_RANGE
        spot.light_color = Color(1.0, 0.95, 0.85)
        spot.shadow_enabled = false
        _body.add_child(spot)

    # --- Задние фары: эмиссия красного ---
    _tail_material = _make_emissive(TAILLIGHT_COLOR, TAIL_EMISSION)
    for side in sides:
        var tag: String = "L" if side < 0.0 else "R"
        _add_box(_body, "Taillight%s" % tag,
            Vector3(0.30, 0.11, 0.10),
            Vector3(side * lamp_x, lamp_y + 0.02, half_l + 0.02), _tail_material)

    # --- Зеркала по бокам салона ---
    for side in sides:
        var tag: String = "L" if side < 0.0 else "R"
        _add_box(_body, "Mirror%s" % tag,
            Vector3(0.13, 0.07, 0.07),
            Vector3(side * (cabin_w * 0.5 + 0.07), belt_top + 0.10,
                cabin_front + 0.16), body_mat)

    # --- Выхлопная труба: цилиндр сзади снизу ---
    var exhaust := CylinderMesh.new()
    exhaust.top_radius = 0.045
    exhaust.bottom_radius = 0.045
    exhaust.height = 0.22
    exhaust.radial_segments = 12
    var exhaust_mi := MeshInstance3D.new()
    exhaust_mi.name = "Exhaust"
    exhaust_mi.mesh = exhaust
    exhaust_mi.material_override = _make_material(CHROME_COLOR, 0.90, 0.35)
    exhaust_mi.position = Vector3(-body_w * 0.22, 0.13, half_l + 0.02)
    exhaust_mi.rotation_degrees = Vector3(90.0, 0.0, 0.0)
    _body.add_child(exhaust_mi)


# --- Построение колёс --------------------------------------------------------------

## Создаёт четыре контейнера колёс: покрышка (цилиндр), диск и спицы.
func _build_wheels() -> void:
    var wheel_r: float = _spec["wheel_radius"]
    var wheel_w: float = _spec["wheel_width"]
    var wheelbase: float = _spec["wheelbase"]
    var track_f: float = _spec["track_front"]
    var track_r: float = _spec["track_rear"]

    var tire_mat: StandardMaterial3D = _make_material(TIRE_COLOR, 0.0, 0.90)
    var rim_mat: StandardMaterial3D = _make_material(RIM_COLOR, 0.85, 0.30)
    var spoke_mat: StandardMaterial3D = _make_material(SPOKE_COLOR, 0.70, 0.40)

    # Порядок как в физике: [FL, FR, RL, RR].
    var names: PackedStringArray = ["FL", "FR", "RL", "RR"]
    var local_pos: Array[Vector3] = [
        Vector3(-track_f * 0.5, wheel_r, -wheelbase * 0.5),
        Vector3(track_f * 0.5, wheel_r, -wheelbase * 0.5),
        Vector3(-track_r * 0.5, wheel_r, wheelbase * 0.5),
        Vector3(track_r * 0.5, wheel_r, wheelbase * 0.5),
    ]

    # Базовая мировая трансформация машины — для стартовой позы колёс.
    var base := Transform3D(Basis(), Vector3.ZERO)
    if _car is Node3D and (_car as Node3D).is_inside_tree():
        base = (_car as Node3D).global_transform

    for i in names.size():
        var root := Node3D.new()
        root.name = "Wheel_%s" % names[i]
        add_child(root)
        _wheel_roots.append(root)

        # Контейнер колёса не наследует трансформацию машины: позу задаёт
        # физика через get_wheel_transforms(). Стартовая поза считается по
        # габаритам; у top_level-узла transform равна global_transform, так
        # что начальная мировая поза корректна и до входа в дерево.
        root.top_level = true
        root.global_transform = base * Transform3D(Basis(), local_pos[i])

        # Покрышка: цилиндр по умолчанию смотрит вдоль Y, поэтому меш
        # поворачиваем на -90° вокруг Z — ось колеса становится вдоль X.
        var tire := CylinderMesh.new()
        tire.top_radius = wheel_r
        tire.bottom_radius = wheel_r
        tire.height = wheel_w
        tire.radial_segments = 20
        var tire_mi := MeshInstance3D.new()
        tire_mi.name = "Tire"
        tire_mi.mesh = tire
        tire_mi.material_override = tire_mat
        tire_mi.rotation_degrees = Vector3(0.0, 0.0, -90.0)
        root.add_child(tire_mi)

        # Диск: радиус 0.62 * wheel_radius, ширина wheel_width * 1.02.
        # Стоит на внешней стороне колеса, иначе его не видно под покрышкой.
        var rim := CylinderMesh.new()
        rim.top_radius = wheel_r * 0.62
        rim.bottom_radius = wheel_r * 0.62
        rim.height = wheel_w * 1.02
        rim.radial_segments = 16
        var rim_mi := MeshInstance3D.new()
        rim_mi.name = "Rim"
        rim_mi.mesh = rim
        rim_mi.material_override = rim_mat
        rim_mi.rotation_degrees = Vector3(0.0, 0.0, -90.0)
        # Направление «наружу» колеса: левое — -X, правое — +X.
        var outward: float = -1.0 if local_pos[i].x < 0.0 else 1.0
        var rim_x: float = outward * (wheel_w * 0.5 + RIM_PROUD - wheel_w * 1.02 * 0.5)
        rim_mi.position = Vector3(rim_x, 0.0, 0.0)
        root.add_child(rim_mi)

        # Спицы на диске: делают вращение колеса заметным.
        var rim_face: float = rim_x + outward * (wheel_w * 1.02 * 0.5)
        var spoke_r: float = wheel_r * 0.62 * 0.55
        var spoke_len: float = wheel_r * 0.62 * 0.75
        for s in SPOKE_COUNT:
            var ang: float = TAU * float(s) / float(SPOKE_COUNT)
            var spoke := _add_box(root, "Spoke%d" % s,
                Vector3(0.010, spoke_len, 0.045),
                Vector3(rim_face, spoke_r * cos(ang), spoke_r * sin(ang)), spoke_mat)
            spoke.rotation_degrees = Vector3(rad_to_deg(ang), 0.0, 0.0)


# --- Обновление --------------------------------------------------------------------

func _process(_delta: float) -> void:
    if _car == null or not is_instance_valid(_car):
        return
    _update_wheels()
    _update_lights()


## Ставит колёсам мировые трансформации из физики (поворот руля + вращение).
func _update_wheels() -> void:
    if _wheel_roots.is_empty():
        return
    if not _car.has_method(&"get_wheel_transforms"):
        return
    var raw: Variant = _car.call(&"get_wheel_transforms")
    if not (raw is Array):
        return
    var transforms: Array = raw
    if transforms.size() < 4 or transforms.size() < _wheel_roots.size():
        return
    for i in _wheel_roots.size():
        var value: Variant = transforms[i]
        if not (value is Transform3D):
            return
        var root: Node3D = _wheel_roots[i]
        if not root.top_level:
            root.top_level = true
        root.global_transform = value


## Слабое свечение задних фар при пробуксовке задних колёс.
func _update_lights() -> void:
    if _tail_material == null:
        return
    var boost: float = 1.0
    if _car.has_method(&"get_wheel_info"):
        var raw: Variant = _car.call(&"get_wheel_info")
        if raw is Array:
            var infos: Array = raw
            var slip: float = 0.0
            for i in range(2, 4):  # задние колёса
                if i >= infos.size():
                    break
                var info: Variant = infos[i]
                if not (info is Dictionary):
                    continue
                var data: Dictionary = info
                slip = maxf(slip, absf(float(data.get("slip", 0.0))))
            boost = 1.0 + minf(slip, 1.0) * 0.8
    _tail_material.emission_energy_multiplier = TAIL_EMISSION * boost


# --- Вспомогательное -----------------------------------------------------------------

## Удаляет ранее построенную геометрию (повторный вызов setup).
func _clear() -> void:
    for child in get_children():
        remove_child(child)
        child.queue_free()
    _body = null
    _wheel_roots.clear()
    _tail_material = null


## Стандартный материал с заданными параметрами.
func _make_material(color: Color, metallic: float, roughness: float) -> StandardMaterial3D:
    var mat := StandardMaterial3D.new()
    mat.albedo_color = color
    mat.metallic = metallic
    mat.roughness = roughness
    return mat


## Материал тёмного стекла салона.
func _make_glass_material() -> StandardMaterial3D:
    var mat := StandardMaterial3D.new()
    mat.albedo_color = GLASS_COLOR
    mat.metallic = 0.10
    mat.roughness = 0.05
    mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
    mat.cull_mode = BaseMaterial3D.CULL_DISABLED
    return mat


## Материал со свечением.
func _make_emissive(color: Color, energy: float) -> StandardMaterial3D:
    var mat := StandardMaterial3D.new()
    mat.albedo_color = color
    mat.metallic = 0.0
    mat.roughness = 0.20
    mat.emission_enabled = true
    mat.emission = color
    mat.emission_energy_multiplier = energy
    return mat


## Добавляет коробку-примитив к узлу.
func _add_box(parent: Node3D, node_name: String, size: Vector3,
        pos: Vector3, mat: Material) -> MeshInstance3D:
    var mesh := BoxMesh.new()
    mesh.size = size
    var instance := MeshInstance3D.new()
    instance.name = node_name
    instance.mesh = mesh
    instance.material_override = mat
    instance.position = pos
    parent.add_child(instance)
    return instance


## Добавляет клин (PrismMesh) к узлу.
##
## Клин повёрнут вдоль машины: его длина (size.x) идёт по Z, ширина (size.z)
## — по X. [param left_to_right] — положение верхней кромки: 0.0 — высокая
## сторона смотрит назад (+Z), 1.0 — вперёд (-Z).
func _add_prism(parent: Node3D, node_name: String, size: Vector3,
        pos: Vector3, left_to_right: float, mat: Material) -> MeshInstance3D:
    var mesh := PrismMesh.new()
    mesh.size = size
    mesh.left_to_right = left_to_right
    var instance := MeshInstance3D.new()
    instance.name = node_name
    instance.mesh = mesh
    instance.material_override = mat
    instance.position = pos
    instance.rotation_degrees = Vector3(0.0, 90.0, 0.0)
    parent.add_child(instance)
    return instance
