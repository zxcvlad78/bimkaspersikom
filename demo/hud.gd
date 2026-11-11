class_name VehicleHud
extends CanvasLayer

## Приборная панель автомобиля (HUD).
##
## Показывает круглый тахометр со стрелкой, крупную цифру скорости, передачу,
## индикаторы двигателя / ESC / ручного тормоза, а также вспомогательную панель
## слева снизу (мощность, буст, температуры, топливо), индикатор перегрузки,
## подсказку по управлению и счётчик FPS.
##
## Использование:
## [codeblock]
## var hud := VehicleHud.new()
## add_child(hud)
## hud.setup(car)   # car — узел класса "VbvCarBody"
## [/codeblock]

# --- Оформление ----------------------------------------------------------------

## Тёмный полупрозрачный фон панелей.
const BG_COLOR := Color(0.04, 0.05, 0.07, 0.55)
## Тонкая рамка панелей.
const BORDER_COLOR := Color(0.30, 0.80, 0.92, 0.45)
## Основной акцент — циан.
const CYAN := Color(0.35, 0.85, 0.95)
## Второй акцент — оранжевый.
const ORANGE := Color(1.0, 0.55, 0.15)
## Зелёный — штатные состояния (двигатель, топливо).
const GREEN := Color(0.30, 0.90, 0.45)
## Янтарный — предупреждения (ESC/TCS).
const AMBER := Color(1.0, 0.72, 0.15)
## Красный — критичные состояния (redline, ручной тормоз).
const RED := Color(0.95, 0.25, 0.22)
## Обычный текст.
const TEXT := Color(0.88, 0.93, 0.97)
## Приглушённый текст (подписи).
const TEXT_DIM := Color(0.66, 0.72, 0.78, 0.9)

## Отступ панелей от краёв окна.
const MARGIN := 24.0
## Диаметр тахометра.
const TACHO_SIZE := 250.0
## Сторона квадрата индикатора перегрузки.
const GFORCE_SIZE := 122.0

## Скорости сглаживания значений, 1/с.
const SMOOTH_FAST := 14.0     # обороты и стрелка
const SMOOTH_MEDIUM := 7.0     # скорость, мощность
const SMOOTH_SLOW := 2.5       # температуры, уровни

## Значения redline по умолчанию, если сборка их не сообщила.
const DEFAULT_REDLINE := 7000.0

## Номинальный объём бака для полоски топлива (в колонках 55 и 70 л).
@export var fuel_tank_l: float = 70.0

## Подсказка по управлению.
const HELP_TEXT := """W / S   газ / тормоз
A / D   руль
SHIFT   ручной тормоз
Q       сцепление
E / R   передача вверх / вниз
F       зажигание
SPACE   стартер"""

# --- Данные --------------------------------------------------------------------

## Узел автомобиля ("VbvCarBody").
var _car: Node = null
## Красная зона тахометра, об/мин.
var _redline_rpm: float = DEFAULT_REDLINE
## Верхняя граница шкалы тахометра, об/мин.
var _dial_max: float = 8000.0

# Сглаженные значения (чтобы приборы не дёргались).
var _rpm_smooth: float = 0.0
var _speed_smooth: float = 0.0
var _power_smooth: float = 0.0
var _boost_smooth: float = 0.0
var _coolant_smooth: float = 20.0
var _oil_smooth: float = 20.0
var _fuel_smooth: float = 0.0
var _long_g: float = 0.0
var _lat_g: float = 0.0

# --- Узлы интерфейса -----------------------------------------------------------

var _root: Control = null
var _tacho: TachometerGauge = null
var _gforce: GForceMeter = null
var _panel_left: PanelContainer = null
var _lbl_gear: Label = null
var _lbl_speed: Label = null
var _lbl_power: Label = null
var _lbl_boost: Label = null
var _lbl_coolant: Label = null
var _lbl_oil: Label = null
var _lbl_fuel: Label = null
var _lbl_afr: Label = null
var _bar_fuel: ProgressBar = null
var _panel_help: PanelContainer = null
var _panel_fps: PanelContainer = null
var _lbl_fps: Label = null

var _fps_timer: float = 0.0
var _last_viewport_size: Vector2 = Vector2.ZERO


# --- Публичный API -------------------------------------------------------------

