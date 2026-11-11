extends Node3D

## Тест гаража: применяем сборку с другими деталями и проверяем, что машина
## осталась рабочей и едет.

var body: Node3D
var garage: Node
var elapsed := 0.0
var applied := false


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

    garage = preload("res://demo/garage.gd").new()
    add_child(garage)
    garage.call("setup", body)
    garage.call("open")
    print("[garage] opened")


func _physics_process(delta: float) -> void:
    if body == null:
        get_tree().quit(1)
        return
    body.call("set_controls", 1.0, 0.0, 0.0, 0.0)
    elapsed += delta

    if elapsed > 1.0 and not applied:
        applied = true
        # Выбираем деталь в каждой категории и применяем сборку.
        for cat in range(5):
            garage.call("_on_category_selected", cat)
            garage.call("_on_part_selected", 0)
        garage.call("_on_apply_pressed")
        print("[garage] applied: wheels=%d valid=%s" % [
            body.call("get_wheel_count"), str(body.call("get_validation"))])

    if elapsed > 5.0:
        var kmh = body.call("get_speed_kmh")
        print("[garage] RESULT speed_after_apply_kmh=%.1f" % kmh)
        get_tree().quit()
