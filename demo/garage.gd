class_name GarageMenu
extends CanvasLayer

## «Гараж» — меню механика.
##
## Позволяет выбрать детали машины (двигатель, КПП, привод, шины, дифференциал,
## подвеску), собрать из них `CarAssemblyResource` и применить сборку к живой
## машине (`VbvCarBody`): узел получает новую сборку и пересобирается.
##
## Все детали создаются кодом через `ClassDB.instantiate(...)`, потому что
## классы приходят из Rust GDExtension (`vehicle_physics`), а не из .tres-файлов.

# --- Категории деталей ---------------------------------------------------

const CAT_ENGINE := 0
const CAT_GEARBOX := 1
const CAT_TIRE := 2
const CAT_DIFFERENTIAL := 3
const CAT_SUSPENSION := 4
const CAT_DRIVE := 5

const CATEGORY_TITLES: PackedStringArray = [
    "Двигатель",
    "Коробка передач",
    "Шины",
    "Дифференциал",
    "Подвеска",
    "Привод",
]

# --- Схемы привода (enum GearboxResource.layout) -------------------------

const DRIVE_RWD := 0
const DRIVE_FWD := 1
const DRIVE_AWD := 2

const DRIVE_LAYOUTS: PackedInt32Array = [DRIVE_RWD, DRIVE_FWD, DRIVE_AWD]
const DRIVE_TITLES: PackedStringArray = [
    "Задний привод (RWD)",
    "Передний привод (FWD)",
    "Полный привод (AWD)",
]

# --- Названия для числовых enum'ов ресурсов ------------------------------

const ASPIRATION_NAMES: PackedStringArray = ["Атмосферный", "Турбо", "Наддув (суперчарджер)"]
const FUEL_NAMES: PackedStringArray = ["АИ-95", "АИ-98", "E85", "Дизель", "Газ"]
const DIFFERENTIAL_KIND_NAMES: PackedStringArray = ["Открытый", "Блокируемый", "LSD", "Torsen"]
const SUSPENSION_KIND_NAMES: PackedStringArray = [
    "McPherson",
    "Двухрычажная",
    "Многорычажная",
    "Trailing arm",
    "Жёсткий мост",
    "Pushrod",
]

# --- Поля коробки передач, которые переносятся в копию --------------------

const GEARBOX_PROPERTIES: PackedStringArray = [
    "part_id",
    "part_name",
    "manufacturer",
    "mass",
    "price",
    "condition",
    "ratios",
    "reverse",
    "final_drive",
    "efficiency",
    "shift_time",
    "automatic",
    "upshift_rpm",
    "downshift_rpm",
    "kickdown_throttle",
    "max_torque",
    "inertia",
    "reverse_max_speed",
    "converter",
    "converter_stall_ratio",
    "converter_coupling_ratio",
    "converter_capacity",
    "converter_lockup",
    "converter_lockup_speed",
    "converter_lockup_capacity",
    "size",
    "mount",
]

# --- Прочее ---------------------------------------------------------------

const STATUS_TIMEOUT := 2.5
const ASSEMBLY_NAME := "Сборка из гаража"

# --- Состояние ------------------------------------------------------------

## Узел машины (`VbvCarBody`), к которому применяется сборка.
var _car: Node = null
## Исходная сборка машины — для кнопки «Сбросить».
var _original_assembly: Variant = null
## Выбранный индекс детали в каждой категории.
var _selected := {
    CAT_ENGINE: 0,
    CAT_GEARBOX: 0,
    CAT_TIRE: 0,
    CAT_DIFFERENTIAL: 0,
    CAT_SUSPENSION: 0,
    CAT_DRIVE: 0,
}
## Текущая открытая категория.
var _current_category: int = CAT_ENGINE
## Управление машиной было отключено открытым меню.
var _car_input_suspended := false
## Состояние управления машиной до открытия меню.
var _prev_input_enabled := true

# --- Каталог деталей ------------------------------------------------------

var _chassis: Resource = null
var _body: Resource = null
var _engines: Array[Resource] = []
var _gearboxes: Array[Resource] = []
var _tires: Array[Resource] = []
var _rims: Array[Resource] = []
var _differentials: Array[Resource] = []
var _center_differential: Resource = null
var _suspensions: Array[Resource] = []
var _brake: Resource = null

# --- Узлы интерфейса ------------------------------------------------------

var _ui_ready := false
var _root: Control = null
var _panel: PanelContainer = null
var _category_list: ItemList = null
var _part_list: ItemList = null
var _details_box: VBoxContainer = null
var _detail_labels: Array[Label] = []
var _info_label: Label = null
var _issues_label: Label = null
var _status_label: Label = null
var _status_timer: Timer = null


# ---------------------------------------------------------------------------
# Публичный API
# ---------------------------------------------------------------------------

## Привязывает меню к машине. Сохраняет её текущую сборку для «Сбросить».
func setup(car: Node) -> void:
    _car = car
    _load_original_assembly()
    if _ui_ready:
        _refresh_all()


## Открывает меню гаража.
func open() -> void:
    if _ui_ready:
        _panel.visible = true
        _refresh_all()
    _suspend_car_input(true)


## Закрывает меню гаража.
func close() -> void:
    if _ui_ready:
        _panel.visible = false
    _suspend_car_input(false)


## Переключает видимость меню.
func toggle() -> void:
    if _panel != null and _panel.visible:
        close()
    else:
        open()


# ---------------------------------------------------------------------------
# Жизненный цикл
# ---------------------------------------------------------------------------

func _ready() -> void:
    layer = 10
    _build_ui()
    _build_catalog()
    # Штатная схема привода берётся из выбранной по умолчанию коробки передач.
    var default_gearbox := _selected_gearbox()
    if default_gearbox != null:
        _selected[CAT_DRIVE] = _drive_index_for_layout(int(default_gearbox.get("layout")))
    _populate_categories()
    _refresh_all()
    _ui_ready = true
    _panel.visible = false


func _input(event: InputEvent) -> void:
    if not (event is InputEventKey):
        return
    var key_event := event as InputEventKey
    if not key_event.pressed or key_event.echo:
        return
    if key_event.keycode == KEY_TAB or key_event.physical_keycode == KEY_TAB:
        toggle()
        get_viewport().set_input_as_handled()
    elif key_event.keycode == KEY_ESCAPE and _panel != null and _panel.visible:
        close()
        get_viewport().set_input_as_handled()


# ---------------------------------------------------------------------------
# Построение интерфейса
# ---------------------------------------------------------------------------