## Привязывает HUD к автомобилю и строит весь интерфейс.
## [param car] — узел класса "VbvCarBody" (или любой узел с теми же методами).
func setup(car: Node) -> void:
    _car = car
    _redline_rpm = _read_redline(car)
    # Верх шкалы: не меньше 8000 и на 15% выше красной зоны,
    # округляем вверх до кратного 1000, чтобы деления ложились ровно.
    _dial_max = ceilf(maxf(_redline_rpm * 1.15, 8000.0) / 1000.0) * 1000.0
    _build_ui()
    _apply_layout()
    # Размеры контейнеров уточняются после первого прохода layout — пересчитаем.
    call_deferred("_apply_layout")


## Текст передачи: -1 → "R", 0 → "N", иначе номер.
func gear_text(gear: int) -> String:
    if gear < 0:
        return "R"
    if gear == 0:
        return "N"
    return str(gear)


# --- Жизненный цикл ------------------------------------------------------------

func _process(delta: float) -> void:
    # Пересчитываем раскладку при изменении размера окна.
    var viewport: Viewport = get_viewport()
    if viewport != null:
        var viewport_size: Vector2 = viewport.get_visible_rect().size
        if viewport_size != _last_viewport_size:
            _last_viewport_size = viewport_size
            _apply_layout()
    _update_fps(delta)
    if _car == null or not is_instance_valid(_car):
        return
    _update_data(delta)


func _notification(what: int) -> void:
    if what == NOTIFICATION_WM_SIZE_CHANGED:
        _apply_layout()


# --- Сборка интерфейса ---------------------------------------------------------

func _build_ui() -> void:
    if _root != null and is_instance_valid(_root):
        return

    _root = Control.new()
    _root.name = "HudRoot"
    _root.set_anchors_preset(Control.PRESET_FULL_RECT)
    _root.mouse_filter = Control.MOUSE_FILTER_IGNORE
    add_child(_root)

    _build_tacho()
    _build_gforce()
    _build_left_panel()
    _build_help_panel()
    _build_fps()


func _build_tacho() -> void:
    _tacho = TachometerGauge.new()
    _tacho.name = "Tachometer"
    _tacho.custom_minimum_size = Vector2(TACHO_SIZE, TACHO_SIZE)
    _tacho.mouse_filter = Control.MOUSE_FILTER_IGNORE
    _tacho.dial_max = _dial_max
    _tacho.redline_rpm = _redline_rpm
    _root.add_child(_tacho)


func _build_gforce() -> void:
    _gforce = GForceMeter.new()
    _gforce.name = "GForceMeter"
    _gforce.custom_minimum_size = Vector2(GFORCE_SIZE, GFORCE_SIZE)
    _gforce.mouse_filter = Control.MOUSE_FILTER_IGNORE
    _root.add_child(_gforce)


func _build_left_panel() -> void:
    _panel_left = _make_panel()
    var box := VBoxContainer.new()
    box.add_theme_constant_override("separation", 8)
    box.mouse_filter = Control.MOUSE_FILTER_IGNORE
    _panel_left.add_child(box)

    # Передача и скорость крупным планом.
    var top := HBoxContainer.new()
    top.add_theme_constant_override("separation", 14)
    top.mouse_filter = Control.MOUSE_FILTER_IGNORE
    box.add_child(top)

    _lbl_gear = _make_label("N", 34, ORANGE)
    _lbl_gear.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
    top.add_child(_lbl_gear)

    _lbl_speed = _make_label("0 км/ч", 22, CYAN)
    _lbl_speed.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
    top.add_child(_lbl_speed)

    box.add_child(_make_separator())

    # Сетка технических показателей.
    var grid := GridContainer.new()
    grid.columns = 2
    grid.add_theme_constant_override("h_separation", 20)
    grid.add_theme_constant_override("v_separation", 6)
    grid.mouse_filter = Control.MOUSE_FILTER_IGNORE
    box.add_child(grid)

    _lbl_power = _add_stat(grid, "МОЩНОСТЬ", "0 кВт", CYAN)
    _lbl_boost = _add_stat(grid, "БУСТ", "0.00 бар", CYAN)
    _lbl_coolant = _add_stat(grid, "ВОДА", "20 °C", TEXT)
    _lbl_oil = _add_stat(grid, "МАСЛО", "20 °C", TEXT)
    _lbl_fuel = _add_fuel_stat(grid)
    _lbl_afr = _add_stat(grid, "СМЕСЬ", "—", TEXT_DIM)

    _root.add_child(_panel_left)


