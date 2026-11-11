using Godot;
using System;

namespace BeamNGVehiclePhysics.Core
{
    /// <summary>
    /// Компонент двигателя с реалистичной физикой
    /// </summary>
    public partial class EngineComponent : VehicleComponent
    {
        [ExportGroup("Engine Specifications")]
        [Export] public float Displacement { get; set; } = 2.0f; // Литры
        [Export] public int Cylinders { get; set; } = 4;
        [Export] public float MaxRPM { get; set; } = 7000.0f;
        [Export] public float IdleRPM { get; set; } = 800.0f;
        [Export] public float MaxTorque { get; set; } = 250.0f; // Нм
        [Export] public float TorquePeakRPM { get; set; } = 4000.0f;
        [Export] public float MaxPower { get; set; } = 150.0f; // кВт
        [Export] public float PowerPeakRPM { get; set; } = 5500.0f;
        
        [ExportGroup("Engine State")]
        [Export] public float CurrentRPM { get; private set; } = 0.0f;
        [Export] public float CurrentTorque { get; private set; } = 0.0f;
        [Export] public float CurrentPower { get; private set; } = 0.0f;
        [Export] public bool IsRunning { get; private set; } = false;
        
        [ExportGroup("Thermal Properties")]
        [Export] public float OperatingTemperature { get; set; } = 90.0f;
        [Export] public float HeatGenerationRate { get; set; } = 5.0f;
        
        [ExportGroup("Fuel Consumption")]
        [Export] public float FuelConsumptionRate { get; set; } = 0.3f; // л/ч на киловатт
        [Export] public float CurrentFuelConsumption { get; private set; } = 0.0f;
        
        [ExportGroup("Engine Curve")]
        [Export] public Curve TorqueCurve { get; set; }
        
        private float throttle = 0.0f;
        private float inertia = 0.1f;
        private float friction = 0.05f;
        private float engineLoad = 0.0f;
        
        [Signal] public delegate void RPMChangedEventHandler(float rpm);
        [Signal] public delegate void EngineStartedEventHandler();
        [Signal] public delegate void EngineStoppedEventHandler();
        
        protected override void Initialize()
        {
            base.Initialize();
            
            MaxTemperature = 120.0f;
            CurrentTemperature = 20.0f;
            
            if (TorqueCurve == null)
            {
                TorqueCurve = new Curve();
                GenerateDefaultTorqueCurve();
            }
        }
        
        protected override void UpdateComponent(float delta)
        {
            base.UpdateComponent(delta);
            
            if (IsRunning)
            {
                UpdateRPM(delta);
                UpdateTorque();
                UpdatePower();
                UpdateTemperature(delta);
                UpdateFuelConsumption();
            }
            else
            {
                // Двигатель останавливается
                CurrentRPM = Mathf.Max(0, CurrentRPM - 500.0f * delta);
            }
        }
        
        private void UpdateRPM(float delta)
        {
            float targetRPM = IdleRPM + (MaxRPM - IdleRPM) * throttle;
            
            // Учитываем нагрузку от трансмиссии
            targetRPM -= engineLoad * 100.0f;
            targetRPM = Mathf.Clamp(targetRPM, IdleRPM, MaxRPM);
            
            // Инерция двигателя
            float rpmDelta = (targetRPM - CurrentRPM) / inertia;
            
            // Трение
            rpmDelta -= friction * CurrentRPM;
            
            CurrentRPM += rpmDelta * delta;
            CurrentRPM = Mathf.Clamp(CurrentRPM, 0, MaxRPM * 1.1f);
            
            // Перекрут - повреждение
            if (CurrentRPM > MaxRPM)
            {
                float overRevDamage = (CurrentRPM - MaxRPM) * 0.001f * delta;
                ApplyDamage(overRevDamage, GlobalPosition, Vector3.Zero);
            }
            
            EmitSignal(SignalName.RPMChanged, CurrentRPM);
        }
        
        private void UpdateTorque()
        {
            // Нормализованные обороты (0-1)
            float normalizedRPM = (CurrentRPM - IdleRPM) / (MaxRPM - IdleRPM);
            normalizedRPM = Mathf.Clamp(normalizedRPM, 0, 1);
            
            // Получаем крутящий момент из кривой
            float torqueMultiplier = TorqueCurve.Sample(normalizedRPM);
            
            CurrentTorque = MaxTorque * torqueMultiplier * throttle * Integrity;
        }
        
        private void UpdatePower()
        {
            // Мощность = Крутящий момент × Угловая скорость
            // P(кВт) = T(Нм) × RPM / 9549
            CurrentPower = (CurrentTorque * CurrentRPM) / 9549.0f;
        }
        
        protected override void UpdateTemperature(float delta)
        {
            if (IsRunning)
            {
                // Генерация тепла пропорциональна нагрузке
                float heatGeneration = HeatGenerationRate * (CurrentRPM / MaxRPM) * throttle;
                CurrentTemperature += heatGeneration * delta;
                
                // Охлаждение
                if (vehicleController != null)
                {
                    var cooling = vehicleController.GetNode<CoolingSystem>("CoolingSystem");
                    if (cooling != null)
                    {
                        float coolingEffect = cooling.GetCoolingEffect(CurrentTemperature);
                        CurrentTemperature -= coolingEffect * delta;
                    }
                }
            }
            
            base.UpdateTemperature(delta);
            
            // Перегрев снижает эффективность
            if (CurrentTemperature > OperatingTemperature * 1.2f)
            {
                float efficiencyLoss = (CurrentTemperature - OperatingTemperature * 1.2f) / 100.0f;
                CurrentTorque *= (1.0f - efficiencyLoss);
            }
        }
        
        private void UpdateFuelConsumption()
        {
            // л/ч = кВт × расход на кВт
            CurrentFuelConsumption = CurrentPower * FuelConsumptionRate;
            
            if (vehicleController != null)
            {
                var fuel = vehicleController.GetNode<FuelSystem>("FuelSystem");
                if (fuel != null)
                {
                    fuel.ConsumeFuel(CurrentFuelConsumption / 3600.0f * (float)GetPhysicsProcessDeltaTime());
                }
            }
        }
        
        public void SetThrottle(float value)
        {
            throttle = Mathf.Clamp(value, 0, 1);
        }
        
        public void SetEngineLoad(float load)
        {
            engineLoad = Mathf.Clamp(load, 0, 1);
        }
        
        public void StartEngine()
        {
            if (electricalSystem != null && electricalSystem.BatteryCharge > 10.0f)
            {
                IsRunning = true;
                CurrentRPM = IdleRPM;
                EmitSignal(SignalName.EngineStarted);
            }
        }
        
        public void StopEngine()
        {
            IsRunning = false;
            EmitSignal(SignalName.EngineStopped);
        }
        
        private void GenerateDefaultTorqueCurve()
        {
            TorqueCurve.AddPoint(new Vector2(0, 0.6f));
            TorqueCurve.AddPoint(new Vector2(0.2f, 0.8f));
            TorqueCurve.AddPoint(new Vector2(0.5f, 1.0f)); // Пик момента
            TorqueCurve.AddPoint(new Vector2(0.7f, 0.95f));
            TorqueCurve.AddPoint(new Vector2(0.85f, 0.85f));
            TorqueCurve.AddPoint(new Vector2(1.0f, 0.7f));
        }
        
        public override Godot.Collections.Dictionary GetComponentData()
        {
            var data = base.GetComponentData();
            data["current_rpm"] = CurrentRPM;
            data["current_torque"] = CurrentTorque;
            data["is_running"] = IsRunning;
            return data;
        }
    }
}