func _build_ui() -> void:
    _root = Control.new()
    _root.name = "GarageRoot"
    _root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
    _root.mouse_filter = Control.MOUSE_FILTER_IGNORE
    add_child(_root)

    var center := CenterContainer.new()
    center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
    center.mouse_filter = Control.MOUSE_FILTER_IGNORE
    _root.add_child(center)

    _panel = PanelContainer.new()
    _panel.custom_minimum_size = Vector2(1000.0, 620.0)
    center.add_child(_panel)

    var main_box := VBoxContainer.new()
    main_box.add_theme_constant_override("separation", 8)
    _panel.add_child(main_box)

    # --- Заголовок ---
    var header := HBoxContainer.new()
    main_box.add_child(header)

    var title := Label.new()
    title.text = "ГАРАЖ"
    title.add_theme_font_size_override("font_size", 24)
    header.add_child(title)

    var hint := Label.new()
    hint.text = "TAB — открыть/закрыть   •   ESC — закрыть"
    hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
    hint.size_flags_horizontal = Control.SIZE_EXPAND_FILL
    header.add_child(hint)

    main_box.add_child(HSeparator.new())

    # --- Основная часть: категории слева, детали справа ---
    var body := HBoxContainer.new()
    body.size_flags_vertical = Control.SIZE_EXPAND_FILL
    body.add_theme_constant_override("separation", 10)
    main_box.add_child(body)

    var left := VBoxContainer.new()
    left.custom_minimum_size = Vector2(190.0, 0.0)
    left.add_theme_constant_override("separation", 4)
    body.add_child(left)

    var left_title := Label.new()
    left_title.text = "Категории"
    left.add_child(left_title)

    _category_list = ItemList.new()
    _category_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
    _category_list.item_selected.connect(_on_category_selected)
    left.add_child(_category_list)

    var right := VBoxContainer.new()
    right.size_flags_horizontal = Control.SIZE_EXPAND_FILL
    right.add_theme_constant_override("separation", 4)
    body.add_child(right)

    var right_title := Label.new()
    right_title.text = "Детали"
    right.add_child(right_title)

    # Список деталей скроллится сам, чтобы он не съедал место у карточек.
    _part_list = ItemList.new()
    _part_list.custom_minimum_size = Vector2(0.0, 200.0)
    _part_list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
    _part_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
    _part_list.item_selected.connect(_on_part_selected)
    right.add_child(_part_list)

    var details_scroll := ScrollContainer.new()
    details_scroll.custom_minimum_size = Vector2(0.0, 150.0)
    details_scroll.size_flags_horizontal = Control.SIZE_EXPAND_FILL
    details_scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
    details_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
    right.add_child(details_scroll)

    _details_box = VBoxContainer.new()
    _details_box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
    _details_box.add_theme_constant_override("separation", 2)
    details_scroll.add_child(_details_box)

    for _i in range(14):
        var detail := Label.new()
        detail.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
        detail.size_flags_horizontal = Control.SIZE_EXPAND_FILL
        _details_box.add_child(detail)
        _detail_labels.append(detail)

    main_box.add_child(HSeparator.new())

    # --- Итоги и проблемы ---
    _info_label = Label.new()
    _info_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
    main_box.add_child(_info_label)

    _issues_label = Label.new()
    _issues_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
    _issues_label.add_theme_color_override("font_color", Color(1.0, 0.72, 0.25))
    main_box.add_child(_issues_label)

    # --- Кнопки ---
    var buttons := HBoxContainer.new()
    buttons.alignment = BoxContainer.ALIGNMENT_CENTER
    buttons.add_theme_constant_override("separation", 12)
    main_box.add_child(buttons)

    var apply_button := Button.new()
    apply_button.text = "Применить"
    apply_button.custom_minimum_size = Vector2(150.0, 38.0)
    apply_button.pressed.connect(_on_apply_pressed)
    buttons.add_child(apply_button)

    var reset_button := Button.new()
    reset_button.text = "Сбросить"
    reset_button.custom_minimum_size = Vector2(150.0, 38.0)
    reset_button.pressed.connect(_on_reset_pressed)
    buttons.add_child(reset_button)

    var close_button := Button.new()
    close_button.text = "Закрыть"
    close_button.custom_minimum_size = Vector2(150.0, 38.0)
    close_button.pressed.connect(_on_close_pressed)
    buttons.add_child(close_button)

    _status_label = Label.new()
    _status_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
    _status_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
    _status_label.modulate = Color(1.0, 1.0, 1.0, 0.0)
    main_box.add_child(_status_label)

    _status_timer = Timer.new()
    _status_timer.one_shot = true
    _status_timer.wait_time = STATUS_TIMEOUT
    _status_timer.timeout.connect(_on_status_timeout)
    add_child(_status_timer)

    # Кнопка вызова гаража: видна всегда, в том числе когда панель закрыта.
    # Добавляется последним, чтобы рисоваться поверх панели.
    var toggle_button := Button.new()
    toggle_button.text = "Гараж (TAB)"
    toggle_button.set_anchors_and_offsets_preset(Control.PRESET_TOP_RIGHT)
    toggle_button.offset_left = -160.0
    toggle_button.offset_top = 12.0
    toggle_button.offset_right = -12.0
    toggle_button.offset_bottom = 44.0
    toggle_button.pressed.connect(toggle)
    _root.add_child(toggle_button)


# ---------------------------------------------------------------------------
# Каталог деталей
# ---------------------------------------------------------------------------

func _build_catalog() -> void:
    _chassis = _make_chassis()
    _body = _make_body()
    _brake = _make_brake()
    _center_differential = _make_center_differential()

    var engines: Array[Resource] = []
    engines.append(_make_engine_2l_turbo())
    engines.append(_make_engine_v8_5l_na())
    engines.append(_make_engine_1l6_turbo())
    _engines = engines

    var gearboxes: Array[Resource] = []
    gearboxes.append(_make_gearbox_6mt_rwd())
    gearboxes.append(_make_gearbox_6at_awd())
    _gearboxes = gearboxes

    var tires: Array[Resource] = []
    tires.append(_make_tire_street())
    tires.append(_make_tire_semi_slick())
    tires.append(_make_tire_track_slick())
    _tires = tires

    # Диски подбираются под шину, чтобы радиус диска совпадал с профилем.
    var rims: Array[Resource] = []
    rims.append(_make_rim(18.0, 8.5, 45.0))
    rims.append(_make_rim(19.0, 9.0, 40.0))
    rims.append(_make_rim(20.0, 9.5, 35.0))
    _rims = rims

    var differentials: Array[Resource] = []
    differentials.append(_make_differential_open())
    differentials.append(_make_differential_lsd())
    differentials.append(_make_differential_torsen())
    _differentials = differentials

    var suspensions: Array[Resource] = []
    suspensions.append(_make_suspension_sport())
    suspensions.append(_make_suspension_double_wishbone())
    _suspensions = suspensions


## Создаёт ресурс класса из ClassDB (Rust GDExtension) либо `null`.
func _make_part(class_name_: String) -> Resource:
    if not ClassDB.class_exists(class_name_):
        push_error("[garage] Класс %s недоступен через ClassDB" % class_name_)
        return null
    var object: Object = ClassDB.instantiate(class_name_)
    if object == null:
        push_error("[garage] Не удалось создать ресурс %s" % class_name_)
        return null
    return object as Resource


