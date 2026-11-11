class_name EngineAudio
extends Node

## Процедурный реалистичный звук двигателя автомобиля.
##
## Звук синтезируется в реальном времени, без единого аудиофайла:
##   * вспышки сгорания по цилиндрам (4-такт: f = rpm/60 * цилиндры/2) —
##     каждая вспышка порождает burst из 6 гармоник, затухающих по экспоненте,
##     со случайным разбросом амплитуды и длительности (живой звук, а не «синтезатор»);
##   * резонатор выпуска (band-pass по частоте вспышек, Q ~ 2..4) даёт
##     «полый» характер мотора, плюс фиксированный формант выпускной трубы;
##   * механическое гуление на частоте вращения коленвала;
##   * турбина: тонкий свист + шипящий шум, сброс давления (blow-off);
##   * вой редуктора, визг шин, шум дороги и ветра.
##
## Весь тракт проходит через мягкий лимитер, клиппинга не возникает.

const MIX_RATE := 44100.0          # частота дискретизации, Гц
const BUFFER_LENGTH := 0.25         # длина буфера генератора, с
const CHUNK_FRAMES := 1024         # порция кадров за один _process
const TWO_PI := 6.283185307179586

const NHARM := 6                    # гармоник в одной вспышке сгорания
const DECAY_BASE := 55.0            # базовая скорость затухания гармоник, 1/с
const DECAY_STEP := 38.0            # доп. затухание старших гармоник, 1/с
const ENV_CARRY := 0.2              # доля остатка burst, переходящая на след. вспышку

const TORQUE_REF := 260.0           # момент, соответствующий полной нагрузке, Н·м
const AMBIENT_PA := 101325.0        # атмосферное давление, Па
const BOOST_BAR_LIMIT := 0.25       # порог сброса давления, бар

# Веса слоёв в итоговой миксе
const MIX_RES_FIRE := 0.38          # резонатор по частоте вспышек
const MIX_RES_PIPE := 0.30          # формант выпускной трубы
const MIX_EXC_RAW := 0.14           # прямой сигнал вспышек (сохраняет гармоники)
const MIX_RUMBLE := 0.20            # механическое гуление коленвала


## Простой биквад-фильтр (band-pass / low-pass) на прямой форме I.
class Biquad extends RefCounted:
	const TAU2 := 6.283185307179586

	var b0: float = 0.0
	var b1: float = 0.0
	var b2: float = 0.0
	var a1: float = 0.0
	var a2: float = 0.0
	var x1: float = 0.0
	var x2: float = 0.0
	var y1: float = 0.0
	var y2: float = 0.0

	## Полосовой фильтр с пиковым усилением, равным добротности Q.
	func set_bandpass(freq: float, q: float, sr: float) -> void:
		var f: float = clamp(freq, 10.0, sr * 0.45)
		var w0: float = TAU2 * f / sr
		var alpha: float = sin(w0) / (2.0 * max(q, 0.1))
		var a0: float = 1.0 + alpha
		var cw: float = cos(w0)
		b0 = q * alpha / a0
		b1 = 0.0
		b2 = -q * alpha / a0
		a1 = -2.0 * cw / a0
		a2 = (1.0 - alpha) / a0

	## Фильтр нижних частот.
	func set_lowpass(freq: float, q: float, sr: float) -> void:
		var f: float = clamp(freq, 20.0, sr * 0.45)
		var w0: float = TAU2 * f / sr
		var alpha: float = sin(w0) / (2.0 * max(q, 0.1))
		var a0: float = 1.0 + alpha
		var cw: float = cos(w0)
		b0 = (1.0 - cw) * 0.5 / a0
		b1 = (1.0 - cw) / a0
		b2 = b0
		a1 = -2.0 * cw / a0
		a2 = (1.0 - alpha) / a0

	## Один отсчёт фильтра.
	func process(x: float) -> float:
		var y: float = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2
		x2 = x1
		x1 = x
		y2 = y1
		y1 = y
		return y