func _build_help_panel() -> void:
    _panel_help = _make_panel()
    var box := VBoxContainer.new()
    box.add_theme_constant_override("separation", 3)
    box.mouse_filter = Control.MOUSE_FILTER_IGNORE
    _panel_help.add_child(box)

    box.add_child(_make_label("УПРАВЛЕНИЕ", 12, CYAN))

    var body := _make_label(HELP_TEXT, 11, TEXT_DIM)
    body.add_theme_constant_override("line_spacing", 2)
    box.add_child(body)

    _root.add_child(_panel_help)


func _build_fps() -> void:
    _panel_fps = _make_panel()
    _lbl_fps = _make_label("FPS 60", 12, TEXT_DIM)
    _panel_fps.add_child(_lbl_fps)
    _root.add_child(_panel_fps)


# --- Раскладка -----------------------------------------------------------------

func _apply_layout() -> void:
    if _root == null or not is_instance_valid(_root):
        return
    if _tacho == null or _gforce == null or _panel_left == null:
        return
    if _panel_help == null or _panel_fps == null:
        return
    _root.set_anchors_preset(Control.PRESET_FULL_RECT)
    _root.offset_left = 0.0
    _root.offset_top = 0.0
    _root.offset_right = 0.0
    _root.offset_bottom = 0.0

    # Тахометр — правый нижний угол.
    _tacho.set_anchors_preset(Control.PRESET_BOTTOM_RIGHT)
    _tacho.offset_left = -TACHO_SIZE - MARGIN
    _tacho.offset_top = -TACHO_SIZE - MARGIN
    _tacho.offset_right = -MARGIN
    _tacho.offset_bottom = -MARGIN

    # Левая нижняя панель с показателями.
    var panel_size: Vector2 = _panel_left.get_combined_minimum_size()
    panel_size.x = maxf(panel_size.x, 240.0)
    panel_size.y = maxf(panel_size.y, 150.0)
    _panel_left.set_anchors_preset(Control.PRESET_BOTTOM_LEFT)
    _panel_left.offset_left = MARGIN
    _panel_left.offset_right = MARGIN + panel_size.x
    _panel_left.offset_top = -(panel_size.y + MARGIN)
    _panel_left.offset_bottom = -MARGIN

    # Индикатор перегрузки — прямо над левой панелью.
    _gforce.set_anchors_preset(Control.PRESET_BOTTOM_LEFT)
    _gforce.offset_left = MARGIN
    _gforce.offset_right = MARGIN + GFORCE_SIZE
    _gforce.offset_bottom = -(panel_size.y + MARGIN + 10.0)
    _gforce.offset_top = -(panel_size.y + MARGIN + 10.0 + GFORCE_SIZE)

    # Подсказка по управлению — левый верхний угол.
    var help_size: Vector2 = _panel_help.get_combined_minimum_size()
    _panel_help.set_anchors_preset(Control.PRESET_TOP_LEFT)
    _panel_help.offset_left = MARGIN
    _panel_help.offset_right = MARGIN + help_size.x
    _panel_help.offset_top = MARGIN
    _panel_help.offset_bottom = MARGIN + help_size.y

    # Счётчик FPS — правый верхний угол.
    var fps_size: Vector2 = _panel_fps.get_combined_minimum_size()
    _panel_fps.set_anchors_preset(Control.PRESET_TOP_RIGHT)
    _panel_fps.offset_left = -(fps_size.x + MARGIN)
    _panel_fps.offset_right = -MARGIN
    _panel_fps.offset_top = MARGIN
    _panel_fps.offset_bottom = MARGIN + fps_size.y


# --- Обновление данных ---------------------------------------------------------

