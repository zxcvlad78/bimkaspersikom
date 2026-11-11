extends EditorPlugin

## Регистрирует input actions для управления автомобилем.

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
]

const ACTION_KEY := {
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
}


func _enter_tree() -> void:
    for action in ACTIONS:
        if not InputMap.has_action(action):
            var ev := InputEventKey.new()
            ev.physical_keycode = ACTION_KEY[action]
            ev.pressed = true
            InputMap.action_add_event(action, ev)
            ProjectSettings.set_setting("input/actions/" + action, {
                "deadzone": 0.2,
                "events": [
                    {
                        "device": -1,
                        "engine_type": "InputEventKey",
                        "event": {
                            "type": "InputEventKey",
                            "device": -1,
                            "handler": "InputEventKey",
                            "echo": false,
                            "physical_keycode": ACTION_KEY[action],
                            "pressed": true,
                            "modifiers": 0,
                        },
                    },
                ],
            })