func _make_chassis() -> Resource:
    var chassis := _make_part("ChassisResource")
    if chassis == null:
        return null
    chassis.set("part_id", "chassis_coupe")
    chassis.set("part_name", "Шасси купе")
    chassis.set("manufacturer", "Garage Works")
    chassis.set("mass", 1150.0)
    chassis.set("price", 900000.0)
    chassis.set("condition", 1.0)
    chassis.set("wheelbase", 2.62)
    chassis.set("track_front", 1.57)
    chassis.set("track_rear", 1.59)
    chassis.set("com_height", 0.50)
    chassis.set("com_offset", 0.05)
    chassis.set("arch_radius", 0.36)
    chassis.set("hub_bolts", 5)
    chassis.set("hub_pcd_mm", 114)
    chassis.set("max_brake_disc", 0.37)
    chassis.set("torsional_stiffness", 8.0)
    chassis.set("engine_bay", Vector3(0.90, 0.80, 0.70))
    chassis.set("material", 0)
    return chassis


func _make_body() -> Resource:
    var body := _make_part("BodyResource")
    if body == null:
        return null
    body.set("part_id", "body_coupe")
    body.set("part_name", "Кузов купе")
    body.set("manufacturer", "Garage Works")
    body.set("mass", 320.0)
    body.set("price", 350000.0)
    body.set("condition", 1.0)
    body.set("drag_cd", 0.32)
    body.set("frontal_area", 2.05)
    body.set("downforce", 0.6)
    body.set("material", 0)
    return body


func _make_brake() -> Resource:
    var brake := _make_part("BrakeResource")
    if brake == null:
        return null
    brake.set("part_id", "brake340")
    brake.set("part_name", "Тормоз 340 мм, 4 поршня")
    brake.set("manufacturer", "Garage Works")
    brake.set("mass", 9.0)
    brake.set("price", 18000.0)
    brake.set("condition", 1.0)
    brake.set("disc_diameter", 0.34)
    brake.set("disc_thickness", 0.028)
    brake.set("pistons", 4)
    brake.set("pad_mu", 0.44)
    brake.set("vented", true)
    return brake


func _make_engine_2l_turbo() -> Resource:
    var engine := _make_part("EngineResource")
    if engine == null:
        return null
    engine.set("part_id", "i4_2.0t")
    engine.set("part_name", "2.0 Turbo")
    engine.set("manufacturer", "Garage Works")
    engine.set("mass", 160.0)
    engine.set("price", 249000.0)
    engine.set("condition", 1.0)
    engine.set("displacement_l", 2.0)
    engine.set("cylinders", 4)
    engine.set("compression_ratio", 9.5)
    engine.set("plenum_volume_l", 2.4)
    engine.set("volumetric_efficiency", PackedFloat32Array([
        800.0, 0.70, 1500.0, 0.86, 2500.0, 0.94, 3500.0, 0.97,
        4500.0, 0.96, 5500.0, 0.92, 6500.0, 0.84, 7000.0, 0.72,
    ]))
    engine.set("aspiration", 1)
    engine.set("fuel", 1)
    engine.set("afr_target", 12.4)
    engine.set("idle_rpm", 850.0)
    engine.set("redline_rpm", 7000.0)
    engine.set("limiter_rpm", 7200.0)
    engine.set("inertia", 0.22)
    engine.set("friction_constant", 12.0)
    engine.set("friction_linear", 0.06)
    engine.set("friction_quadratic", 0.00022)
    engine.set("coolant_mass", 6.0)
    engine.set("radiator_rate", 900.0)
    engine.set("thermostat_open_c", 88.0)
    engine.set("fan_rate", 500.0)
    engine.set("max_temp_c", 125.0)
    engine.set("oil_mass", 4.5)
    engine.set("oil_cooler_rate", 250.0)
    engine.set("accessory_torque", 6.0)
    engine.set("injector_max_flow", 0.10)
    engine.set("fuel_tank_l", 55.0)
    engine.set("engine_mass", 160.0)
    engine.set("turbo_max_boost", 95000.0)
    engine.set("turbo_wastegate", 85000.0)
    engine.set("turbo_lag", 0.22)
    engine.set("turbo_efficiency", 0.76)
    engine.set("turbo_intercooler", 0.72)
    engine.set("clutch_capacity", 480.0)
    engine.set("clutch_stiffness", 65.0)
    engine.set("size", Vector3(0.62, 0.68, 0.52))
    engine.set("mount", Vector3(0.0, 0.45, -1.1))
    return engine


func _make_engine_v8_5l_na() -> Resource:
    var engine := _make_part("EngineResource")
    if engine == null:
        return null
    engine.set("part_id", "v8_5.0")
    engine.set("part_name", "V8 5.0 NA")
    engine.set("manufacturer", "Garage Works")
    engine.set("mass", 230.0)
    engine.set("price", 480000.0)
    engine.set("condition", 1.0)
    engine.set("displacement_l", 5.0)
    engine.set("cylinders", 8)
    engine.set("compression_ratio", 10.5)
    engine.set("plenum_volume_l", 5.0)
    engine.set("volumetric_efficiency", PackedFloat32Array([
        700.0, 0.72, 1500.0, 0.86, 2500.0, 0.93, 3500.0, 0.96,
        4500.0, 0.95, 5500.0, 0.92, 6500.0, 0.86,
    ]))
    engine.set("aspiration", 0)
    engine.set("fuel", 1)
    engine.set("afr_target", 12.8)
    engine.set("idle_rpm", 700.0)
    engine.set("redline_rpm", 6500.0)
    engine.set("limiter_rpm", 6800.0)
    engine.set("inertia", 0.32)
    engine.set("friction_constant", 14.0)
    engine.set("friction_linear", 0.075)
    engine.set("friction_quadratic", 0.00025)
    engine.set("coolant_mass", 7.0)
    engine.set("radiator_rate", 1100.0)
    engine.set("thermostat_open_c", 90.0)
    engine.set("fan_rate", 700.0)
    engine.set("max_temp_c", 125.0)
    engine.set("oil_mass", 5.5)
    engine.set("oil_cooler_rate", 320.0)
    engine.set("accessory_torque", 8.0)
    engine.set("injector_max_flow", 0.14)
    engine.set("fuel_tank_l", 70.0)
    engine.set("engine_mass", 230.0)
    engine.set("turbo_max_boost", 0.0)
    engine.set("turbo_wastegate", 0.0)
    engine.set("turbo_lag", 0.0)
    engine.set("turbo_efficiency", 0.0)
    engine.set("turbo_intercooler", 0.0)
    engine.set("clutch_capacity", 650.0)
    engine.set("clutch_stiffness", 80.0)
    engine.set("size", Vector3(0.72, 0.74, 0.60))
    engine.set("mount", Vector3(0.0, 0.45, -1.1))
    return engine