func _update_data(delta: float) -> void:
    if _tacho == null or _gforce == null or _bar_fuel == null:
        return

    var telemetry: Dictionary = _read_dict(&"get_telemetry")
    var engine_state: Dictionary = _read_dict(&"get_engine_state")

    var speed_kmh: float = _pick_float(telemetry, "speed_kmh", 0.0)
    var rpm: float = _pick_float(engine_state, "rpm", -1.0)
    if rpm < 0.0:
        rpm = _call_float(&"get_rpm", 0.0)
    var gear: int = _pick_int(engine_state, "gear", -99)
    if gear == -99:
        gear = _call_int(&"get_gear", 0)
    var running: bool = _pick_bool(engine_state, "running", false)
    var power_kw: float = _pick_float(engine_state, "power_kw", 0.0)
    var boost_pa: float = _pick_float(engine_state, "boost", 0.0)
    var coolant: float = _pick_float(engine_state, "coolant_temp", 20.0)
    var oil: float = _pick_float(engine_state, "oil_temp", 20.0)
    var fuel_l: float = _pick_float(engine_state, "fuel_l", 0.0)
    var afr: float = _pick_float(engine_state, "afr", 0.0)
    var esc_active: bool = _pick_bool(telemetry, "esc_active", false)
    var tcs_active: bool = _pick_bool(engine_state, "tcs_active", false)
    var longitudinal_g: float = _pick_float(telemetry, "longitudinal_g", 0.0)
    var lateral_g: float = _pick_float(telemetry, "lateral_g", 0.0)

    # Сглаживание значений.
    _rpm_smooth = _smooth(_rpm_smooth, clampf(rpm, 0.0, _dial_max), SMOOTH_FAST, delta)
    _speed_smooth = _smooth(_speed_smooth, maxf(speed_kmh, 0.0), SMOOTH_MEDIUM, delta)
    _power_smooth = _smooth(_power_smooth, maxf(power_kw, 0.0), SMOOTH_MEDIUM, delta)
    _boost_smooth = _smooth(_boost_smooth, maxf(boost_pa, 0.0), SMOOTH_MEDIUM, delta)
    _coolant_smooth = _smooth(_coolant_smooth, coolant, SMOOTH_SLOW, delta)
    _oil_smooth = _smooth(_oil_smooth, oil, SMOOTH_SLOW, delta)
    _fuel_smooth = _smooth(_fuel_smooth, maxf(fuel_l, 0.0), SMOOTH_SLOW, delta)
    _long_g = _smooth(_long_g, longitudinal_g, SMOOTH_MEDIUM, delta)
    _lat_g = _smooth(_lat_g, lateral_g, SMOOTH_MEDIUM, delta)

    # Тахометр.
    _tacho.rpm = _rpm_smooth
    _tacho.speed_kmh = _speed_smooth
    _tacho.gear_text = gear_text(gear)
    _tacho.engine_running = running
    _tacho.esc_active = esc_active
    _tacho.tcs_active = tcs_active
    _tacho.handbrake = _is_action_pressed(&"vehicle_handbrake")
    _tacho.queue_redraw()

    # Индикатор перегрузки.
    _gforce.update_forces(_long_g, _lat_g)

    # Левая панель.
    _lbl_gear.text = gear_text(gear)
    _lbl_speed.text = "%d км/ч" % int(round(_speed_smooth))
    _lbl_power.text = "%d кВт" % int(round(_power_smooth))
    _lbl_boost.text = "%.2f бар" % (_boost_smooth / 100000.0)
    _lbl_coolant.text = "%d °C" % int(round(_coolant_smooth))
    _lbl_oil.text = "%d °C" % int(round(_oil_smooth))
    _lbl_fuel.text = "%.1f л" % _fuel_smooth
    if afr <= 0.0:
        _lbl_afr.text = "—"
    else:
        _lbl_afr.text = "%.1f" % afr

    # Цветовые предупреждения температур.
    _lbl_coolant.add_theme_color_override("font_color", RED if _coolant_smooth > 110.0 else TEXT)
    _lbl_oil.add_theme_color_override("font_color", RED if _oil_smooth > 125.0 else TEXT)

    # Полоска топлива.
    var fuel_percent: float = clampf(_fuel_smooth / maxf(fuel_tank_l, 1.0) * 100.0, 0.0, 100.0)
    _bar_fuel.value = fuel_percent
    var fill_style: StyleBoxFlat = _bar_fuel.get_theme_stylebox("fill") as StyleBoxFlat
    if fill_style != null:
        fill_style.bg_color = RED if fuel_percent < 10.0 else (AMBER if fuel_percent < 25.0 else GREEN)


func _update_fps(delta: float) -> void:
    if _lbl_fps == null or not is_instance_valid(_lbl_fps):
        return
    _fps_timer += delta
    if _fps_timer < 0.25:
        return
    _fps_timer = 0.0
    _lbl_fps.text = "FPS %d" % Engine.get_frames_per_second()


# --- Вспомогательное -----------------------------------------------------------