# --- Ссылка на машину и аудио -------------------------------------------------

var _car: Node = null
var _player: AudioStreamPlayer = null
var _generator: AudioStreamGenerator = null
var _playback: AudioStreamGeneratorPlayback = null

# --- Сглаженные параметры двигателя -------------------------------------------

var _cylinders: int = 4
var _redline: float = 6500.0
var _idle_rpm: float = 850.0
var _rpm_smooth: float = 0.0
var _idle_wander: float = 0.0        # блуждание оборотов на холостых
var _load: float = 0.0               # 0..1, нагрузка на двигатель
var _throttle: float = 0.0
var _prev_throttle: float = 0.0
var _boost_bar: float = 0.0
var _prev_boost: float = 0.0
var _speed: float = 0.0
var _gear: int = 1
var _running: bool = false
var _fuel_cut: bool = false
var _master: float = 0.0              # плавное затухание при остановке двигателя
var _last_peak: float = 0.0           # пик последней сгенерированной порции
var _flutter: float = 1.0             # медленное дрожание амплитуды
var _flutter_walk: float = 0.0
var _lope_depth: float = 0.0          # «лоп» на холостых (полуполный порядок)

# --- Фазы ----------------------------------------------------------------------

var _fire_phase: float = 0.0          # 0..1, фаза цикла сгорания
var _fire_jitter: float = 0.0         # накопленное смещение фазы (неточность цилиндров)
var _crank_phase: float = 0.0
var _lope_phase: float = 0.0
var _whistle_phase: float = 0.0
var _whine_phase: float = 0.0

# --- Burst сгорания -------------------------------------------------------------

var _env: PackedFloat32Array = PackedFloat32Array()   # текущие огибающие гармоник
var _dec: PackedFloat32Array = PackedFloat32Array()   # затухание огибающей за семпл

# --- Фильтры -------------------------------------------------------------------

var _res_fire: Biquad = Biquad.new()   # резонатор выпуска по частоте вспышек
var _res_pipe: Biquad = Biquad.new()   # формант выпускной трубы
var _lp_intake: Biquad = Biquad.new()  # ФНЧ шума впуска
var _lp_hiss: Biquad = Biquad.new()    # ФНЧ для выделения шипения
var _res_blow: Biquad = Biquad.new()   # резонатор сброса давления
var _res_tire: Biquad = Biquad.new()   # резонатор визга шин
var _lp_wind: Biquad = Biquad.new()    # ФНЧ шума дороги/ветра

# --- Состояние сброса давления --------------------------------------------------

var _blowoff: float = 0.0              # огибающая выброса
var _blowoff_dec: float = 1.0          # затухание огибающей за семпл


func _ready() -> void:
	if _car == null:
		set_process(false)
	elif _playback == null:
		_start_playback()


## Привязать генератор звука к автомобилю (узел класса VbvCarBody).
func setup(car: Node) -> void:
	_car = car
	_read_visual_spec()
	_create_audio()
	_start_playback()
	set_process(true)


## Публично остановить звук (вызывается перед выходом из игры).
##
## Важно: Godot удаляет playback из списка аудиосервера только на следующем
## микс-шаге (около 11 мс). Если сцена разбирается быстрее, остаётся
## предупреждение «ObjectDB instance was leaked at exit». Поэтому перед
## выходом звук останавливают заранее и дают серверу время освободить playback.
func stop() -> void:
	if _player != null and is_instance_valid(_player):
		_player.stop()
	_playback = null


func _exit_tree() -> void:
	# Аккуратно освобождаем аудио-ресурсы, иначе Godot ругается
	# «ObjectDB instance was leaked at exit»:AudioStreamGeneratorPlayback.
	# queue_free() на выходе из сцены не выполняется (дерево уже разбирается),
	# поэтому освобождаем плеер немедленно.
	_playback = null
	if _player != null and is_instance_valid(_player):
		_player.stop()
		# Godot удаляет playback из списка аудиосервера только на следующем
		# микс-шаге (~11 мс). Даём серверу его выполнить, иначе остаётся
		# предупреждение «ObjectDB instance was leaked at exit».
		OS.delay_msec(60)
		_player.stream = null
		_player.free()
	_generator = null
	_player = null
	_car = null