func _make_engine_1l6_turbo() -> Resource:
    var engine := _make_part("EngineResource")
    if engine == null:
        return null
    engine.set("part_id", "i3_1.6t")
    engine.set("part_name", "1.6 Turbo")
    engine.set("manufacturer", "Garage Works")
    engine.set("mass", 125.0)
    engine.set("price", 165000.0)
    engine.set("condition", 1.0)
    engine.set("displacement_l", 1.6)
    engine.set("cylinders", 3)
    engine.set("compression_ratio", 10.0)
    engine.set("plenum_volume_l", 1.8)
    engine.set("volumetric_efficiency", PackedFloat32Array([
        800.0, 0.72, 1600.0, 0.88, 2600.0, 0.96, 3600.0, 0.98,
        4600.0, 0.97, 5600.0, 0.93, 6600.0, 0.87, 7500.0, 0.75,
    ]))
    engine.set("aspiration", 1)
    engine.set("fuel", 1)
    engine.set("afr_target", 12.2)
    engine.set("idle_rpm", 900.0)
    engine.set("redline_rpm", 7500.0)
    engine.set("limiter_rpm", 7700.0)
    engine.set("inertia", 0.16)
    engine.set("friction_constant", 11.0)
    engine.set("friction_linear", 0.055)
    engine.set("friction_quadratic", 0.0002)
    engine.set("coolant_mass", 5.0)
    engine.set("radiator_rate", 800.0)
    engine.set("thermostat_open_c", 88.0)
    engine.set("fan_rate", 450.0)
    engine.set("max_temp_c", 125.0)
    engine.set("oil_mass", 4.0)
    engine.set("oil_cooler_rate", 220.0)
    engine.set("accessory_torque", 5.0)
    engine.set("injector_max_flow", 0.09)
    engine.set("fuel_tank_l", 45.0)
    engine.set("engine_mass", 125.0)
    engine.set("turbo_max_boost", 80000.0)
    engine.set("turbo_wastegate", 70000.0)
    engine.set("turbo_lag", 0.18)
    engine.set("turbo_efficiency", 0.74)
    engine.set("turbo_intercooler", 0.70)
    engine.set("clutch_capacity", 380.0)
    engine.set("clutch_stiffness", 55.0)
    engine.set("size", Vector3(0.58, 0.62, 0.50))
    engine.set("mount", Vector3(0.0, 0.45, -1.15))
    return engine


func _make_gearbox_6mt_rwd() -> Resource:
    var gearbox := _make_part("GearboxResource")
    if gearbox == null:
        return null
    gearbox.set("part_id", "6mt_rwd")
    gearbox.set("part_name", "6MT RWD")
    gearbox.set("manufacturer", "Garage Works")
    gearbox.set("mass", 48.0)
    gearbox.set("price", 120000.0)
    gearbox.set("condition", 1.0)
    gearbox.set("ratios", PackedFloat32Array([3.6, 2.2, 1.5, 1.0, 0.85, 0.7]))
    gearbox.set("reverse", -3.4)
    gearbox.set("final_drive", 3.9)
    gearbox.set("efficiency", 0.93)
    gearbox.set("shift_time", 0.28)
    gearbox.set("automatic", false)
    gearbox.set("upshift_rpm", 6000.0)
    gearbox.set("downshift_rpm", 2200.0)
    gearbox.set("kickdown_throttle", 0.85)
    # Предельный момент коробки должен переживать момент двигателя (~418 Н·м у V8 5.0).
    gearbox.set("max_torque", 520.0)
    gearbox.set("inertia", 0.02)
    gearbox.set("reverse_max_speed", 3.0)
    gearbox.set("converter", false)
    gearbox.set("converter_stall_ratio", 2.1)
    gearbox.set("converter_coupling_ratio", 0.86)
    gearbox.set("converter_capacity", 0.007)
    gearbox.set("converter_lockup", true)
    gearbox.set("converter_lockup_speed", 55.0)
    gearbox.set("converter_lockup_capacity", 480.0)
    gearbox.set("layout", DRIVE_RWD)
    gearbox.set("front_bias", 0.0)
    gearbox.set("size", Vector3(0.40, 0.45, 0.65))
    gearbox.set("mount", Vector3(0.0, 0.35, -0.2))
    return gearbox


func _make_gearbox_6at_awd() -> Resource:
    var gearbox := _make_part("GearboxResource")
    if gearbox == null:
        return null
    gearbox.set("part_id", "6at_awd")
    gearbox.set("part_name", "6AT AWD")
    gearbox.set("manufacturer", "Garage Works")
    gearbox.set("mass", 78.0)
    gearbox.set("price", 280000.0)
    gearbox.set("condition", 1.0)
    gearbox.set("ratios", PackedFloat32Array([3.4, 2.1, 1.5, 1.0, 0.75, 0.6]))
    gearbox.set("reverse", -3.8)
    gearbox.set("final_drive", 3.4)
    gearbox.set("efficiency", 0.90)
    gearbox.set("shift_time", 0.35)
    gearbox.set("automatic", true)
    gearbox.set("upshift_rpm", 5800.0)
    gearbox.set("downshift_rpm", 2000.0)
    gearbox.set("kickdown_throttle", 0.70)
    gearbox.set("max_torque", 800.0)
    gearbox.set("inertia", 0.03)
    gearbox.set("reverse_max_speed", 3.0)
    gearbox.set("converter", true)
    gearbox.set("converter_stall_ratio", 2.1)
    gearbox.set("converter_coupling_ratio", 0.86)
    gearbox.set("converter_capacity", 0.007)
    gearbox.set("converter_lockup", true)
    gearbox.set("converter_lockup_speed", 55.0)
    gearbox.set("converter_lockup_capacity", 480.0)
    gearbox.set("layout", DRIVE_AWD)
    gearbox.set("front_bias", 0.4)
    gearbox.set("size", Vector3(0.46, 0.50, 0.72))
    gearbox.set("mount", Vector3(0.0, 0.35, -0.1))
    return gearbox


func _make_tire_street() -> Resource:
    var tire := _make_part("TireResource")
    if tire == null:
        return null
    tire.set("part_id", "tire_street235")
    tire.set("part_name", "Street 235")
    tire.set("manufacturer", "Garage Works")
    tire.set("mass", 10.5)
    tire.set("price", 9000.0)
    tire.set("condition", 1.0)
    tire.set("friction", 1.0)
    tire.set("long_stiffness", 9.0)
    tire.set("long_shape", 1.60)
    tire.set("long_curvature", 0.60)
    tire.set("lat_stiffness", 7.5)
    tire.set("lat_shape", 1.40)
    tire.set("lat_curvature", 0.75)
    tire.set("load_sensitivity", 0.10)
    tire.set("reference_load", 3800.0)
    tire.set("relaxation_length", 0.38)
    tire.set("camber_stiffness", 0.0014)
    tire.set("rolling_resistance", 0.015)
    tire.set("pneumatic_trail", 0.050)
    tire.set("radius", 0.33)
    tire.set("width", 0.235)
    tire.set("tire_mass", 10.5)
    tire.set("wear_rate", 1.0)
    tire.set("optimal_temperature", 85.0)
    tire.set("load_index", 95)
    tire.set("speed_rating", 240.0)
    return tire


