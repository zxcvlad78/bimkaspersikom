using Godot;
using System;

namespace BeamNGVehiclePhysics.Core
{
    /// <summary>
    /// Компонент трансмиссии
    /// </summary>
    public partial class TransmissionComponent : VehicleComponent
    {
        public enum TransmissionType
        {
            Manual,        // Механическая
            Automatic,     // Автоматическая
            CVT,          // Вариатор
            DCT           // Робот с двойным сцеплением
        }
        
        [ExportGroup("Transmission Type")]
        [Export] public TransmissionType Type { get; set; } = TransmissionType.Manual;
        
        [ExportGroup("Gears")]
        [Export] public float[] GearRatios { get; set; } = new float[] { -3.5f, 0f, 3.5f, 2.0f, 1.5f, 1.0f, 0.8f };
        [Export] public float FinalDriveRatio { get; set; } = 3.7f;
        [Export] public int CurrentGear { get; set; } = 1; // 0 = нейтраль, <0 = задняя
        
        [ExportGroup("Clutch")]
        [Export(PropertyHint.Range, "0,1")] public float ClutchEngagement { get; private set; } = 1.0f;
        [Export] public float ClutchEngageSpeed { get; set; } = 5.0f;
        
        [ExportGroup("Shift Parameters")]
        [Export] public float ShiftTime { get; set; } = 0.2f;
        [Export] public float AutoShiftRPM { get; set; } = 6500.0f;
        [Export] public float AutoDownshiftRPM { get; set; } = 2000.0f;
        
        [ExportGroup("Efficiency")]
        [Export] public float MechanicalEfficiency { get; set; } = 0.95f;
        
        private float targetClutch = 1.0f;
        private bool isShifting = false;
        private float shiftTimer = 0.0f;
        private EngineComponent engine;
        
        [Signal] public delegate void GearChangedEventHandler(int newGear);
        [Signal] public delegate void ShiftStartedEventHandler();
        [Signal] public delegate void ShiftCompletedEventHandler();
        
        protected override void Initialize()
        {
            base.Initialize();
            Mass = 80.0f;
            MaxTemperature = 150.0f;
            
            engine = vehicleController?.GetNode<EngineComponent>("Engine");
        }
        
        protected override void UpdateComponent(float delta)
        {
            base.UpdateComponent(delta);
            
            UpdateClutch(delta);
            UpdateShifting(delta);
            
            if (Type == TransmissionType.Automatic)
            {
                AutoShift();
            }
            
            TransferPowerToWheels();
        }
        
        private void UpdateClutch(float delta)
        {
            ClutchEngagement = Mathf.MoveToward(ClutchEngagement, targetClutch, ClutchEngageSpeed * delta);
        }
        
        private void UpdateShifting(float delta)
        {
            if (isShifting)
            {
                shiftTimer += delta;
                
                if (shiftTimer >= ShiftTime)
                {
                    isShifting = false;
                    shiftTimer = 0.0f;
                    EmitSignal(SignalName.ShiftCompleted);
                }
            }
        }
        
        private void AutoShift()
        {
            if (engine == null || isShifting) return;
            
            // Повышение передачи
            if (engine.CurrentRPM > AutoShiftRPM && CurrentGear < GearRatios.Length - 1 && CurrentGear > 0)
            {
                ShiftGear(1);
            }
            // Понижение передачи
            else if (engine.CurrentRPM < AutoDownshiftRPM && CurrentGear > 1)
            {
                ShiftGear(-1);
            }
        }
        
        public void ShiftGear(int direction)
        {
            if (isShifting) return;
            
            int newGear = CurrentGear + direction;
            newGear = Mathf.Clamp(newGear, -1, GearRatios.Length - 1);
            
            if (newGear != CurrentGear)
            {
                CurrentGear = newGear;
                isShifting = true;
                targetClutch = 0.0f;
                
                EmitSignal(SignalName.GearChanged, CurrentGear);
                EmitSignal(SignalName.ShiftStarted);
                
                // После переключения снова включаем сцепление
                GetTree().CreateTimer(ShiftTime * 0.5f).Timeout += () => targetClutch = 1.0f;
            }
        }
        
        public void SetClutch(float value)
        {
            if (Type == TransmissionType.Manual)
            {
                targetClutch = Mathf.Clamp(value, 0, 1);
            }
        }
        
        private void TransferPowerToWheels()
        {
            if (engine == null || vehicleController == null) return;
            if (CurrentGear == 0 || Mathf.Approximately(ClutchEngagement, 0)) return;
            
            // Крутящий момент на выходе из трансмиссии
            float gearRatio = GearRatios[Mathf.Abs(CurrentGear)];
            float totalRatio = gearRatio * FinalDriveRatio;
            float outputTorque = engine.CurrentTorque * totalRatio * ClutchEngagement * MechanicalEfficiency * Integrity;
            
            // Распределяем момент на ведущие колеса
            var wheels = vehicleController.GetNode<Node>("Wheels");
            if (wheels != null)
            {
                var poweredWheels = new System.Collections.Generic.List<WheelComponent>();
                
                foreach (Node child in wheels.GetChildren())
                {
                    if (child is WheelComponent wheel && wheel.IsPowered)
                    {
                        poweredWheels.Add(wheel);
                    }
                }
                
                if (poweredWheels.Count > 0)
                {
                    float torquePerWheel = outputTorque / poweredWheels.Count;
                    
                    foreach (var wheel in poweredWheels)
                    {
                        wheel.ApplyDriveTorque(torquePerWheel);
                    }
                }
            }
            
            // Нагрузка двигателя от колес
            if (engine != null)
            {
                float wheelResistance = CalculateWheelResistance();
                engine.SetEngineLoad(wheelResistance);
            }
            
            // Нагрев от проскальзывания сцепления
            float slippage = 1.0f - ClutchEngagement;
            CurrentTemperature += slippage * engine.CurrentRPM * 0.0001f * (float)GetPhysicsProcessDeltaTime();
        }
        
        private float CalculateWheelResistance()
        {
            if (vehicleController == null) return 0.0f;
            
            float resistance = 0.0f;
            var wheels = vehicleController.GetNode<Node>("Wheels");
            
            if (wheels != null)
            {
                foreach (Node child in wheels.GetChildren())
                {
                    if (child is WheelComponent wheel && wheel.IsPowered)
                    {
                        resistance += wheel.CurrentBrakeTorque * 0.001f;
                        resistance += Mathf.Abs(wheel.SlipRatio) * 0.1f;
                    }
                }
            }
            
            return Mathf.Clamp(resistance, 0, 1);
        }
        
        public override Godot.Collections.Dictionary GetComponentData()
        {
            var data = base.GetComponentData();
            data["current_gear"] = CurrentGear;
            data["clutch_engagement"] = ClutchEngagement;
            data["is_shifting"] = isShifting;
            return data;
        }
    }
}
