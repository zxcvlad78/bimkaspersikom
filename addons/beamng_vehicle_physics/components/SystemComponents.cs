using Godot;
using System;

namespace BeamNGVehiclePhysics.Core
{
    /// <summary>
    /// Топливная система
    /// </summary>
    public partial class FuelSystem : VehicleComponent
    {
        [ExportGroup("Fuel Tank")]
        [Export] public float TankCapacity { get; set; } = 60.0f; // Литры
        [Export] public float CurrentFuel { get; set; } = 60.0f;
        [Export] public float FuelDensity { get; set; } = 0.75f; // кг/л
        
        [ExportGroup("Fuel Pump")]
        [Export] public float PumpPressure { get; set; } = 3.0f; // бар
        [Export] public bool PumpWorking { get; private set; } = true;
        
        [Signal] public delegate void FuelLevelChangedEventHandler(float percentage);
        [Signal] public delegate void FuelLowEventHandler();
        [Signal] public delegate void FuelEmptyEventHandler();
        
        protected override void Initialize()
        {
            base.Initialize();
            UpdateMass();
        }
        
        protected override void UpdateComponent(float delta)
        {
            base.UpdateComponent(delta);
            CheckFuelLevel();
        }
        
        public void ConsumeFuel(float liters)
        {
            CurrentFuel = Mathf.Max(0, CurrentFuel - liters);
            UpdateMass();
            EmitSignal(SignalName.FuelLevelChanged, GetFuelPercentage());
        }
        
        public void RefuelTo(float targetFuel)
        {
            CurrentFuel = Mathf.Clamp(targetFuel, 0, TankCapacity);
            UpdateMass();
            EmitSignal(SignalName.FuelLevelChanged, GetFuelPercentage());
        }
        
        private void UpdateMass()
        {
            Mass = 10.0f + (CurrentFuel * FuelDensity);
        }
        
        private void CheckFuelLevel()
        {
            float percentage = GetFuelPercentage();
            
            if (percentage <= 0)
            {
                EmitSignal(SignalName.FuelEmpty);
                PumpWorking = false;
            }
            else if (percentage < 0.1f)
            {
                EmitSignal(SignalName.FuelLow);
            }
        }
        
        public float GetFuelPercentage()
        {
            return CurrentFuel / TankCapacity;
        }
        
        public bool HasFuel()
        {
            return CurrentFuel > 0;
        }
    }
    
    /// <summary>
    /// Система охлаждения
    /// </summary>
    public partial class CoolingSystem : VehicleComponent
    {
        [ExportGroup("Radiator")]
        [Export] public float CoolingCapacity { get; set; } = 15.0f; // кВт
        [Export] public float CoolantCapacity { get; set; } = 8.0f; // Литры
        [Export] public float CoolantLevel { get; set; } = 8.0f;
        
        [ExportGroup("Thermostat")]
        [Export] public float ThermostatOpenTemp { get; set; } = 85.0f;
        [Export] public bool ThermostatOpen { get; private set; } = false;
        
        [ExportGroup("Fan")]
        [Export] public bool HasElectricFan { get; set; } = true;
        [Export] public float FanTriggerTemp { get; set; } = 95.0f;
        [Export] public bool FanRunning { get; private set; } = false;
        
        protected override void Initialize()
        {
            base.Initialize();
            Mass = 15.0f;
        }
        
        protected override void UpdateComponent(float delta)
        {
            base.UpdateComponent(delta);
            UpdateThermostat();
            UpdateFan();
        }
        
        private void UpdateThermostat()
        {
            ThermostatOpen = CurrentTemperature > ThermostatOpenTemp;
        }
        
        private void UpdateFan()
        {
            if (HasElectricFan)
            {
                FanRunning = CurrentTemperature > FanTriggerTemp;
            }
        }
        
        public float GetCoolingEffect(float engineTemp)
        {
            if (CoolantLevel <= 0) return 0.1f;
            
            float baseCooling = CoolingCapacity * Integrity;
            
            if (ThermostatOpen)
            {
                baseCooling *= 1.5f;
            }
            
            if (FanRunning)
            {
                baseCooling *= 1.3f;
            }
            
            // Эффективность зависит от скорости
            if (vehicleController != null)
            {
                float speedFactor = Mathf.Clamp(vehicleController.CurrentSpeed / 100.0f, 0.3f, 1.0f);
                baseCooling *= speedFactor;
            }
            
            return baseCooling;
        }
    }
    
    /// <summary>
    /// Электрическая система
    /// </summary>
    public partial class ElectricalSystem : VehicleComponent
    {
        [ExportGroup("Battery")]
        [Export] public float BatteryCapacity { get; set; } = 60.0f; // Ампер-часы
        [Export] public float BatteryCharge { get; set; } = 60.0f;
        [Export] public float BatteryVoltage { get; set; } = 12.0f;
        
        [ExportGroup("Alternator")]
        [Export] public float AlternatorOutput { get; set; } = 90.0f; // Ампер
        [Export] public bool AlternatorWorking { get; private set; } = true;
        
        [ExportGroup("Engine")]
        [Export] public bool IsEngineRunning { get; set; } = false;
        
        [Signal] public delegate void BatteryLowEventHandler();
        [Signal] public delegate void BatteryDeadEventHandler();
        
        protected override void Initialize()
        {
            base.Initialize();
            Mass = 12.0f;
        }
        
        protected override void UpdateComponent(float delta)
        {
            base.UpdateComponent(delta);
            
            if (IsEngineRunning && AlternatorWorking)
            {
                // Зарядка от генератора
                float chargeRate = AlternatorOutput * delta / 3600.0f;
                BatteryCharge = Mathf.Min(BatteryCapacity, BatteryCharge + chargeRate);
            }
            else
            {
                // Разрядка от потребителей
                float drainRate = 5.0f * delta / 3600.0f;
                BatteryCharge = Mathf.Max(0, BatteryCharge - drainRate);
            }
            
            CheckBatteryLevel();
        }
        
        private void CheckBatteryLevel()
        {
            float percentage = BatteryCharge / BatteryCapacity;
            
            if (percentage <= 0)
            {
                EmitSignal(SignalName.BatteryDead);
                if (IsEngineRunning)
                {
                    IsEngineRunning = false;
                }
            }
            else if (percentage < 0.2f)
            {
                EmitSignal(SignalName.BatteryLow);
            }
        }
        
        public bool CanStartEngine()
        {
            return BatteryCharge > BatteryCapacity * 0.3f;
        }
    }
}