## Экспоненциальное сглаживание значения.
func _smooth(current: float, target: float, rate: float, delta: float) -> float:
    return lerpf(current, target, 1.0 - exp(-rate * delta))


func _read_redline(target: Node) -> float:
    if target == null or not is_instance_valid(target):
        return DEFAULT_REDLINE
    if not target.has_method("get_visual_spec"):
        return DEFAULT_REDLINE
    var spec: Variant = target.call(&"get_visual_spec")
    if spec is Dictionary:
        var data: Dictionary = spec
        if data.has("redline_rpm"):
            var value: float = float(data["redline_rpm"])
            if value > 100.0:
                return value
    return DEFAULT_REDLINE


func _read_dict(method: StringName) -> Dictionary:
    if _car == null or not is_instance_valid(_car):
        return {}
    if not _car.has_method(method):
        return {}
    var value: Variant = _car.call(method)
    if value is Dictionary:
        return value
    return {}


func _call_float(method: StringName, fallback: float) -> float:
    if _car == null or not is_instance_valid(_car):
        return fallback
    if not _car.has_method(method):
        return fallback
    var value: Variant = _car.call(method)
    if value is float or value is int:
        return float(value)
    return fallback


func _call_int(method: StringName, fallback: int) -> int:
    if _car == null or not is_instance_valid(_car):
        return fallback
    if not _car.has_method(method):
        return fallback
    var value: Variant = _car.call(method)
    if value is int or value is float:
        return int(value)
    return fallback


func _pick_float(data: Dictionary, key: String, fallback: float) -> float:
    if data.has(key):
        var value: Variant = data[key]
        if value is float or value is int:
            return float(value)
    return fallback


func _pick_int(data: Dictionary, key: String, fallback: int) -> int:
    if data.has(key):
        var value: Variant = data[key]
        if value is int or value is float:
            return int(value)
    return fallback


func _pick_bool(data: Dictionary, key: String, fallback: bool) -> bool:
    if data.has(key):
        var value: Variant = data[key]
        if value is bool:
            return value
        if value is int or value is float:
            return float(value) != 0.0
    return fallback


func _is_action_pressed(action: StringName) -> bool:
    if not InputMap.has_action(action):
        return false
    return Input.is_action_pressed(action)


func _make_label(text: String, font_size: int, color: Color) -> Label:
    var label := Label.new()
    label.text = text
    label.add_theme_font_size_override("font_size", font_size)
    label.add_theme_color_override("font_color", color)
    label.mouse_filter = Control.MOUSE_FILTER_IGNORE
    return label


func _make_panel() -> PanelContainer:
    var panel := PanelContainer.new()
    var style := StyleBoxFlat.new()
    style.bg_color = BG_COLOR
    style.border_color = BORDER_COLOR
    style.set_border_width_all(1)
    style.set_corner_radius_all(8)
    style.content_margin_left = 12.0
    style.content_margin_right = 12.0
    style.content_margin_top = 10.0
    style.content_margin_bottom = 10.0
    panel.add_theme_stylebox_override("panel", style)
    panel.mouse_filter = Control.MOUSE_FILTER_IGNORE
    return panel


func _make_separator() -> ColorRect:
    var line := ColorRect.new()
    line.color = Color(0.30, 0.80, 0.92, 0.22)
    line.custom_minimum_size = Vector2(0.0, 1.0)
    line.mouse_filter = Control.MOUSE_FILTER_IGNORE
    return line


## Добавляет ячейку «подпись + значение» в сетку показателей.
func _add_stat(grid: GridContainer, caption: String, value: String, color: Color) -> Label:
    var cell := VBoxContainer.new()
    cell.add_theme_constant_override("separation", 0)
    cell.mouse_filter = Control.MOUSE_FILTER_IGNORE
    cell.add_child(_make_label(caption, 10, TEXT_DIM))
    var value_label: Label = _make_label(value, 15, color)
    value_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
    cell.add_child(value_label)
    grid.add_child(cell)
    return value_label