func _make_tire_semi_slick() -> Resource:
    var tire := _make_part("TireResource")
    if tire == null:
        return null
    tire.set("part_id", "tire_semi255")
    tire.set("part_name", "Sport Semi-Slick")
    tire.set("manufacturer", "Garage Works")
    tire.set("mass", 11.5)
    tire.set("price", 26000.0)
    tire.set("condition", 1.0)
    tire.set("friction", 1.25)
    tire.set("long_stiffness", 12.0)
    tire.set("long_shape", 1.65)
    tire.set("long_curvature", 0.60)
    tire.set("lat_stiffness", 10.0)
    tire.set("lat_shape", 1.45)
    tire.set("lat_curvature", 0.75)
    tire.set("load_sensitivity", 0.08)
    tire.set("reference_load", 4200.0)
    tire.set("relaxation_length", 0.30)
    tire.set("camber_stiffness", 0.0018)
    tire.set("rolling_resistance", 0.013)
    tire.set("pneumatic_trail", 0.045)
    tire.set("radius", 0.34)
    tire.set("width", 0.255)
    tire.set("tire_mass", 11.5)
    tire.set("wear_rate", 2.5)
    tire.set("optimal_temperature", 95.0)
    tire.set("load_index", 100)
    tire.set("speed_rating", 280.0)
    return tire


func _make_tire_track_slick() -> Resource:
    var tire := _make_part("TireResource")
    if tire == null:
        return null
    tire.set("part_id", "tire_slick275")
    tire.set("part_name", "Track Slick")
    tire.set("manufacturer", "Garage Works")
    tire.set("mass", 12.5)
    tire.set("price", 52000.0)
    tire.set("condition", 1.0)
    tire.set("friction", 1.45)
    tire.set("long_stiffness", 15.0)
    tire.set("long_shape", 1.70)
    tire.set("long_curvature", 0.55)
    tire.set("lat_stiffness", 12.5)
    tire.set("lat_shape", 1.50)
    tire.set("lat_curvature", 0.72)
    tire.set("load_sensitivity", 0.06)
    tire.set("reference_load", 4600.0)
    tire.set("relaxation_length", 0.24)
    tire.set("camber_stiffness", 0.0022)
    tire.set("rolling_resistance", 0.011)
    tire.set("pneumatic_trail", 0.035)
    tire.set("radius", 0.35)
    tire.set("width", 0.275)
    tire.set("tire_mass", 12.5)
    tire.set("wear_rate", 5.0)
    tire.set("optimal_temperature", 105.0)
    tire.set("load_index", 110)
    tire.set("speed_rating", 320.0)
    return tire


func _make_rim(diameter_in: float, width_in: float, offset_mm: float) -> Resource:
    var rim := _make_part("RimResource")
    if rim == null:
        return null
    rim.set("part_id", "rim_%d" % int(diameter_in))
    rim.set("part_name", "Диск %d×%.1f" % [int(diameter_in), width_in])
    rim.set("manufacturer", "Garage Works")
    rim.set("mass", 11.0)
    rim.set("price", 14000.0)
    rim.set("condition", 1.0)
    rim.set("diameter_in", diameter_in)
    rim.set("width_in", width_in)
    rim.set("offset_mm", offset_mm)
    rim.set("bolts", 5)
    rim.set("pcd_mm", 114)
    return rim


func _make_differential_open() -> Resource:
    var differential := _make_part("DifferentialResource")
    if differential == null:
        return null
    differential.set("part_id", "diff_open")
    differential.set("part_name", "Open")
    differential.set("manufacturer", "Garage Works")
    differential.set("mass", 12.0)
    differential.set("price", 15000.0)
    differential.set("condition", 1.0)
    differential.set("kind", 0)
    differential.set("preload", 0.0)
    differential.set("coupling", 0.0)
    differential.set("bias_ratio", 0.0)
    differential.set("max_torque", 500.0)
    return differential


func _make_differential_lsd() -> Resource:
    var differential := _make_part("DifferentialResource")
    if differential == null:
        return null
    differential.set("part_id", "diff_lsd")
    differential.set("part_name", "LSD")
    differential.set("manufacturer", "Garage Works")
    differential.set("mass", 14.0)
    differential.set("price", 45000.0)
    differential.set("condition", 1.0)
    differential.set("kind", 2)
    differential.set("preload", 45.0)
    differential.set("coupling", 280.0)
    differential.set("bias_ratio", 0.5)
    differential.set("max_torque", 700.0)
    return differential


func _make_differential_torsen() -> Resource:
    var differential := _make_part("DifferentialResource")
    if differential == null:
        return null
    differential.set("part_id", "diff_torsen")
    differential.set("part_name", "Torsen")
    differential.set("manufacturer", "Garage Works")
    differential.set("mass", 15.0)
    differential.set("price", 82000.0)
    differential.set("condition", 1.0)
    differential.set("kind", 3)
    differential.set("preload", 80.0)
    differential.set("coupling", 0.0)
    differential.set("bias_ratio", 0.65)
    differential.set("max_torque", 900.0)
    return differential


func _make_center_differential() -> Resource:
    var differential := _make_part("DifferentialResource")
    if differential == null:
        return null
    differential.set("part_id", "diff_center_lsd")
    differential.set("part_name", "Центральный LSD")
    differential.set("manufacturer", "Garage Works")
    differential.set("mass", 22.0)
    differential.set("price", 60000.0)
    differential.set("condition", 1.0)
    differential.set("kind", 2)
    differential.set("preload", 60.0)
    differential.set("coupling", 200.0)
    differential.set("bias_ratio", 0.4)
    differential.set("max_torque", 800.0)
    return differential


func _make_suspension_sport() -> Resource:
    var suspension := _make_part("SuspensionResource")
    if suspension == null:
        return null
    suspension.set("part_id", "susp_sport")
    suspension.set("part_name", "Sport")
    suspension.set("manufacturer", "Garage Works")
    suspension.set("mass", 32.0)
    suspension.set("price", 60000.0)
    suspension.set("condition", 1.0)
    suspension.set("kind", 0)
    suspension.set("rest_length", 0.34)
    suspension.set("max_length", 0.46)
    suspension.set("min_length", 0.12)
    suspension.set("spring_rate", 42000.0)
    suspension.set("damper_compression", 3600.0)
    suspension.set("damper_rebound", 5200.0)
    suspension.set("bump_stop_rate", 220000.0)
    suspension.set("anti_roll", 9000.0)
    suspension.set("camber_gain", -2.4)
    suspension.set("toe_gain", 0.0)
    suspension.set("camber_deg", -1.5)
    suspension.set("toe_deg", 0.08)
    suspension.set("max_steer_deg", 38.0)
    return suspension


func _make_suspension_double_wishbone() -> Resource:
    var suspension := _make_part("SuspensionResource")
    if suspension == null:
        return null
    suspension.set("part_id", "susp_dw")
    suspension.set("part_name", "Double Wishbone")
    suspension.set("manufacturer", "Garage Works")
    suspension.set("mass", 38.0)
    suspension.set("price", 95000.0)
    suspension.set("condition", 1.0)
    suspension.set("kind", 1)
    suspension.set("rest_length", 0.32)
    suspension.set("max_length", 0.44)
    suspension.set("min_length", 0.10)
    suspension.set("spring_rate", 48000.0)
    suspension.set("damper_compression", 4200.0)
    suspension.set("damper_rebound", 6000.0)
    suspension.set("bump_stop_rate", 250000.0)
    suspension.set("anti_roll", 12000.0)
    suspension.set("camber_gain", -3.0)
    suspension.set("toe_gain", 0.0)
    suspension.set("camber_deg", -2.0)
    suspension.set("toe_deg", 0.05)
    suspension.set("max_steer_deg", 40.0)
    return suspension


