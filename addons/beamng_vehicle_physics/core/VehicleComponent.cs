using Godot;
using System;
using System.Collections.Generic;

namespace BeamNGVehiclePhysics.Core
{
    /// <summary>
    /// Базовый класс для всех компонентов автомобиля
    /// </summary>
    public partial class VehicleComponent : Node3D
    {
        [ExportGroup("Component Identity")]
        [Export] public string ComponentId { get; set; } = "";
        [Export] public string ComponentName { get; set; } = "Component";
        [Export] public float Mass { get; set; } = 1.0f;
        
        [ExportGroup("Damage")]
        [Export(PropertyHint.Range, "0,1")] public float Integrity { get; set; } = 1.0f;
        [Export] public float MaxDurability { get; set; } = 100.0f;
        [Export] public float CurrentDurability { get; set; } = 100.0f;
        
        [ExportGroup("Temperature")]
        [Export] public float CurrentTemperature { get; set; } = 20.0f;
        [Export] public float MaxTemperature { get; set; } = 100.0f;
        [Export] public float CoolingRate { get; set; } = 0.5f;
        
        [ExportGroup("Physics")]
        [Export] public bool AffectsVehicleMass { get; set; } = true;
        
        protected RigidBody3D rigidBody;
        protected VehicleController vehicleController;
        
        [Signal] public delegate void DamagedEventHandler(float damage);
        [Signal] public delegate void DestroyedEventHandler();
        [Signal] public delegate void TemperatureChangedEventHandler(float temperature);
        
        public override void _Ready()
        {
            if (string.IsNullOrEmpty(ComponentId))
                ComponentId = Guid.NewGuid().ToString();
            
            rigidBody = GetParent() as RigidBody3D;
            if (rigidBody == null)
                rigidBody = GetParentOrNull<RigidBody3D>();
            
            vehicleController = GetParentOrNull<VehicleController>();
            
            Initialize();
        }
        
        public override void _PhysicsProcess(double delta)
        {
            UpdateTemperature((float)delta);
            UpdateIntegrity((float)delta);
            UpdateComponent((float)delta);
        }
        
        protected virtual void Initialize()
        {
            // Переопределяется в дочерних классах
        }
        
        protected virtual void UpdateComponent(float delta)
        {
            // Переопределяется в дочерних классах
        }
        
        public virtual void ApplyDamage(float damage, Vector3 impactPoint, Vector3 impactForce)
        {
            CurrentDurability = Mathf.Max(0, CurrentDurability - damage);
            Integrity = CurrentDurability / MaxDurability;
            
            EmitSignal(SignalName.Damaged, damage);
            
            if (CurrentDurability <= 0)
            {
                EmitSignal(SignalName.Destroyed);
                HandleDestruction();
            }
        }
        
        protected virtual void HandleDestruction()
        {
            // Переопределяется в дочерних классах
            GD.Print($"Component {ComponentName} destroyed!");
        }
        
        protected virtual void UpdateTemperature(float delta)
        {
            // Естественное охлаждение до температуры окружающей среды
            if (CurrentTemperature > 20.0f)
            {
                CurrentTemperature -= CoolingRate * delta;
                CurrentTemperature = Mathf.Max(20.0f, CurrentTemperature);
                EmitSignal(SignalName.TemperatureChanged, CurrentTemperature);
            }
        }
        
        protected virtual void UpdateIntegrity(float delta)
        {
            // Перегрев вызывает повреждение
            if (CurrentTemperature > MaxTemperature * 0.9f)
            {
                float overheatDamage = (CurrentTemperature - MaxTemperature * 0.9f) * 0.01f * delta;
                ApplyDamage(overheatDamage, GlobalPosition, Vector3.Zero);
            }
        }
        
        public virtual Godot.Collections.Dictionary GetComponentData()
        {
            return new Godot.Collections.Dictionary
            {
                ["component_id"] = ComponentId,
                ["integrity"] = Integrity,
                ["temperature"] = CurrentTemperature,
                ["durability"] = CurrentDurability
            };
        }
        
        public virtual void SetComponentData(Godot.Collections.Dictionary data)
        {
            if (data.ContainsKey("integrity"))
                Integrity = (float)data["integrity"];
            if (data.ContainsKey("temperature"))
                CurrentTemperature = (float)data["temperature"];
            if (data.ContainsKey("durability"))
                CurrentDurability = (float)data["durability"];
        }
        
        protected T GetParentOrNull<T>() where T : Node
        {
            Node parent = GetParent();
            while (parent != null)
            {
                if (parent is T result)
                    return result;
                parent = parent.GetParent();
            }
            return null;
        }
    }
}
