using Godot;
using System;

namespace BeamNGVehiclePhysics.Core
{
    /// <summary>
    /// Деформируемая часть кузова
    /// </summary>
    public partial class DeformableBodyPart : VehicleComponent
    {
        [ExportGroup("Deformation")]
        [Export] public float DeformationResistance { get; set; } = 1000.0f;
        [Export] public float MaxDeformation { get; set; } = 0.5f;
        [Export] public float PlasticDeformationThreshold { get; set; } = 500.0f;
        
        [ExportGroup("Mesh")]
        [Export] public MeshInstance3D MeshInstance { get; set; }
        [Export] public bool EnableVisualDeformation { get; set; } = true;
        
        private Vector3[] originalVertices;
        private Vector3[] deformedVertices;
        private float[] vertexDeformation;
        
        [Signal] public delegate void PartDeformedEventHandler(float deformationAmount);
        [Signal] public delegate void PartSeverelyDamagedEventHandler();
        
        protected override void Initialize()
        {
            base.Initialize();
            
            if (MeshInstance == null)
                MeshInstance = GetNode<MeshInstance3D>("MeshInstance3D");
            
            if (EnableVisualDeformation && MeshInstance != null)
            {
                InitializeDeformableMesh();
            }
        }
        
        private void InitializeDeformableMesh()
        {
            if (MeshInstance.Mesh is ArrayMesh arrayMesh)
            {
                var arrays = arrayMesh.SurfaceGetArrays(0);
                originalVertices = (Vector3[])arrays[(int)Mesh.ArrayType.Vertex];
                deformedVertices = (Vector3[])originalVertices.Clone();
                vertexDeformation = new float[originalVertices.Length];
            }
        }
        
        public override void ApplyDamage(float damage, Vector3 impactPoint, Vector3 impactForce)
        {
            base.ApplyDamage(damage, impactPoint, impactForce);
            
            if (EnableVisualDeformation && originalVertices != null)
            {
                ApplyDeformation(impactPoint, impactForce, damage);
            }
            
            EmitSignal(SignalName.PartDeformed, damage);
            
            if (Integrity < 0.3f)
            {
                EmitSignal(SignalName.PartSeverelyDamaged);
            }
        }
        
        private void ApplyDeformation(Vector3 impactPoint, Vector3 impactForce, float damage)
        {
            Vector3 localImpactPoint = ToLocal(impactPoint);
            float impactRadius = Mathf.Sqrt(damage / DeformationResistance);
            
            for (int i = 0; i < originalVertices.Length; i++)
            {
                float distance = originalVertices[i].DistanceTo(localImpactPoint);
                
                if (distance < impactRadius)
                {
                    // Falloff от центра удара
                    float influence = 1.0f - (distance / impactRadius);
                    influence = Mathf.Pow(influence, 2);
                    
                    // Деформация
                    float deformationAmount = damage / DeformationResistance * influence;
                    deformationAmount = Mathf.Min(deformationAmount, MaxDeformation);
                    
                    // Направление деформации
                    Vector3 deformDirection = impactForce.Normalized();
                    
                    // Применяем деформацию
                    deformedVertices[i] += deformDirection * deformationAmount;
                    vertexDeformation[i] += deformationAmount;
                    
                    // Ограничиваем максимальную деформацию
                    float totalDeformation = originalVertices[i].DistanceTo(deformedVertices[i]);
                    if (totalDeformation > MaxDeformation)
                    {
                        Vector3 direction = (deformedVertices[i] - originalVertices[i]).Normalized();
                        deformedVertices[i] = originalVertices[i] + direction * MaxDeformation;
                    }
                }
            }
            
            UpdateMeshDeformation();
        }
        
        private void UpdateMeshDeformation()
        {
            if (MeshInstance.Mesh is ArrayMesh arrayMesh)
            {
                var arrays = arrayMesh.SurfaceGetArrays(0);
                arrays[(int)Mesh.ArrayType.Vertex] = deformedVertices;
                
                // Пересчитываем нормали
                var newMesh = new ArrayMesh();
                newMesh.AddSurfaceFromArrays(Mesh.PrimitiveType.Triangles, arrays);
                MeshInstance.Mesh = newMesh;
            }
        }
        
        public void RepairDeformation(float repairAmount)
        {
            if (originalVertices == null) return;
            
            for (int i = 0; i < deformedVertices.Length; i++)
            {
                if (vertexDeformation[i] > 0)
                {
                    // Восстанавливаем только пластическую деформацию
                    if (vertexDeformation[i] > PlasticDeformationThreshold)
                    {
                        deformedVertices[i] = deformedVertices[i].Lerp(originalVertices[i], repairAmount);
                        vertexDeformation[i] *= (1.0f - repairAmount);
                    }
                }
            }
            
            UpdateMeshDeformation();
        }
        
        protected override void HandleDestruction()
        {
            base.HandleDestruction();
            
            // Создаем осколки/обломки
            CreateDebris();
        }
        
        private void CreateDebris()
        {
            // Создание осколков при разрушении
            GD.Print($"Creating debris for {ComponentName}");
            
            // TODO: Spawn debris particles/rigid bodies
        }
    }
}