# ---------------------------------------------------------------------------
# Сборка
# ---------------------------------------------------------------------------

## Собирает `CarAssemblyResource` из выбранных деталей.
func _build_assembly() -> Resource:
    var assembly := _make_part("CarAssemblyResource")
    if assembly == null:
        return null

    assembly.set("assembly_name", _assembly_title())
    assembly.set("chassis", _chassis)
    assembly.set("engine", _selected_engine())
    assembly.set("gearbox", _effective_gearbox())
    assembly.set("differential", _selected_differential())
    # Центральный дифферендиал нужен только для полного привода; для RWD/FWD
    # поле сбрасывается в null (Option<Gd<...>> принимает пустое значение).
    assembly.set("center_differential", _effective_center_differential())
    assembly.set("body", _body)

    var wheel_fl := _build_wheel()
    var wheel_fr := _build_wheel()
    var wheel_rl := _build_wheel()
    var wheel_rr := _build_wheel()
    assembly.set("front_wheels", wheel_fl)
    assembly.set("rear_wheels", wheel_rl)
    assembly.set("wheel_fl", wheel_fl)
    assembly.set("wheel_fr", wheel_fr)
    assembly.set("wheel_rl", wheel_rl)
    assembly.set("wheel_rr", wheel_rr)
    return assembly


## Собирает колесо в сборе: подвеска + диск + шина + тормоз.
func _build_wheel() -> Resource:
    var wheel := _make_part("WheelResource")
    if wheel == null:
        return null
    wheel.set("suspension", _selected_suspension())
    wheel.set("rim", _selected_rim())
    wheel.set("tire", _selected_tire())
    wheel.set("brake", _brake)
    return wheel


## КПП с учётом выбранной схемы привода (копия, чтобы не менять каталог).
func _effective_gearbox() -> Resource:
    var source := _selected_gearbox()
    if source == null:
        return null
    var gearbox := _make_part("GearboxResource")
    if gearbox == null:
        return source
    for property_name: String in GEARBOX_PROPERTIES:
        gearbox.set(property_name, source.get(property_name))
    # Схема привода переопределяет штатную схему выбранной коробки передач.
    var layout: int = DRIVE_LAYOUTS[_selected_index(CAT_DRIVE)]
    gearbox.set("layout", layout)
    gearbox.set("front_bias", 0.4 if layout == DRIVE_AWD else 0.0)
    return gearbox


## Центральный дифференциал нужен только для полного привода.
func _effective_center_differential() -> Variant:
    if DRIVE_LAYOUTS[_selected_index(CAT_DRIVE)] == DRIVE_AWD:
        return _center_differential
    return null


# ---------------------------------------------------------------------------
# Выбор деталей
# ---------------------------------------------------------------------------

func _selected_index(category: int) -> int:
    return int(_selected.get(category, 0))


func _selected_resource(category: int) -> Resource:
    var parts := _parts_of(category)
    if parts.is_empty():
        return null
    var index: int = clampi(_selected_index(category), 0, parts.size() - 1)
    return parts[index]


func _selected_engine() -> Resource:
    return _selected_resource(CAT_ENGINE)


func _selected_gearbox() -> Resource:
    return _selected_resource(CAT_GEARBOX)


func _selected_tire() -> Resource:
    return _selected_resource(CAT_TIRE)


func _selected_rim() -> Resource:
    if _rims.is_empty():
        return null
    var index: int = clampi(_selected_index(CAT_TIRE), 0, _rims.size() - 1)
    return _rims[index]


func _selected_differential() -> Resource:
    return _selected_resource(CAT_DIFFERENTIAL)


func _selected_suspension() -> Resource:
    return _selected_resource(CAT_SUSPENSION)


## Список ресурсов выбранной категории (для «привода» — пусто).
func _parts_of(category: int) -> Array[Resource]:
    var empty: Array[Resource] = []
    match category:
        CAT_ENGINE:
            return _engines
        CAT_GEARBOX:
            return _gearboxes
        CAT_TIRE:
            return _tires
        CAT_DIFFERENTIAL:
            return _differentials
        CAT_SUSPENSION:
            return _suspensions
        _:
            return empty


# ---------------------------------------------------------------------------
# Обновление интерфейса
# ---------------------------------------------------------------------------

func _populate_categories() -> void:
    _category_list.clear()
    for title: String in CATEGORY_TITLES:
        _category_list.add_item(title)
    _category_list.select(_current_category)


func _refresh_all() -> void:
    _refresh_parts_list()
    _refresh_details()
    _refresh_info()


func _refresh_parts_list() -> void:
    _part_list.clear()
    if _current_category == CAT_DRIVE:
        for title: String in DRIVE_TITLES:
            _part_list.add_item(title)
    else:
        for part: Resource in _parts_of(_current_category):
            if part == null:
                continue
            _part_list.add_item("%s — %s ₽" % [
                str(part.get("part_name")),
                _fmt_num(float(part.get("price"))),
            ])
    if _part_list.item_count == 0:
        return
    var index: int = clampi(_selected_index(_current_category), 0, _part_list.item_count - 1)
    _part_list.select(index)


func _refresh_details() -> void:
    var lines: Array[String] = _describe_current()
    for index in range(_detail_labels.size()):
        var label := _detail_labels[index]
        if index < lines.size():
            label.text = lines[index]
            label.visible = true
        else:
            label.text = ""
            label.visible = false


func _refresh_info() -> void:
    var preview := _build_assembly()
    var mass := 0.0
    var issues: Array[String] = []
    if preview != null:
        mass = float(preview.call("total_mass"))
        var raw: Array = preview.call("validate")
        for message: Variant in raw:
            issues.append(str(message))

    var engine := _selected_engine()
    var power_kw := 0.0
    var torque := 0.0
    if engine != null:
        power_kw = float(engine.call("max_power_kw"))
        torque = float(engine.call("max_torque"))

    _info_label.text = "Масса: %s кг   •   Мощность: %.0f кВт (%.0f л.с.)   •   Момент: %.0f Н·м   •   Цена: %s ₽" % [
        _fmt_num(mass),
        power_kw,
        power_kw * 1.341,
        torque,
        _fmt_num(_total_price()),
    ]

    # Проблемы уже применённой сборки (машина) + проблемы черновой сборки.
    var car_issues: Array[String] = []
    if _car != null and _car.has_method("get_validation"):
        var raw_car: Array = _car.call("get_validation")
        for message: Variant in raw_car:
            car_issues.append(str(message))
    var all_issues: Array[String] = []
    all_issues.append_array(car_issues)
    for message: String in issues:
        if not car_issues.has(message):
            all_issues.append(message)
    _issues_label.text = "\n".join(all_issues)


