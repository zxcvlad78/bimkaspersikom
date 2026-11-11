using Godot;
using System;

namespace BeamNGVehiclePhysics.Core
{
    /// <summary>
    /// Компонент колеса с реалистичной физикой контакта
    /// </summary>
    public partial class WheelComponent : VehicleComponent
    {
        [ExportGroup("Wheel Specifications")]
        [Export] public float Radius { get; set; } = 0.35f;
        [Export] public float Width { get; set; } = 0.225f;
        [Export] public float MaxSteeringAngle { get; set; } = 35.0f;
        [Export] public bool IsSteerable { get; set; } = false;
        [Export] public bool CanBrake { get; set; } = true;
        [Export] public bool IsPowered { get; set; } = false;
        
        [ExportGroup("Tire Properties")]
        [Export] public float TireStiffness { get; set; } = 15.0f;
        [Export] public float TireDamping { get; set; } = 2.0f;
        [Export] public float GripCoefficient { get; set; } = 1.0f;
        [Export] public float MaxGrip { get; set; } = 10000.0f;
        [Export] public float TirePressure { get; set; } = 2.2f; // бар
        
        [ExportGroup("Friction Curves")]
        [Export] public Curve LongitudinalFrictionCurve { get; set; }
        [Export] public Curve LateralFrictionCurve { get; set; }
        
        [ExportGroup("Wheel State")]
        [Export] public float CurrentSteeringAngle { get; private set; } = 0.0f;
        [Export] public float AngularVelocity { get; private set; } = 0.0f;
        [Export] public float SlipRatio { get; private set; } = 0.0f;
        [Export] public float SlipAngle { get; private set; } = 0.0f;
        [Export] public bool IsGrounded { get; private set; } = false;
        [Export] public float Compression { get; private set; } = 0.0f;
        
        [ExportGroup("Brake")]
        [Export] public float MaxBrakeTorque { get; set; } = 3000.0f;
        [Export] public float CurrentBrakeTorque { get; private set; } = 0.0f;
        
        private float targetSteeringAngle = 0.0f;
        private float steeringSpeed = 5.0f;
        private float wheelInertia = 1.5f;
        private Vector3 contactPoint;
        private Vector3 contactNormal;
        private PhysicsDirectSpaceState3D spaceState;
        
        [Signal] public delegate void WheelContactEventHandler(bool isGrounded);
        [Signal] public delegate void TireDeflatedEventHandler();
        
        protected override void Initialize()
        {
            base.Initialize();
            Mass = 15.0f;
            
            if (LongitudinalFrictionCurve == null)
            {
                LongitudinalFrictionCurve = new Curve();
                GenerateDefaultLongitudinalFriction();
            }
            
            if (LateralFrictionCurve == null)
            {
                LateralFrictionCurve = new Curve();
                GenerateDefaultLateralFriction();
            }
        }
        
        public override void _PhysicsProcess(double delta)
        {
            base._PhysicsProcess(delta);
            
            spaceState = GetWorld3D().DirectSpaceState;
            
            UpdateSteering((float)delta);
            UpdateGroundContact();
            
            if (IsGrounded)
            {
                CalculateSlip();
                ApplyTireForces((float)delta);
            }
            
            UpdateWheelRotation((float)delta);
        }
        
        private void UpdateSteering(float delta)
        {
            if (IsSteerable)
            {
                CurrentSteeringAngle = Mathf.Lerp(CurrentSteeringAngle, targetSteeringAngle, steeringSpeed * delta);
                RotationDegrees = new Vector3(RotationDegrees.X, CurrentSteeringAngle, RotationDegrees.Z);
            }
        }
        
        private void UpdateGroundContact()
        {
            var rayOrigin = GlobalPosition;
            var rayEnd = rayOrigin + Vector3.Down * (Radius + 0.1f);
            
            var query = PhysicsRayQueryParameters3D.Create(rayOrigin, rayEnd);
            query.Exclude = new Godot.Collections.Array<Rid> { rigidBody.GetRid() };
            
            var result = spaceState.IntersectRay(query);
            
            bool wasGrounded = IsGrounded;
            IsGrounded = result.Count > 0;
            
            if (IsGrounded)
            {
                contactPoint = (Vector3)result["position"];
                contactNormal = (Vector3)result["normal"];
                Compression = (rayOrigin - contactPoint).Length() / Radius;
            }
            else
            {
                Compression = 0.0f;
            }
            
            if (wasGrounded != IsGrounded)
            {
                EmitSignal(SignalName.WheelContact, IsGrounded);
            }
        }
        