func _process(delta: float) -> void:
	if _playback == null:
		if _player != null and is_instance_valid(_player):
			_playback = _player.get_stream_playback()
		if _playback == null:
			return
	if not is_instance_valid(_playback):
		_playback = null
		return

	_update_controls(delta)

	# Если буфер переполнен — просто пропускаем порцию (звук не рвётся).
	if not _playback.can_push_buffer(CHUNK_FRAMES):
		return

	# PackedVector2Array — тип-значение, поэтому порция возвращается из метода.
	var frames := _render_chunk()
	if frames.is_empty():
		return
	if not _playback.push_buffer(frames):
		return


# --- Настройка -------------------------------------------------------------------

func _create_audio() -> void:
	if _player != null:
		return
	_generator = AudioStreamGenerator.new()
	_generator.mix_rate = MIX_RATE
	_generator.buffer_length = BUFFER_LENGTH
	_player = AudioStreamPlayer.new()
	_player.stream = _generator
	_player.volume_db = 0.0
	add_child(_player)
	_res_pipe.set_bandpass(190.0, 1.2, MIX_RATE)
	_lp_hiss.set_lowpass(2400.0, 0.7, MIX_RATE)
	_res_tire.set_bandpass(1800.0, 4.5, MIX_RATE)
	_lp_wind.set_lowpass(380.0, 0.7, MIX_RATE)
	# огибающая сброса давления: ~0.15 с до тишины
	_blowoff_dec = exp(-20.0 / MIX_RATE)
	for k in range(NHARM):
		_env.append(0.0)
		_dec.append(0.99)


func _start_playback() -> void:
	if _player == null or not is_instance_valid(_player):
		return
	if not is_inside_tree():
		# Воспроизведение разрешено только внутри дерева сцены — стартуем из _ready().
		return
	if _player.stream == null:
		_player.stream = _generator
	if not _player.playing:
		_player.play()
	_playback = _player.get_stream_playback()


## Снять характеристики мотора (число цилиндров, редлайн, холостые).
func _read_visual_spec() -> void:
	_cylinders = 4
	_redline = 6500.0
	_idle_rpm = 850.0
	if _car == null or not is_instance_valid(_car):
		return
	if not _car.has_method(&"get_visual_spec"):
		return
	var spec: Dictionary = _car.call(&"get_visual_spec")
	_cylinders = int(clamp(int(spec.get("cylinders", _cylinders)), 1, 12))
	_redline = float(clamp(float(spec.get("redline_rpm", _redline)), 1000.0, 20000.0))
	_idle_rpm = float(clamp(float(spec.get("idle_rpm", _idle_rpm)), 400.0, 2500.0))


# --- Обновление параметров -------------------------------------------------------