## Ячейка показателя топлива: значение и полоска уровня.
func _add_fuel_stat(grid: GridContainer) -> Label:
    var cell := VBoxContainer.new()
    cell.add_theme_constant_override("separation", 2)
    cell.mouse_filter = Control.MOUSE_FILTER_IGNORE
    cell.add_child(_make_label("ТОПЛИВО", 10, TEXT_DIM))
    var value_label: Label = _make_label("0.0 л", 15, GREEN)
    value_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
    cell.add_child(value_label)

    _bar_fuel = ProgressBar.new()
    _bar_fuel.custom_minimum_size = Vector2(96.0, 6.0)
    _bar_fuel.min_value = 0.0
    _bar_fuel.max_value = 100.0
    _bar_fuel.value = 0.0
    _bar_fuel.step = 0.1
    _bar_fuel.show_percentage = false
    _bar_fuel.mouse_filter = Control.MOUSE_FILTER_IGNORE
    var background := StyleBoxFlat.new()
    background.bg_color = Color(0.12, 0.15, 0.18, 0.85)
    background.set_corner_radius_all(3)
    _bar_fuel.add_theme_stylebox_override("background", background)
    var fill := StyleBoxFlat.new()
    fill.bg_color = GREEN
    fill.set_corner_radius_all(3)
    _bar_fuel.add_theme_stylebox_override("fill", fill)
    cell.add_child(_bar_fuel)

    grid.add_child(cell)
    return value_label


# --- Внутренние виджеты --------------------------------------------------------