## Полная стоимость выбранной сборки.
func _total_price() -> float:
    var total := 0.0
    if _chassis != null:
        total += float(_chassis.get("price"))
    if _body != null:
        total += float(_body.get("price"))
    var engine := _selected_engine()
    if engine != null:
        total += float(engine.get("price"))
    var gearbox := _selected_gearbox()
    if gearbox != null:
        total += float(gearbox.get("price"))
    var differential := _selected_differential()
    if differential != null:
        total += float(differential.get("price"))
    if DRIVE_LAYOUTS[_selected_index(CAT_DRIVE)] == DRIVE_AWD and _center_differential != null:
        total += float(_center_differential.get("price"))
    var suspension := _selected_suspension()
    var tire := _selected_tire()
    var rim := _selected_rim()
    if suspension != null:
        total += float(suspension.get("price")) * 4.0
    if tire != null:
        total += float(tire.get("price")) * 4.0
    if rim != null:
        total += float(rim.get("price")) * 4.0
    if _brake != null:
        total += float(_brake.get("price")) * 4.0
    return total


## Название сборки для меню.
func _assembly_title() -> String:
    var engine := _selected_engine()
    var gearbox := _selected_gearbox()
    var engine_name := "без двигателя"
    var gearbox_name := "без КПП"
    if engine != null:
        engine_name = str(engine.get("part_name"))
    if gearbox != null:
        gearbox_name = str(gearbox.get("part_name"))
    return "%s / %s / %s" % [ASSEMBLY_NAME, engine_name, gearbox_name]


# ---------------------------------------------------------------------------
# Описание детали
# ---------------------------------------------------------------------------

func _describe_current() -> Array[String]:
    var lines: Array[String] = []
    if _current_category == CAT_DRIVE:
        lines.append_array(_describe_drive())
        return lines
    var part := _selected_resource(_current_category)
    if part == null:
        lines.append("Деталь не выбрана.")
        return lines
    match _current_category:
        CAT_ENGINE:
            lines.append_array(_describe_engine(part))
        CAT_GEARBOX:
            lines.append_array(_describe_gearbox(part))
        CAT_TIRE:
            lines.append_array(_describe_tire(part))
        CAT_DIFFERENTIAL:
            lines.append_array(_describe_differential(part))
        CAT_SUSPENSION:
            lines.append_array(_describe_suspension(part))
        _:
            lines.append(str(part.get("part_name")))
    return lines


func _describe_engine(engine: Resource) -> Array[String]:
    var aspiration: int = int(engine.get("aspiration"))
    var fuel: int = int(engine.get("fuel"))
    var power_kw := float(engine.call("max_power_kw"))
    var lines: Array[String] = []
    lines.append("%s · %s" % [str(engine.get("part_name")), str(engine.get("manufacturer"))])
    lines.append("Цена: %s ₽   •   Масса: %.0f кг" % [
        _fmt_num(float(engine.get("price"))),
        float(engine.get("mass")),
    ])
    lines.append("Объём: %.1f л   •   Цилиндров: %d   •   Сжатие: %.1f" % [
        float(engine.get("displacement_l")),
        int(engine.get("cylinders")),
        float(engine.get("compression_ratio")),
    ])
    lines.append("Наддув: %s   •   Топливо: %s   •   AFR: %.1f" % [
        ASPIRATION_NAMES[clampi(aspiration, 0, ASPIRATION_NAMES.size() - 1)],
        FUEL_NAMES[clampi(fuel, 0, FUEL_NAMES.size() - 1)],
        float(engine.get("afr_target")),
    ])
    lines.append("Холостые: %.0f об/мин   •   Красная зона: %.0f об/мин   •   Отсечка: %.0f об/мин" % [
        float(engine.get("idle_rpm")),
        float(engine.get("redline_rpm")),
        float(engine.get("limiter_rpm")),
    ])
    lines.append("Расчётный момент: %.0f Н·м   •   Расчётная мощность: %.0f кВт (%.0f л.с.)" % [
        float(engine.call("max_torque")),
        power_kw,
        power_kw * 1.341,
    ])
    return lines


func _describe_gearbox(gearbox: Resource) -> Array[String]:
    var ratios: PackedFloat32Array = gearbox.get("ratios")
    var ratio_text := ""
    for index in range(ratios.size()):
        ratio_text += ("%.2f" % ratios[index])
        if index < ratios.size() - 1:
            ratio_text += " / "
    var layout: int = DRIVE_LAYOUTS[_selected_index(CAT_DRIVE)]
    var converter_text := "Без гидротрансформатора"
    if bool(gearbox.get("converter")):
        converter_text = "Гидротрансформатор: отсечка %.1f, блокировка с %.0f рад/с" % [
            float(gearbox.get("converter_stall_ratio")),
            float(gearbox.get("converter_lockup_speed")),
        ]
    var lines: Array[String] = []
    lines.append("%s · %s" % [str(gearbox.get("part_name")), str(gearbox.get("manufacturer"))])
    lines.append("Цена: %s ₽   •   Масса: %.0f кг" % [
        _fmt_num(float(gearbox.get("price"))),
        float(gearbox.get("mass")),
    ])
    lines.append("Передачи: %s" % ratio_text)
    lines.append("Главная пара: %.2f   •   Задний ход: %.2f   •   КПД: %.0f%%" % [
        float(gearbox.get("final_drive")),
        float(gearbox.get("reverse")),
        float(gearbox.get("efficiency")) * 100.0,
    ])
    lines.append("Тип: %s   •   Привод: %s   •   Момент: %.0f Н·м" % [
        "Автомат" if bool(gearbox.get("automatic")) else "Механика",
        DRIVE_TITLES[clampi(layout, 0, DRIVE_TITLES.size() - 1)],
        float(gearbox.get("max_torque")),
    ])
    lines.append("Время переключения: %.2f с   •   %s" % [
        float(gearbox.get("shift_time")),
        converter_text,
    ])
    return lines


func _describe_tire(tire: Resource) -> Array[String]:
    var lines: Array[String] = []
    lines.append("%s · %s" % [str(tire.get("part_name")), str(tire.get("manufacturer"))])
    lines.append("Цена: %s ₽   •   Масса: %.0f кг" % [
        _fmt_num(float(tire.get("price"))),
        float(tire.get("mass")),
    ])
    lines.append("Радиус: %.3f м   •   Ширина: %.0f мм   •   Сцепление: %.2f" % [
        float(tire.get("radius")),
        float(tire.get("width")) * 1000.0,
        float(tire.get("friction")),
    ])
    lines.append("Износ: %.2f   •   Оптимальная температура: %.0f °C" % [
        float(tire.get("wear_rate")),
        float(tire.get("optimal_temperature")),
    ])
    lines.append("Индекс нагрузки: %d   •   Индекс скорости: %.0f км/ч" % [
        int(tire.get("load_index")),
        float(tire.get("speed_rating")),
    ])
    lines.append("Сопротивление качению: %.4f   •   Длина релаксации: %.2f м" % [
        float(tire.get("rolling_resistance")),
        float(tire.get("relaxation_length")),
    ])
    return lines