func _update_controls(delta: float) -> void:
	var dt: float = clamp(delta, 0.0, 0.1)
	var state: Dictionary = _read_state()
	var telem: Dictionary = _read_telemetry()

	_running = bool(state.get("running", false))
	_fuel_cut = bool(state.get("fuel_cut", false))
	var rpm: float = float(state.get("rpm", 0.0))
	_gear = int(state.get("gear", _gear))
	_speed = float(telem.get("speed", 0.0))

	# --- нагрузка: момент + газ + давление впуска ---
	var torque: float = float(state.get("engine_torque", 0.0))
	var torque_norm: float = clamp(abs(torque) / TORQUE_REF, 0.0, 1.0)
	var map_pa: float = float(state.get("manifold_pressure", AMBIENT_PA))
	var map_norm: float = clamp((map_pa / AMBIENT_PA - 0.8) / 0.8, 0.0, 1.0)
	var thr_raw: float = float(state.get("throttle", -1.0))
	if thr_raw < 0.0:
		_throttle = clamp(max(torque_norm, map_norm * 0.9), 0.0, 1.0)
	else:
		_throttle = clamp(thr_raw, 0.0, 1.0)
	_load = clamp(0.55 * torque_norm + 0.45 * _throttle, 0.0, 1.0)

	# --- наддув: в Rust давление в Па, приводим к барам ---
	var boost_raw: float = float(state.get("boost", 0.0))
	if boost_raw > 20.0:
		_boost_bar = boost_raw / 100000.0
	else:
		_boost_bar = boost_raw
	_boost_bar = clamp(_boost_bar, 0.0, 3.0)

	# --- обороты с небольшой нестабильностью на холостых ---
	var rpm_norm: float = clamp((rpm - _idle_rpm * 0.75) / max(_redline - _idle_rpm * 0.75, 500.0), 0.0, 1.0)
	var idle_weight: float = 1.0 - rpm_norm
	_idle_wander += randf_range(-1.0, 1.0) * dt * 26.0 * idle_weight
	_idle_wander *= exp(-dt * 0.9)
	_idle_wander = clamp(_idle_wander, -38.0, 38.0)
	var rpm_target: float = rpm + _idle_wander * idle_weight
	_rpm_smooth = lerp(_rpm_smooth, rpm_target, 1.0 - exp(-dt * 12.0))

	# --- дрожание амплитуды (flutter) ---
	_flutter_walk += randf_range(-1.0, 1.0) * dt * 5.0
	_flutter_walk = clamp(_flutter_walk, -1.0, 1.0)
	_flutter = 1.0 + _flutter_walk * (0.18 * _load * _load)

	# --- «полуполный порядок» на холостых ---
	_lope_depth = 0.22 * idle_weight

	# --- общее затухание при выключенном двигателе ---
	var master_target: float = 1.0 if _running else 0.0
	_master = lerp(_master, master_target, 1.0 - exp(-dt * 5.0))

	# --- сброс давления (blow-off) ---
	if _running and _prev_boost > BOOST_BAR_LIMIT and _prev_throttle > 0.45 \
			and _throttle < _prev_throttle - 0.25:
		_blowoff = 1.0
	_prev_throttle = _throttle
	_prev_boost = _boost_bar


func _read_state() -> Dictionary:
	if _car == null or not is_instance_valid(_car):
		return {}
	if not _car.has_method(&"get_engine_state"):
		return {}
	return _car.call(&"get_engine_state")


func _read_telemetry() -> Dictionary:
	if _car == null or not is_instance_valid(_car):
		return {}
	if not _car.has_method(&"get_telemetry"):
		return {}
	return _car.call(&"get_telemetry")


## Максимальный пробуксовщик среди колёс, стоящих на земле.
func _read_max_slip() -> float:
	var result: float = 0.0
	if _car == null or not is_instance_valid(_car):
		return result
	if not _car.has_method(&"get_wheel_info"):
		return result
	var wheels: Array = _car.call(&"get_wheel_info")
	for w in wheels:
		var wd: Dictionary = w
		if not bool(wd.get("contact", false)):
			continue
		result = max(result, abs(float(wd.get("slip", 0.0))))
	return result


# --- Синтез ----------------------------------------------------------------------