## Круглый тахометр: дуга шкалы, деления, красная зона, стрелка и табло по центру.
##
## Виджет самодостаточен: своя палитра, встроенный шрифт Godot,
## никаких внешних ресурсов.
class TachometerGauge extends Control:
    ## Начало дуги шкалы, градусы (0 — вправо, дальше по часовой стрелке).
    const START_ANGLE := 135.0
    ## Размах дуги шкалы, градусы.
    const SWEEP := 270.0
    ## Шаг мелких делений, об/мин.
    const MINOR_STEP := 500.0
    ## Шаг крупных делений с цифрами, об/мин.
    const MAJOR_STEP := 1000

    # Палитра виджета.
    const C_FACE := Color(0.04, 0.05, 0.07, 0.55)
    const C_RING := Color(0.28, 0.32, 0.38, 0.85)
    const C_TICK := Color(0.75, 0.82, 0.88, 0.95)
    const C_RED := Color(0.95, 0.25, 0.22)
    const C_ORANGE := Color(1.0, 0.55, 0.15)
    const C_GREEN := Color(0.30, 0.90, 0.45)
    const C_AMBER := Color(1.0, 0.72, 0.15)
    const C_TEXT := Color(0.88, 0.93, 0.97)
    const C_DIM := Color(0.66, 0.72, 0.78, 0.9)
    const C_LAMP_OFF := Color(0.22, 0.25, 0.30, 0.9)
    const C_CYAN := Color(0.30, 0.80, 0.92, 0.8)

    ## Верх шкалы, об/мин.
    var dial_max: float = 8000.0
    ## Начало красной зоны, об/мин.
    var redline_rpm: float = 7000.0
    ## Текущие обороты (уже сглаженные).
    var rpm: float = 0.0
    ## Текущая скорость, км/ч.
    var speed_kmh: float = 0.0
    ## Текущая передача в виде текста ("R", "N", "1"...).
    var gear_text: String = "N"
    ## Работает ли двигатель.
    var engine_running: bool = false
    ## Активна ли система стабилизации.
    var esc_active: bool = false
    ## Активна ли система контроля тяги.
    var tcs_active: bool = false
    ## Затянут ли ручной тормоз.
    var handbrake: bool = false

    var _font: Font = null
    var _readout_style: StyleBoxFlat = null

    func _draw() -> void:
        var font: Font = _get_font()
        var center: Vector2 = size * 0.5
        var radius: float = minf(size.x, size.y) * 0.5 - 18.0
        if radius <= 20.0:
            return
        var start_angle: float = deg_to_rad(START_ANGLE)
        var sweep: float = deg_to_rad(SWEEP)
        var end_angle: float = start_angle + sweep

        # Корпус циферблата.
        draw_circle(center, radius + 8.0, C_FACE)
        draw_arc(center, radius, start_angle, end_angle, 96, C_RING, 3.0, true)

        # Красная зона — от redline до конца шкалы.
        if redline_rpm < dial_max:
            var red_start: float = start_angle + sweep * (redline_rpm / dial_max)
            draw_arc(center, radius, red_start, end_angle, 48, C_RED, 7.0, true)

        _draw_scale(font, center, radius, start_angle, sweep)
        _draw_needle(center, radius, start_angle, sweep)
        _draw_readout(font, center)
        _draw_lamps(font, center)

    ## Деления и цифры шкалы.
    func _draw_scale(font: Font, center: Vector2, radius: float, start_angle: float, sweep: float) -> void:
        var value: float = 0.0
        while value <= dial_max + 0.5:
            var ratio: float = value / dial_max
            var angle: float = start_angle + sweep * ratio
            var direction := Vector2(cos(angle), sin(angle))
            var is_major: bool = int(round(value)) % MAJOR_STEP == 0
            var length: float = 15.0 if is_major else 7.0
            var width: float = 2.6 if is_major else 1.4
            var color: Color = C_RED if value >= redline_rpm else C_TICK
            draw_line(
                center + direction * radius,
                center + direction * (radius - length),
                color, width, true
            )
            if is_major:
                var label_point: Vector2 = center + direction * (radius - 31.0)
                var text: String = str(int(round(value / 1000.0)))
                var text_size: Vector2 = font.get_string_size(text, HORIZONTAL_ALIGNMENT_LEFT, -1, 13)
                var baseline := Vector2(
                    label_point.x - text_size.x * 0.5,
                    label_point.y + font.get_ascent(13) * 0.5 - text_size.y * 0.5
                )
                draw_string(font, baseline, text, HORIZONTAL_ALIGNMENT_LEFT, -1, 13, C_TEXT)
            value += MINOR_STEP

    ## Стрелка тахометра.
    func _draw_needle(center: Vector2, radius: float, start_angle: float, sweep: float) -> void:
        var ratio: float = clampf(rpm / dial_max, 0.0, 1.0)
        var angle: float = start_angle + sweep * ratio
        var direction := Vector2(cos(angle), sin(angle))
        var side := Vector2(-direction.y, direction.x)
        var tip: Vector2 = center + direction * (radius - 7.0)
        var base: Vector2 = center - direction * (radius * 0.10)
        var half_width: float = 3.6
        var color: Color = C_RED if rpm >= redline_rpm else C_ORANGE
        draw_colored_polygon(
            PackedVector2Array([tip, base + side * half_width, base - side * half_width]),
            color
        )

    ## Цифровое табло в центре: скорость, единицы измерения и передача.
    func _draw_readout(font: Font, center: Vector2) -> void:
        # Подложка цифрового табло.
        var box_size := Vector2(96.0, 70.0)
        var box_rect := Rect2(center.x - box_size.x * 0.5, center.y + 9.0, box_size.x, box_size.y)
        if _readout_style == null:
            _readout_style = StyleBoxFlat.new()
            _readout_style.bg_color = Color(0.04, 0.05, 0.07, 0.82)
            _readout_style.border_color = Color(0.30, 0.80, 0.92, 0.35)
            _readout_style.set_border_width_all(1)
            _readout_style.set_corner_radius_all(8)
        draw_style_box(_readout_style, box_rect)

        # Скорость крупными цифрами.
        var speed_text: String = str(int(round(speed_kmh)))
        var speed_size: Vector2 = font.get_string_size(speed_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 34)
        draw_string(
            font,
            Vector2(center.x - speed_size.x * 0.5, box_rect.position.y + 30.0),
            speed_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 34, C_TEXT
        )

        # Единицы измерения.
        var unit_text := "км/ч"
        var unit_size: Vector2 = font.get_string_size(unit_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 11)
        draw_string(
            font,
            Vector2(center.x - unit_size.x * 0.5, box_rect.position.y + 44.0),
            unit_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 11, C_DIM
        )

        # Передача под скоростью.
        var gear_size: Vector2 = font.get_string_size(gear_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 26)
        draw_string(
            font,
            Vector2(center.x - gear_size.x * 0.5, box_rect.position.y + 64.0),
            gear_text, HORIZONTAL_ALIGNMENT_LEFT, -1, 26, C_ORANGE
        )

        # Ось стрелки поверх подложки.
        draw_circle(center, 6.0, Color(0.08, 0.09, 0.11, 0.95))
        draw_arc(center, 6.0, 0.0, TAU, 24, C_CYAN, 1.6, true)

    ## Индикаторы двигателя, ESC/TCS и ручного тормоза.
    func _draw_lamps(font: Font, center: Vector2) -> void:
        var lamp_y: float = center.y - 46.0
        var items: Array = [
            [engine_running, C_GREEN, "ДВГ"],
            [esc_active or tcs_active, C_AMBER, "ESC"],
            [handbrake, C_RED, "P"],
        ]
        for i in range(items.size()):
            var item: Array = items[i]
            var active: bool = bool(item[0])
            var color: Color = item[1]
            var caption: String = item[2]
            var x: float = center.x + (float(i) - 1.0) * 30.0
            var lamp_point := Vector2(x, lamp_y)
            draw_circle(lamp_point, 4.5, color if active else C_LAMP_OFF)
            if active:
                draw_arc(
                    lamp_point, 7.0, 0.0, TAU, 16,
                    Color(color.r, color.g, color.b, 0.45), 1.2, true
                )
            var caption_size: Vector2 = font.get_string_size(caption, HORIZONTAL_ALIGNMENT_LEFT, -1, 9)
            draw_string(
                font,
                Vector2(x - caption_size.x * 0.5, lamp_y + 16.0),
                caption, HORIZONTAL_ALIGNMENT_LEFT, -1, 9, C_DIM
            )

    ## Встроенный шрифт Godot.
    func _get_font() -> Font:
        if _font == null:
            _font = ThemeDB.fallback_font
        return _font