func _describe_differential(differential: Resource) -> Array[String]:
    var kind: int = int(differential.get("kind"))
    var lines: Array[String] = []
    lines.append("%s · %s" % [
        str(differential.get("part_name")),
        str(differential.get("manufacturer")),
    ])
    lines.append("Цена: %s ₽   •   Масса: %.0f кг" % [
        _fmt_num(float(differential.get("price"))),
        float(differential.get("mass")),
    ])
    lines.append("Тип: %s" % DIFFERENTIAL_KIND_NAMES[clampi(kind, 0, DIFFERENTIAL_KIND_NAMES.size() - 1)])
    lines.append("Предельный момент: %.0f Н·м" % float(differential.get("max_torque")))
    lines.append("Преднатяжение: %.0f Н·м   •   Связь: %.0f Н·м·с/рад   •   Переброс: %.0f%%" % [
        float(differential.get("preload")),
        float(differential.get("coupling")),
        float(differential.get("bias_ratio")) * 100.0,
    ])
    return lines


func _describe_suspension(suspension: Resource) -> Array[String]:
    var kind: int = int(suspension.get("kind"))
    var lines: Array[String] = []
    lines.append("%s · %s" % [
        str(suspension.get("part_name")),
        str(suspension.get("manufacturer")),
    ])
    lines.append("Цена: %s ₽   •   Масса: %.0f кг" % [
        _fmt_num(float(suspension.get("price"))),
        float(suspension.get("mass")),
    ])
    lines.append("Схема: %s" % SUSPENSION_KIND_NAMES[clampi(kind, 0, SUSPENSION_KIND_NAMES.size() - 1)])
    lines.append("Жёсткость пружины: %.0f Н/м   •   Демпфер: %.0f / %.0f" % [
        float(suspension.get("spring_rate")),
        float(suspension.get("damper_compression")),
        float(suspension.get("damper_rebound")),
    ])
    lines.append("Стабилизатор: %.0f Н·м/рад   •   Отбойник: %.0f Н/м" % [
        float(suspension.get("anti_roll")),
        float(suspension.get("bump_stop_rate")),
    ])
    lines.append("Развал: %.1f°   •   Схождение: %.2f°   •   Поворот: %.0f°" % [
        float(suspension.get("camber_deg")),
        float(suspension.get("toe_deg")),
        float(suspension.get("max_steer_deg")),
    ])
    return lines


func _describe_drive() -> Array[String]:
    var layout: int = DRIVE_LAYOUTS[_selected_index(CAT_DRIVE)]
    var description := ""
    match layout:
        DRIVE_RWD:
            description = "Момент идёт на заднюю ось. Просто и надёжно, но склонно к переворотам."
        DRIVE_FWD:
            description = "Момент идёт на переднюю ось. Массовая и дешёвая, но снос в повороте."
        DRIVE_AWD:
            description = "Момент делится между осями. Ставится центральный LSD, доля передней оси 40%."
    var lines: Array[String] = []
    lines.append(DRIVE_TITLES[clampi(layout, 0, DRIVE_TITLES.size() - 1)])
    lines.append(description)
    lines.append("Схема привода переопределяет схему выбранной коробки передач.")
    return lines


# ---------------------------------------------------------------------------
# Обработчики
# ---------------------------------------------------------------------------

func _on_category_selected(index: int) -> void:
    _current_category = index
    _refresh_parts_list()
    _refresh_details()
    _refresh_info()


func _on_part_selected(index: int) -> void:
    _selected[_current_category] = index
    # Выбор коробки передач задаёт штатную схему привода, но её можно переопределить.
    if _current_category == CAT_GEARBOX:
        var gearbox := _selected_gearbox()
        if gearbox != null:
            _selected[CAT_DRIVE] = _drive_index_for_layout(int(gearbox.get("layout")))
    _refresh_parts_list()
    _refresh_details()
    _refresh_info()


## Индекс категории «Привод» для схемы привода коробки передач.
func _drive_index_for_layout(layout: int) -> int:
    for index in range(DRIVE_LAYOUTS.size()):
        if DRIVE_LAYOUTS[index] == layout:
            return index
    return 0


func _on_apply_pressed() -> void:
    if _car == null:
        _show_message("Машина не привязана: вызовите setup(car).")
        return
    var assembly := _build_assembly()
    if assembly == null:
        _show_message("Не удалось создать сборку.")
        return
    _car.set("assembly", assembly)
    _car.call("rebuild")
    # `is_ready()` у VbvCarBody затеняется унаследованным Node.is_ready(),
    # поэтому готовность симуляции проверяем по числу колёс.
    var wheels: int = int(_car.call("get_wheel_count"))
    if wheels > 0:
        _show_message("Сборка применена: %s" % _assembly_title())
    else:
        _show_message("Сборка несовместима — смотрите список проблем.")
    _refresh_info()


func _on_reset_pressed() -> void:
    if _car == null:
        _show_message("Машина не привязана: вызовите setup(car).")
        return
    if _original_assembly != null:
        _car.set("assembly", _original_assembly)
        _car.call("rebuild")
        _show_message("Исходная сборка восстановлена.")
    else:
        _show_message("Исходная сборка неизвестна.")
    _refresh_info()


func _on_close_pressed() -> void:
    close()


func _on_status_timeout() -> void:
    _status_label.modulate = Color(1.0, 1.0, 1.0, 0.0)


# ---------------------------------------------------------------------------
# Служебное
# ---------------------------------------------------------------------------

## Показывает временное сообщение под кнопками.
func _show_message(text: String) -> void:
    _status_label.text = text
    _status_label.modulate = Color(1.0, 1.0, 1.0, 1.0)
    if _status_timer != null:
        _status_timer.start(STATUS_TIMEOUT)


## Показывает меню — управление машиной при этом отключается.
func _suspend_car_input(suspend: bool) -> void:
    if _car == null or not _car.has_method("set_input_enabled"):
        return
    if suspend:
        if _car_input_suspended:
            return
        _prev_input_enabled = bool(_car.get("read_input"))
        _car.call("set_input_enabled", false)
        _car_input_suspended = true
    else:
        if not _car_input_suspended:
            return
        _car.call("set_input_enabled", _prev_input_enabled)
        _car_input_suspended = false


## Запоминает исходную сборку машины для кнопки «Сбросить».
func _load_original_assembly() -> void:
    _original_assembly = null
    if _car == null:
        return
    var current: Variant = _car.get("assembly")
    if current != null:
        _original_assembly = current
        return
    # Машина без сборки — берём готовый образец из каталога.
    var catalog: Object = ClassDB.instantiate("VehicleCatalog")
    if catalog != null:
        _original_assembly = catalog.call("sample_rwd_coupe")


## Форматирует число с разделителями тысяч.
func _fmt_num(value: float) -> String:
    var rounded: int = int(round(value))
    var digits: String = str(absi(rounded))
    var result := ""
    var counter := 0
    for index in range(digits.length() - 1, -1, -1):
        result = digits[index] + result
        counter += 1
        if counter % 3 == 0 and index > 0:
            result = " " + result
    if rounded < 0:
        return "-" + result
    return result