func _render_chunk() -> PackedVector2Array:
	_last_peak = 0.0
	var frames := PackedVector2Array()
	var rpm: float = _rpm_smooth
	var fire_hz: float = 0.0
	if _running and rpm > 120.0:
		fire_hz = clamp(rpm / 60.0 * (float(_cylinders) * 0.5), 4.0, 4000.0)
	var crank_hz: float = clamp(rpm / 60.0, 0.0, 400.0)

	# --- частоты и усиления слоёв ---
	var fire_step: float = fire_hz / MIX_RATE
	var crank_inc: float = TWO_PI * crank_hz / MIX_RATE
	var lope_inc: float = TWO_PI * (fire_hz * 0.5) / MIX_RATE

	var exc_gain: float = (0.26 + 0.34 * _load) * _flutter
	var rumble_gain: float = (0.22 + 0.5 * _load) * clamp(crank_hz / 25.0, 0.0, 1.0)

	var intake_gain: float = 0.45 * _throttle * (0.4 + 0.6 * _load)
	_lp_intake.set_lowpass(700.0 + 2200.0 * _load, 0.8, MIX_RATE)

	var whistle_hz: float = 1800.0 + 6000.0 * _boost_bar
	var whistle_gain: float = 0.16 * _boost_bar
	var hiss_gain: float = 0.20 * _boost_bar
	var whistle_inc: float = TWO_PI * whistle_hz / MIX_RATE

	# вой редуктора: на низкой частоте (нейтраль) тон убираем совсем
	var whine_raw: float = _speed / 0.33 * abs(float(_gear)) * 3.0
	var whine_gain: float = 0.0
	var whine_inc: float = 0.0
	if whine_raw > 60.0:
		whine_gain = 0.10 * clamp(_speed / 30.0, 0.0, 1.4)
		whine_inc = TWO_PI * whine_raw / MIX_RATE

	var squeal_gain: float = 0.0
	var slip: float = _read_max_slip()
	if slip > 0.01:
		squeal_gain = 0.80 * clamp(slip * 2.5, 0.0, 1.0)

	var wind_gain: float = clamp(0.0006 * _speed * _speed, 0.0, 0.5)

	# --- фильтры выпуска ---
	_res_fire.set_bandpass(max(fire_hz, 20.0), 2.0 + 1.6 * _load, MIX_RATE)

	# --- сброс давления ---
	if _blowoff > 0.001:
		var blow_hz: float = lerp(1100.0, 3200.0, float(clamp(_blowoff, 0.0, 1.0)))
		_res_blow.set_bandpass(blow_hz, 1.3, MIX_RATE)
	else:
		_res_blow.set_bandpass(2000.0, 1.3, MIX_RATE)

	for _i in range(CHUNK_FRAMES):
		# --- событие вспышки сгорания (переход фазы через ноль) ---
		_fire_phase += fire_step
		if _fire_phase >= 1.0:
			_fire_phase -= 1.0
			# неточность отдельных цилиндров: небольшой сдвиг фазы
			_fire_jitter = clamp(_fire_jitter + randf_range(-0.008, 0.008), -0.04, 0.04)
			_refresh_burst()

		# --- burst: сумма затухающих гармоник (1..6) ---
		var base: float = (_fire_phase + _fire_jitter) * TWO_PI
		var s1: float = sin(base)
		var c1: float = cos(base)
		var exc: float = 0.0
		var sk: float = s1
		var ck: float = c1
		for k in range(NHARM):
			var w: float = _env[k]
			exc += sk * w
			_env[k] = w * _dec[k]
			# рекурсия Чебышёва: гармоники без лишних вызовов sin
			var ns: float = sk * c1 + ck * s1
			var nc: float = ck * c1 - sk * s1
			sk = ns
			ck = nc

		# --- модуляция амплитуды («лоп» на холостых + flutter) ---
		_lope_phase += lope_inc
		if _lope_phase > TWO_PI:
			_lope_phase -= TWO_PI
		var lope: float = 1.0 + _lope_depth * sin(_lope_phase)
		var ex: float = exc * exc_gain * lope

		# --- резонатор выпуска и формант трубы ---
		var res_fire: float = _res_fire.process(ex)
		var res_pipe: float = _res_pipe.process(ex)

		# --- механическое гуление на частоте коленвала ---
		_crank_phase += crank_inc
		if _crank_phase > TWO_PI:
			_crank_phase -= TWO_PI
		var rumble: float = sin(_crank_phase) + 0.3 * sin(_crank_phase * 2.0)

		var engine_sig: float = res_fire * MIX_RES_FIRE + res_pipe * MIX_RES_PIPE \
				+ ex * MIX_EXC_RAW + rumble * MIX_RUMBLE * rumble_gain

		# --- шум впуска ---
		var intake: float = 0.0
		if intake_gain > 0.0:
			var n1: float = randf() * 2.0 - 1.0
			intake = _lp_intake.process(n1) * intake_gain

		# --- турбина: свист + шипение ---
		var turbo_sig: float = 0.0
		if _boost_bar > 0.001:
			_whistle_phase += whistle_inc
			if _whistle_phase > TWO_PI:
				_whistle_phase -= TWO_PI
			var n2: float = randf() * 2.0 - 1.0
			var hiss: float = (n2 - _lp_hiss.process(n2)) * hiss_gain
			turbo_sig = sin(_whistle_phase) * whistle_gain + hiss

		# --- сброс давления ---
		var blow_sig: float = 0.0
		if _blowoff > 0.001:
			var n3: float = randf() * 2.0 - 1.0
			_blowoff *= _blowoff_dec
			blow_sig = _res_blow.process(n3) * _blowoff * 0.45

		# --- вой редуктора ---
		var whine: float = 0.0
		if whine_gain > 0.0:
			_whine_phase += whine_inc
			if _whine_phase > TWO_PI:
				_whine_phase -= TWO_PI
			whine = (sin(_whine_phase) + 0.35 * sin(_whine_phase * 2.0)) * whine_gain

		# --- визг шин ---
		var squeal: float = 0.0
		if squeal_gain > 0.0:
			var n4: float = randf() * 2.0 - 1.0
			squeal = _res_tire.process(n4) * squeal_gain

		# --- шум дороги и ветра ---
		var wind: float = 0.0
		if wind_gain > 0.0:
			var n5: float = randf() * 2.0 - 1.0
			wind = _lp_wind.process(n5) * wind_gain

		var mix: float = engine_sig + intake + turbo_sig + blow_sig + whine + squeal + wind
		mix *= _master

		# --- мягкий лимитер (tanh-подобный), защита от клиппинга ---
		mix = clamp(mix, -3.0, 3.0)
		mix = mix * (27.0 + mix * mix) / (27.0 + 9.0 * mix * mix)
		mix *= 0.5

		_last_peak = maxf(_last_peak, absf(mix))
		frames.append(Vector2(mix, mix))

	return frames


