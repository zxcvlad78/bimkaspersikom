extends Node3D

## Тест звука: проверяем, что аудио-модуль реально генерирует сигнал
## и отдаёт кадры в воспроизведение.

var body: Node3D
var audio: Node
var elapsed := 0.0
var next_report := 0.5


func _ready() -> void:
    var catalog = ClassDB.instantiate("VehicleCatalog")
    var assembly = catalog.call("sample_awd_auto")
    body = ClassDB.instantiate("VbvCarBody")
    body.set("assembly", assembly)
    add_child(body)

    var ground := StaticBody3D.new()
    var shape := CollisionShape3D.new()
    var box := BoxShape3D.new()
    box.size = Vector3(400.0, 0.5, 400.0)
    shape.shape = box
    ground.add_child(shape)
    ground.position = Vector3(0.0, -0.25, 0.0)
    add_child(ground)

    audio = preload("res://demo/engine_audio.gd").new()
    add_child(audio)
    audio.call("setup", body)


func _process(delta: float) -> void:
    elapsed += delta
    if elapsed >= next_report:
        next_report += 0.5
        var s: Dictionary = audio.call("get_debug_stats")
        print("[snd] t=%.1f master=%.3f running=%s rpm=%.0f load=%.2f thr=%.2f peak=%.4f avail=%d valid=%s rate=%.0f vol=%.1f"
            % [elapsed, s.get("master"), s.get("running"), s.get("rpm"), s.get("load"),
               s.get("throttle"), s.get("peak"), s.get("frames_available"),
               s.get("playback_valid"), s.get("mix_rate"), s.get("volume_db")])
    if elapsed > 3.0:
        print("[snd] RESULT peak_overall=%.4f" % audio.call("get_debug_stats").get("peak"))
        get_tree().quit()
