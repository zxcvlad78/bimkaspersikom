using Godot;
using System;

namespace BeamNGVehiclePhysics.Core
{
    /// <summary>
    /// Компонент подвески
    /// </summary>
    public partial class SuspensionComponent : VehicleComponent
    {
        public enum SuspensionType
        {
            Independent,      // Независимая
            McPherson,        // МакФерсон
            DoubleWishbone,   // Двухрычажная
            MultiLink,        // Многорычажная
            TorsionBeam       // Торсионная балка
        }
        
        [ExportGroup("Suspension Type")]
        [Export] public SuspensionType Type { get; set; } = SuspensionType.Independent;
        
        [ExportGroup("Spring Properties")]
        [Export] public float SpringStiffness { get; set; } = 35000.0f; // Н/м
        [Export] public float DamperCompression { get; set; } = 4500.0f; // Н·с/м
        [Export] public float DamperRebound { get; set; } = 7500.0f;
        [Export] public float RestLength { get; set; } = 0.4f;
        [Export] public float MaxCompression { get; set; } = 0.15f;
        [Export] public float MaxExtension { get; set; } = 0.15f;
        
        [ExportGroup("Anti-Roll Bar")]
        [Export] public bool HasAntiRollBar { get; set; } = false;
        [Export] public float AntiRollStiffness { get; set; } = 10000.0f;
        [Export] public SuspensionComponent PairedSuspension { get; set; }
        
        [ExportGroup("Suspension State")]
        [Export] public float CurrentCompression { get; private set; } = 0.0f;
        [Export] public float CompressionVelocity { get; private set; } = 0.0f;
        [Export] public float CurrentForce { get; private set; } = 0.0f;
        
        [ExportGroup("Wheel Connection")]
        [Export] public WheelComponent ConnectedWheel { get; set; }
        
        private float previousCompression = 0.0f;
        
        [Signal] public delegate void SuspensionBottomedOutEventHandler();
        [Signal] public delegate void SuspensionToppedOutEventHandler();
        
        protected override void Initialize()
        {
            base.Initialize();
            Mass = 25.0f;
        }
        
        protected override void UpdateComponent(float delta)
        {
            base.UpdateComponent(delta);
            
            if (ConnectedWheel != null && ConnectedWheel.IsGrounded)
            {
                UpdateSuspensionCompression();
                ApplySuspensionForces(delta);
                
                if (HasAntiRollBar && PairedSuspension != null)
                {
                    ApplyAntiRollForce();
                }
            }
        }
        
        private void UpdateSuspensionCompression()
        {
            if (ConnectedWheel == null) return;
            
            // Рассчитываем сжатие подвески
            float wheelPosition = ToLocal(ConnectedWheel.GlobalPosition).Y;
            CurrentCompression = Mathf.Clamp(RestLength - wheelPosition, -MaxExtension, MaxCompression);
            
            // Скорость сжатия
            float deltaTime = (float)GetPhysicsProcessDeltaTime();
            if (deltaTime > 0)
            {
                CompressionVelocity = (CurrentCompression - previousCompression) / deltaTime;
                previousCompression = CurrentCompression;
            }
            
            // Проверка на пробой
            if (CurrentCompression >= MaxCompression)
            {
                EmitSignal(SignalName.SuspensionBottomedOut);
                // Повреждение от пробоя
                ApplyDamage(10.0f * deltaTime, GlobalPosition, Vector3.Zero);
            }
            else if (CurrentCompression <= -MaxExtension)
            {
                EmitSignal(SignalName.SuspensionToppedOut);
            }
        }
        
        private void ApplySuspensionForces(float delta)
        {
            if (rigidBody == null) return;
            
            // Сила пружины (закон Гука)
            float springForce = SpringStiffness * CurrentCompression;
            
            // Сила демпфера
            float damperForce = 0.0f;
            if (CompressionVelocity > 0) // Сжатие
            {
                damperForce = DamperCompression * CompressionVelocity;
            }
            else // Отбой
            {
                damperForce = DamperRebound * CompressionVelocity;
            }
            
            // Общая сила
            CurrentForce = (springForce + damperForce) * Integrity;
            
            // Применяем силу вверх к кузову
            Vector3 suspensionForce = Vector3.Up * CurrentForce;
            rigidBody.ApplyForce(suspensionForce, GlobalPosition - rigidBody.GlobalPosition);
            
            // Нагрев от работы демпфера
            float damperHeat = Mathf.Abs(damperForce) * 0.0001f * delta;
            CurrentTemperature += damperHeat;
        }
        
        private void ApplyAntiRollForce()
        {
            if (PairedSuspension == null || rigidBody == null) return;
            
            // Разница в сжатии между парными подвесками
            float compressionDifference = CurrentCompression - PairedSuspension.CurrentCompression;
            
            // Сила стабилизатора пропорциональна разнице
            float antiRollForce = compressionDifference * AntiRollStiffness * 0.5f;
            
            // Применяем противоположные силы
            Vector3 force = Vector3.Up * antiRollForce;
            rigidBody.ApplyForce(force, GlobalPosition - rigidBody.GlobalPosition);
        }
        
        public override Godot.Collections.Dictionary GetComponentData()
        {
            var data = base.GetComponentData();
            data["current_compression"] = CurrentCompression;
            data["compression_velocity"] = CompressionVelocity;
            return data;
        }
    }
}