## Диагностика: что сейчас генерируется (для отладки и тестов).
func get_debug_stats() -> Dictionary:
	var valid: bool = _playback != null and is_instance_valid(_playback)
	var avail: int = -1
	if valid:
		avail = _playback.get_frames_available()
	return {
		"master": _master,
		"running": _running,
		"rpm": _rpm_smooth,
		"load": _load,
		"throttle": _throttle,
		"playback_valid": valid,
		"frames_available": avail,
		"peak": _last_peak,
		"chunk": CHUNK_FRAMES,
		"mix_rate": _generator.mix_rate if _generator != null else 0.0,
		"buffer_length": _generator.buffer_length if _generator != null else 0.0,
		"volume_db": _player.volume_db if _player != null and is_instance_valid(_player) else 0.0,
	}


## Новая вспышка сгорания: burst из NHARM гармоник с экспоненциальным
## затуханием и случайным разбросом амплитуды и длительности.
func _refresh_burst() -> void:
	var dur: float = randf_range(0.62, 1.45)     # случайная длительность burst
	var amp: float = randf_range(0.78, 1.22)     # случайная амплитуда вспышки
	if _fuel_cut:
		# отсечка топлива даёт характерные «попухания» при сбросе газа
		amp *= randf_range(1.3, 2.0)
	for k in range(NHARM):
		var rate: float = (DECAY_BASE + float(k) * DECAY_STEP) / dur
		# огибающая гаснет экспоненциально: множитель за один семпл
		_dec[k] = exp(-rate / MIX_RATE)
		_env[k] = amp * randf_range(0.72, 1.28) * pow(float(k + 1), -0.8) * 0.55 \
				+ _env[k] * ENV_CARRY