        private void CalculateSlip()
        {
            if (rigidBody == null) return;
            
            Vector3 wheelVelocity = rigidBody.LinearVelocity;
            Vector3 wheelForward = -GlobalTransform.Basis.Z;
            Vector3 wheelRight = GlobalTransform.Basis.X;
            
            float forwardSpeed = wheelVelocity.Dot(wheelForward);
            float lateralSpeed = wheelVelocity.Dot(wheelRight);
            
            // Продольное скольжение (slip ratio)
            float wheelSpeed = AngularVelocity * Radius;
            if (Mathf.Abs(forwardSpeed) > 0.1f)
            {
                SlipRatio = (wheelSpeed - forwardSpeed) / Mathf.Abs(forwardSpeed);
            }
            else
            {
                SlipRatio = 0.0f;
            }
            
            // Угол увода (slip angle)
            if (Mathf.Abs(forwardSpeed) > 0.1f)
            {
                SlipAngle = Mathf.Atan2(lateralSpeed, Mathf.Abs(forwardSpeed)) * (180.0f / Mathf.Pi);
            }
            else
            {
                SlipAngle = 0.0f;
            }
        }
        
        private void ApplyTireForces(float delta)
        {
            if (rigidBody == null) return;
            
            Vector3 wheelForward = -GlobalTransform.Basis.Z;
            Vector3 wheelRight = GlobalTransform.Basis.X;
            
            // Продольная сила (ускорение/торможение)
            float normalizedSlipRatio = Mathf.Clamp(Mathf.Abs(SlipRatio), 0, 1);
            float longitudinalGrip = LongitudinalFrictionCurve.Sample(normalizedSlipRatio);
            float longitudinalForce = longitudinalGrip * MaxGrip * Mathf.Sign(SlipRatio) * GripCoefficient * Integrity;
            
            // Боковая сила (поворот)
            float normalizedSlipAngle = Mathf.Clamp(Mathf.Abs(SlipAngle) / 15.0f, 0, 1);
            float lateralGrip = LateralFrictionCurve.Sample(normalizedSlipAngle);
            float lateralForce = lateralGrip * MaxGrip * Mathf.Sign(SlipAngle) * GripCoefficient * Integrity;
            
            // Применяем силы
            Vector3 totalForce = wheelForward * longitudinalForce + wheelRight * lateralForce;
            rigidBody.ApplyForce(totalForce, GlobalPosition - rigidBody.GlobalPosition);
            
            // Нагрев шины от трения
            float frictionHeat = (Mathf.Abs(longitudinalForce) + Mathf.Abs(lateralForce)) * 0.0001f * delta;
            CurrentTemperature += frictionHeat;
        }
        
        private void UpdateWheelRotation(float delta)
        {
            if (IsGrounded && rigidBody != null)
            {
                Vector3 wheelForward = -GlobalTransform.Basis.Z;
                float forwardSpeed = rigidBody.LinearVelocity.Dot(wheelForward);
                
                // Угловая скорость из линейной скорости
                float targetAngularVelocity = forwardSpeed / Radius;
                
                // Торможение
                if (CurrentBrakeTorque > 0)
                {
                    float brakingDeceleration = CurrentBrakeTorque / wheelInertia;
                    AngularVelocity -= brakingDeceleration * delta * Mathf.Sign(AngularVelocity);
                }
                else
                {
                    AngularVelocity = Mathf.Lerp(AngularVelocity, targetAngularVelocity, 10.0f * delta);
                }
            }
            
            // Визуальное вращение
            float rotationDelta = AngularVelocity * delta * (180.0f / Mathf.Pi);
            RotationDegrees = new Vector3(RotationDegrees.X + rotationDelta, RotationDegrees.Y, RotationDegrees.Z);
        }
        
        public void SetSteeringAngle(float normalizedInput)
        {
            targetSteeringAngle = normalizedInput * MaxSteeringAngle;
        }
        
        public void ApplyBrake(float brakeInput)
        {
            CurrentBrakeTorque = brakeInput * MaxBrakeTorque * Integrity;
        }
        
        public void ApplyDriveTorque(float torque)
        {
            if (IsPowered && IsGrounded)
            {
                AngularVelocity += (torque / wheelInertia) * (float)GetPhysicsProcessDeltaTime();
            }
        }
        
        private void GenerateDefaultLongitudinalFriction()
        {
            LongitudinalFrictionCurve.AddPoint(new Vector2(0, 0));
            LongitudinalFrictionCurve.AddPoint(new Vector2(0.05f, 0.9f));
            LongitudinalFrictionCurve.AddPoint(new Vector2(0.1f, 1.0f));
            LongitudinalFrictionCurve.AddPoint(new Vector2(0.3f, 0.9f));
            LongitudinalFrictionCurve.AddPoint(new Vector2(1.0f, 0.7f));
        }
        
        private void GenerateDefaultLateralFriction()
        {
            LateralFrictionCurve.AddPoint(new Vector2(0, 0));
            LateralFrictionCurve.AddPoint(new Vector2(0.1f, 0.95f));
            LateralFrictionCurve.AddPoint(new Vector2(0.3f, 1.0f));
            LateralFrictionCurve.AddPoint(new Vector2(0.6f, 0.8f));
            LateralFrictionCurve.AddPoint(new Vector2(1.0f, 0.6f));
        }
    }
}