## Квадратный индикатор перегрузки: точка показывает продольное и боковое ускорение.
class GForceMeter extends Control:
    ## Предел шкалы, g.
    const G_LIMIT := 1.5
    ## Длина следа точки.
    const TRAIL_MAX := 16

    # Палитра виджета.
    const C_FACE := Color(0.09, 0.12, 0.15, 0.55)
    const C_BORDER := Color(0.30, 0.80, 0.92, 0.45)
    const C_AXIS := Color(0.35, 0.42, 0.48, 0.55)
    const C_RING := Color(0.30, 0.36, 0.42, 0.45)
    const C_ORANGE := Color(1.0, 0.55, 0.15)

    ## Продольное ускорение, g.
    var longitudinal_g: float = 0.0
    ## Боковое ускорение, g.
    var lateral_g: float = 0.0

    var _trail: PackedVector2Array = PackedVector2Array()

    ## Обновляет показания и добавляет точку в след.
    func update_forces(longitudinal: float, lateral: float) -> void:
        longitudinal_g = longitudinal
        lateral_g = lateral
        var center: Vector2 = size * 0.5
        var half: float = minf(size.x, size.y) * 0.5 - 8.0
        if half > 4.0:
            _trail.append(_point(center, half))
            while _trail.size() > TRAIL_MAX:
                _trail.remove_at(0)
        queue_redraw()

    func _draw() -> void:
        var center: Vector2 = size * 0.5
        var half: float = minf(size.x, size.y) * 0.5 - 8.0
        if half <= 4.0:
            return

        # Рамка и оси.
        var rect := Rect2(center - Vector2(half, half), Vector2(half * 2.0, half * 2.0))
        draw_rect(rect, C_FACE, true)
        draw_rect(rect, C_BORDER, false, 1.0)
        draw_line(center - Vector2(half, 0.0), center + Vector2(half, 0.0), C_AXIS, 1.0)
        draw_line(center - Vector2(0.0, half), center + Vector2(0.0, half), C_AXIS, 1.0)
        for ring_step in [0.5, 1.0]:
            var ring: float = half * (float(ring_step) / G_LIMIT)
            draw_arc(center, ring, 0.0, TAU, 32, C_RING, 1.0, true)

        # След точки.
        var count: int = _trail.size()
        for i in range(count - 1):
            var fade: float = float(i) / float(maxi(count - 1, 1))
            draw_line(_trail[i], _trail[i + 1], Color(1.0, 0.55, 0.15, 0.5 * fade), 2.0, true)

        # Точка текущего ускорения.
        var point: Vector2 = _point(center, half)
        draw_circle(point, 4.0, C_ORANGE)
        draw_arc(point, 7.0, 0.0, TAU, 20, Color(1.0, 0.55, 0.15, 0.5), 1.2, true)

    ## Положение точки перегрузки внутри квадрата.
    func _point(center: Vector2, half: float) -> Vector2:
        var x: float = clampf(lateral_g / G_LIMIT, -1.0, 1.0) * half
        var y: float = -clampf(longitudinal_g / G_LIMIT, -1.0, 1.0) * half
        return center + Vector2(x, y)
