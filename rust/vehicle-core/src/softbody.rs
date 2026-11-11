//! Мягкое тело — деформируемый кузов в стиле BeamNG: решётка узлов, соединённых
//! балками. Балки упругие, при превышении порога деформации начинают
//! «плыть» (пластичность) и в конце концов рвутся. Так кузов мнётся от ударов.
//!
//! Модель не зависит от движка: узлы можно визуализировать мешами в Godot, а
//! столкновения передавать через [`SoftBody::apply_impulse`].

use glam::Vec3;

/// Узел мягкого тела.
#[derive(Clone, Copy, Debug)]
pub struct SoftNode {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Положение в недеформированном состоянии (для оценки повреждений).
    pub rest_position: Vec3,
    /// Обратная масса (0 — бесконечная масса).
    pub inv_mass: f32,
    /// Закреплён ли узел (жёстко связан с кузовом).
    pub pinned: bool,
}

impl SoftNode {
    /// Масса узла, кг.
    #[inline]
    pub fn mass(&self) -> f32 {
        if self.inv_mass > 0.0 {
            1.0 / self.inv_mass
        } else {
            0.0
        }
    }

    /// Смещение от исходного положения, м.
    #[inline]
    pub fn displacement(&self) -> f32 {
        (self.position - self.rest_position).length()
    }
}

/// Балка между двумя узлами.
#[derive(Clone, Copy, Debug)]
pub struct SoftBeam {
    pub a: u32,
    pub b: u32,
    /// Длина в покое, м.
    pub rest_length: f32,
    /// Жёсткость, Н/м.
    pub stiffness: f32,
    /// Демпфирование, Н·с/м.
    pub damping: f32,
    /// Относительная деформация начала пластичности.
    pub yield_strain: f32,
    /// Относительная деформация разрыва.
    pub break_strain: f32,
    /// Балка разорвана.
    pub broken: bool,
    /// Накопленная пластическая деформация 0..1.
    pub plastic: f32,
}

impl SoftBeam {
    /// Текущая длина, м.
    #[inline]
    pub fn length(&self, nodes: &[SoftNode]) -> f32 {
        (nodes[self.b as usize].position - nodes[self.a as usize].position).length()
    }
}

/// Деформируемое тело.
#[derive(Clone, Debug)]
pub struct SoftBody {
    pub nodes: Vec<SoftNode>,
    pub beams: Vec<SoftBeam>,
    /// Скорость затухания (сопротивление воздуха), 1/с.
    pub drag: f32,
    /// Множитель гравитации.
    pub gravity_scale: f32,
    /// Скорость пластического течения 0..1.
    pub plasticity: f32,
    /// Ускорение, выше которого начинается повреждение при столкновении, м/с.
    pub damage_speed: f32,
}

impl Default for SoftBody {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            beams: Vec::new(),
            drag: 0.4,
            gravity_scale: 1.0,
            plasticity: 0.6,
            damage_speed: 4.0,
        }
    }
}

impl SoftBody {
    pub fn new() -> Self {
        Self::default()
    }

