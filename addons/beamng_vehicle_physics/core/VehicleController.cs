using Godot;
using System;
using System.Collections.Generic;
using System.Linq;

namespace BeamNGVehiclePhysics.Core
{
    /// <summary>
    /// Главный контроллер автомобиля
    /// </summary>
    public partial class VehicleController : RigidBody3D
    {
        [ExportGroup("Vehicle Identity")]
        [Export] public string VehicleId { get; set; } = "";
        [Export] public string VehicleName { get; set; } = "Vehicle";
        
        [ExportGroup("Input")]
        [Export(PropertyHint.Range, "0,1")] public float ThrottleInput { get; set; } = 0.0f;
        [Export(PropertyHint.Range, "0,1")] public float BrakeInput { get; set; } = 0.0f;
        [Export(PropertyHint.Range, "-1,1")] public float SteeringInput { get; set; } = 0.0f;
        [Export(PropertyHint.Range, "0,1")] public float ClutchInput { get; set; } = 1.0f;
        [Export] public int GearInput { get; set; } = 0;
        
        [ExportGroup("Performance")]
        [Export] public float CurrentSpeed { get; private set; } = 0.0f;
        [Export] public float CurrentRPM { get; private set; } = 0.0f;
        [Export] public int CurrentGear { get; private set; } = 0;
        
        [ExportGroup("Physics")]
        [Export] public Vector3 CenterOfMassOffset { get; set; } = Vector3.Zero;
        [Export] public float TotalMass { get; private set; } = 1000.0f;
        
        // Компоненты
        private EngineComponent engine;
        private TransmissionComponent transmission;
        private FuelSystem fuelSystem;
        private CoolingSystem coolingSystem;
        private ElectricalSystem electricalSystem;
        
        private List<WheelComponent> wheels = new List<WheelComponent>();
        private List<SuspensionComponent> suspensions = new List<SuspensionComponent>();
        private List<DeformableBodyPart> bodyParts = new List<DeformableBodyPart>();
        
        private Dictionary<string, VehicleComponent> componentRegistry = new Dictionary<string, VehicleComponent>();
        
        [Signal] public delegate void VehicleStateChangedEventHandler(Godot.Collections.Dictionary state);
        
        public override void _Ready()
        {
            if (string.IsNullOrEmpty(VehicleId))
                VehicleId = Guid.NewGuid().ToString();
            
            InitializeComponents();
            CalculateMassAndCenterOfMass();
        }
        
        private void InitializeComponents()
        {
            // Находим все компоненты
            var allComponents = GetAllChildrenOfType<VehicleComponent>(this);
            
            foreach (var component in allComponents)
            {
                if (!string.IsNullOrEmpty(component.ComponentId))
                    componentRegistry[component.ComponentId] = component;
            }
            
            // Регистрируем специфичные компоненты
            engine = GetNodeOrNull<EngineComponent>("Engine");
            transmission = GetNodeOrNull<TransmissionComponent>("Transmission");
            fuelSystem = GetNodeOrNull<FuelSystem>("FuelSystem");
            coolingSystem = GetNodeOrNull<CoolingSystem>("CoolingSystem");
            electricalSystem = GetNodeOrNull<ElectricalSystem>("ElectricalSystem");
            
            // Находим все колеса и подвески
            wheels = GetAllChildrenOfType<WheelComponent>(this);
            suspensions = GetAllChildrenOfType<SuspensionComponent>(this);
            bodyParts = GetAllChildrenOfType<DeformableBodyPart>(this);
            
            GD.Print($"Vehicle {VehicleName} initialized with {componentRegistry.Count} components");
        }
        
        private void CalculateMassAndCenterOfMass()
        {
            float totalMass = 0.0f;
            Vector3 massCenter = Vector3.Zero;
            
            foreach (var component in componentRegistry.Values)
            {
                if (component.AffectsVehicleMass)
                {
                    totalMass += component.Mass;
                    massCenter += component.GlobalPosition * component.Mass;
                }
            }
            
            if (totalMass > 0)
            {
                massCenter /= totalMass;
                CenterOfMassOffset = ToLocal(massCenter);
            }
            
            TotalMass = totalMass;
            Mass = TotalMass;
            CenterOfMass = CenterOfMassOffset;
        }
        
        public override void _PhysicsProcess(double delta)
        {
            UpdateVehicleState();
            ProcessInput((float)delta);
        }
        
        private void UpdateVehicleState()
        {
            CurrentSpeed = LinearVelocity.Length() * 3.6f; // км/ч
            
            if (engine != null)
                CurrentRPM = engine.CurrentRPM;
            
            if (transmission != null)
                CurrentGear = transmission.CurrentGear;
            
            EmitSignal(SignalName.VehicleStateChanged, GetVehicleData());
        }
        
        private void ProcessInput(float delta)
        {
            // Двигатель
            if (engine != null && electricalSystem != null && electricalSystem.IsEngineRunning)
            {
                engine.SetThrottle(ThrottleInput);
            }
            
            // Трансмиссия
            if (transmission != null)
            {
                transmission.SetClutch(ClutchInput);
                
                if (GearInput != 0)
                {
                    transmission.ShiftGear(GearInput);
                    GearInput = 0;
                }
            }
            
            // Колеса
            foreach (var wheel in wheels)
            {
                if (wheel.IsSteerable)
                    wheel.SetSteeringAngle(SteeringInput);
                
                if (wheel.CanBrake)
                    wheel.ApplyBrake(BrakeInput);
            }
        }
        
        public VehicleComponent GetComponent(string componentId)
        {
            componentRegistry.TryGetValue(componentId, out var component);
            return component;
        }
        
        public T GetComponent<T>(string componentId) where T : VehicleComponent
        {
            return GetComponent(componentId) as T;
        }
        
        public Godot.Collections.Dictionary GetVehicleData()
        {
            return new Godot.Collections.Dictionary
            {
                ["vehicle_id"] = VehicleId,
                ["position"] = GlobalPosition,
                ["rotation"] = GlobalRotation,
                ["linear_velocity"] = LinearVelocity,
                ["angular_velocity"] = AngularVelocity,
                ["throttle"] = ThrottleInput,
                ["brake"] = BrakeInput,
                ["steering"] = SteeringInput,
                ["current_gear"] = CurrentGear,
                ["current_rpm"] = CurrentRPM,
                ["current_speed"] = CurrentSpeed
            };
        }
        
        public void SetVehicleData(Godot.Collections.Dictionary data)
        {
            if (data.ContainsKey("position"))
                GlobalPosition = (Vector3)data["position"];
            if (data.ContainsKey("rotation"))
                GlobalRotation = (Vector3)data["rotation"];
            if (data.ContainsKey("linear_velocity"))
                LinearVelocity = (Vector3)data["linear_velocity"];
            if (data.ContainsKey("angular_velocity"))
                AngularVelocity = (Vector3)data["angular_velocity"];
            if (data.ContainsKey("throttle"))
                ThrottleInput = (float)data["throttle"];
            if (data.ContainsKey("brake"))
                BrakeInput = (float)data["brake"];
            if (data.ContainsKey("steering"))
                SteeringInput = (float)data["steering"];
        }
        
        private List<T> GetAllChildrenOfType<T>(Node parent) where T : Node
        {
            var result = new List<T>();
            
            foreach (Node child in parent.GetChildren())
            {
                if (child is T typedChild)
                    result.Add(typedChild);
                
                result.AddRange(GetAllChildrenOfType<T>(child));
            }
            
            return result;
        }
    }
}