    /// Строит решётку-параллелепипед размером `size` с заданным числом
    /// делений по осям и общей массой `mass`. Узлы связываются по осям и
    /// диагоналям граней (на сдвиг).
    pub fn box_lattice(size: Vec3, divisions: [u32; 3], mass: f32, stiffness: f32) -> Self {
        let nx = divisions[0].max(2);
        let ny = divisions[1].max(2);
        let nz = divisions[2].max(2);
        let mut body = SoftBody::new();

        let count = (nx * ny * nz) as usize;
        let node_mass = (mass / count as f32).max(1e-3);

        let idx = |x: u32, y: u32, z: u32| -> u32 { (x * ny + y) * nz + z };

        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    let fx = x as f32 / (nx - 1) as f32 - 0.5;
                    let fy = y as f32 / (ny - 1) as f32 - 0.5;
                    let fz = z as f32 / (nz - 1) as f32 - 0.5;
                    let pos = Vec3::new(fx * size.x, fy * size.y, fz * size.z);
                    body.nodes.push(SoftNode {
                        position: pos,
                        velocity: Vec3::ZERO,
                        rest_position: pos,
                        inv_mass: 1.0 / node_mass,
                        pinned: false,
                    });
                }
            }
        }

        // Смещения для связей: оси + диагонали граней.
        let offsets: [(i32, i32, i32); 9] = [
            (1, 0, 0),
            (0, 1, 0),
            (0, 0, 1),
            (1, 1, 0),
            (1, -1, 0),
            (1, 0, 1),
            (1, 0, -1),
            (0, 1, 1),
            (0, 1, -1),
        ];

        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    let a = idx(x, y, z);
                    for &(dx, dy, dz) in &offsets {
                        let nx2 = x as i32 + dx;
                        let ny2 = y as i32 + dy;
                        let nz2 = z as i32 + dz;
                        if nx2 < 0 || ny2 < 0 || nz2 < 0 {
                            continue;
                        }
                        if nx2 >= nx as i32 || ny2 >= ny as i32 || nz2 >= nz as i32 {
                            continue;
                        }
                        let b = idx(nx2 as u32, ny2 as u32, nz2 as u32);
                        let len = (body.nodes[b as usize].position
                            - body.nodes[a as usize].position)
                            .length();
                        body.beams.push(SoftBeam {
                            a,
                            b,
                            rest_length: len.max(1e-4),
                            stiffness,
                            damping: stiffness * 0.06,
                            yield_strain: 0.08,
                            break_strain: 0.55,
                            broken: false,
                            plastic: 0.0,
                        });
                    }
                }
            }
        }

        body
    }

    /// Число узлов.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Суммарная масса, кг.
    pub fn total_mass(&self) -> f32 {
        self.nodes.iter().map(|n| n.mass()).sum()
    }

    /// Центр масс, м.
    pub fn center_of_mass(&self) -> Vec3 {
        let m: f32 = self.total_mass();
        if m <= 0.0 {
            return Vec3::ZERO;
        }
        let mut c = Vec3::ZERO;
        for n in &self.nodes {
            c += n.position * n.mass();
        }
        c / m
    }

    /// Прикладывает импульс к узлу (например, от удара), кг·м/с.
    #[inline]
    pub fn apply_impulse(&mut self, index: usize, impulse: Vec3) {
        if let Some(n) = self.nodes.get_mut(index) {
            if !n.pinned {
                n.velocity += impulse * n.inv_mass;
            }
        }
    }

    /// Прикладывает импульс к ближайшему узлу от точки.
    pub fn apply_impulse_at(&mut self, point: Vec3, impulse: Vec3) {
        if let Some(i) = self.nearest_node(point) {
            self.apply_impulse(i, impulse);
        }
    }

    /// Индекс ближайшего узла.
    pub fn nearest_node(&self, point: Vec3) -> Option<usize> {
        let mut best = None;
        let mut best_d = f32::MAX;
        for (i, n) in self.nodes.iter().enumerate() {
            let d = (n.position - point).length_squared();
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        best
    }

    /// Закрепляет узлы, ближайшие к заданным точкам (крепление к кузову).
    pub fn pin_nearest(&mut self, point: Vec3) {
        if let Some(i) = self.nearest_node(point) {
            self.nodes[i].pinned = true;
            self.nodes[i].velocity = Vec3::ZERO;
        }
    }

    /// Шаг интегрирования.
    pub fn step(&mut self, dt: f32, gravity: Vec3) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }
        let mut forces = vec![Vec3::ZERO; n];

        // Гравитация.
        for i in 0..n {
            let node = &self.nodes[i];
            if node.pinned {
                continue;
            }
            forces[i] += gravity * node.mass() * self.gravity_scale;
        }

        // Силы балок.
        for beam in &self.beams {
            if beam.broken {
                continue;
            }
            let a = beam.a as usize;
            let b = beam.b as usize;
            let delta = self.nodes[b].position - self.nodes[a].position;
            let len = delta.length();
            if len < 1e-6 {
                continue;
            }
            let dir = delta / len;
            let spring = beam.stiffness * (len - beam.rest_length);
            let rel = (self.nodes[b].velocity - self.nodes[a].velocity).dot(dir);
            let f = dir * (spring + beam.damping * rel);
            forces[a] += f;
            forces[b] -= f;
        }

        // Интегрирование.
        let damp = (1.0 - self.drag * dt).clamp(0.0, 1.0);
        for i in 0..n {
            let node = &mut self.nodes[i];
            if node.pinned {
                node.velocity = Vec3::ZERO;
                continue;
            }
            node.velocity = (node.velocity + forces[i] * node.inv_mass * dt) * damp;
            node.position += node.velocity * dt;
        }

        // Пластичность и разрыв балок.
        let mut new_lengths: Vec<(usize, f32)> = Vec::new();
        for (bi, beam) in self.beams.iter().enumerate() {
            if beam.broken {
                continue;
            }
            let len = beam.length(&self.nodes);
            let strain = (len - beam.rest_length) / beam.rest_length.max(1e-4);
            let abs = strain.abs();
            if abs > beam.break_strain {
                new_lengths.push((bi, f32::NAN)); // маркер разрыва
            } else if abs > beam.yield_strain {
                let excess = abs - beam.yield_strain;
                let rate = (excess * self.plasticity).clamp(0.0, 1.0);
                let target = beam.rest_length + (len - beam.rest_length) * rate;
                new_lengths.push((bi, target));
            }
        }
        for (bi, target) in new_lengths {
            if target.is_nan() {
                self.beams[bi].broken = true;
            } else {
                let beam = &mut self.beams[bi];
                let plastic = (target - beam.rest_length).abs() / beam.rest_length.max(1e-4);
                beam.rest_length = target;
                beam.plastic = (beam.plastic + plastic).min(1.0);
            }
        }
    }

    /// Столкновение с горизонтальной плоскостью.
    pub fn collide_plane(&mut self, height: f32, friction: f32, restitution: f32) {
        for node in &mut self.nodes {
            if node.pinned {
                continue;
            }
            if node.position.y < height {
                node.position.y = height;
                if node.velocity.y < 0.0 {
                    node.velocity.y = -node.velocity.y * restitution;
                }
                node.velocity.x *= (1.0 - friction).clamp(0.0, 1.0);
                node.velocity.z *= (1.0 - friction).clamp(0.0, 1.0);
            }
        }
    }

    /// Максимальное смещение узлов от исходного положения, м.
    pub fn max_deformation(&self) -> f32 {
        self.nodes
            .iter()
            .map(|n| n.displacement())
            .fold(0.0f32, f32::max)
    }

    /// Среднее повреждение балок 0..1.
    pub fn damage(&self) -> f32 {
        if self.beams.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.beams
            .iter()
            .map(|b| if b.broken { 1.0 } else { b.plastic })
            .sum();
        sum / self.beams.len() as f32
    }

    /// Число разорванных балок.
    pub fn broken_beams(&self) -> usize {
        self.beams.iter().filter(|b| b.broken).count()
    }

    /// Восстанавливает исходную форму (ремонт).
    pub fn repair(&mut self) {
        for node in &mut self.nodes {
            node.position = node.rest_position;
            node.velocity = Vec3::ZERO;
        }
        for beam in &mut self.beams {
            beam.broken = false;
            beam.plastic = 0.0;
        }
        // Восстанавливаем исходные длины по недеформированным позициям.
        for beam in &mut self.beams {
            let a = beam.a as usize;
            let b = beam.b as usize;
            beam.rest_length = (self.nodes[b].rest_position - self.nodes[a].rest_position)
                .length()
                .max(1e-4);
        }
    }
}
