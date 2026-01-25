use std::time::{Duration, Instant};

use cgmath::{Point3, Vector3};
use ragnarok_packets::{Direction, EntityId, Sex, TilePosition, WorldPosition};
use sysinfo::{get_current_pid, Pid, System};

use crate::graphics::Color;
use crate::loaders::{AsyncLoader, EffectLoader, TextureLoader};
use crate::state::{ClientState, ClientStatePathExt};
use crate::world::{
    EffectCenter, EffectHolder, EffectWithLight, Entity, Library, Map, Npc, PathFinder, PointLightId,
    Traversable,
};

pub const MIN_BENCHMARK_COUNT: usize = 100;
const PORING_JOB_ID: u16 = 1002;
const BENCHMARK_ENTITY_ID_BASE: u32 = 5_000_000;
const BENCHMARK_WALK_OFFSET: i32 = 6;

pub struct BenchmarkConfig {
    pub count: usize,
    pub effect_path: Option<String>,
}

pub struct BenchmarkState {
    config: BenchmarkConfig,
    walkers: Vec<BenchmarkWalker>,
    metrics: BenchmarkMetrics,
    focus_point: Point3<f32>,
}

struct BenchmarkWalker {
    entity_index: usize,
    from: TilePosition,
    to: TilePosition,
    next_is_to: bool,
}

struct BenchmarkMetrics {
    system: System,
    pid: Option<Pid>,
    last_refresh: Instant,
    frame_time_ms: f64,
    frame_time_avg_ms: f64,
    frames_per_second: usize,
    memory_mb: Option<u64>,
}

impl BenchmarkState {
    pub fn new(count: usize, effect_path: Option<String>) -> Self {
        let config = BenchmarkConfig { count, effect_path };
        Self {
            config,
            walkers: Vec::new(),
            metrics: BenchmarkMetrics::new(),
            focus_point: Point3::new(0.0, 0.0, 0.0),
        }
    }

    pub fn focus_point(&self) -> Point3<f32> {
        self.focus_point
    }

    pub fn update_metrics(&mut self, delta_time: f64, frames_per_second: usize) {
        self.metrics.update(delta_time, frames_per_second);
    }

    pub fn overlay_lines(&self) -> [String; 3] {
        self.metrics.overlay_lines(self.config.count)
    }

    pub fn spawn(
        &mut self,
        map: &Map,
        path_finder: &mut PathFinder,
        state_ctx: &mut rust_state::Context<ClientState>,
        async_loader: &AsyncLoader,
        library: &Library,
        effect_loader: &EffectLoader,
        texture_loader: &TextureLoader,
        effect_holder: &mut EffectHolder,
        client_tick: ragnarok_packets::ClientTick,
    ) {
        let positions = collect_spawn_positions(map, self.config.count);
        if positions.is_empty() {
            return;
        }

        let width = map.width() as i32;
        let height = map.height() as i32;
        let center_x = width / 2;
        let center_y = height / 2;
        let focus_tile = find_walkable_near(map, width, height, center_x, center_y, 8)
            .unwrap_or_else(|| positions[positions.len() / 2]);

        self.focus_point = map
            .get_world_position(focus_tile)
            .unwrap_or(Point3::new(0.0, 0.0, 0.0));

        {
            let entities = state_ctx.follow_mut(crate::state::client_state().entities());
            entities.clear();
        }
        state_ctx
            .follow_mut(crate::state::client_state().dead_entities())
            .clear();
        effect_holder.clear();

        self.walkers.clear();
        self.walkers.reserve(self.config.count);

        let effect = self
            .config
            .effect_path
            .as_deref()
            .and_then(|path| effect_loader.get_or_load(path, texture_loader).ok());

        let entities = state_ctx.follow_mut(crate::state::client_state().entities());

        for index in 0..self.config.count {
            let from = positions[index % positions.len()];
            let to = pick_target(map, from, index as i32);

            let entity_id = EntityId(BENCHMARK_ENTITY_ID_BASE + index as u32);
            let entity_data = korangar_networking::EntityData {
                entity_id,
                movement_speed: 150,
                job: PORING_JOB_ID,
                head: 0,
                position: WorldPosition::new(from.x, from.y, Direction::South),
                destination: Some(WorldPosition::new(to.x, to.y, Direction::South)),
                health_points: 1,
                maximum_health_points: 1,
                head_direction: 0,
                sex: Sex::Both,
            };

            if let Some(npc) = Npc::new(map, path_finder, entity_data, client_tick) {
                let mut entity = Entity::Npc(npc);
                let entity_type = entity.get_entity_type();
                let entity_part_files = entity.get_entity_part_files(library);

                if let Some(animation_data) =
                    async_loader.request_animation_data_load(entity_id, entity_type, entity_part_files)
                {
                    entity.set_animation_data(animation_data);
                }

                let entity_index = entities.len();
                entities.push(entity);

                if let Some(effect) = effect.as_ref() {
                    let frame_timer = effect.new_frame_timer();
                    effect_holder.add_unit(
                        Box::new(EffectWithLight::new(
                            effect.clone(),
                            frame_timer,
                            EffectCenter::Entity(entity_id, Point3::new(0.0, 0.0, 0.0)),
                            Vector3::new(0.0, 0.0, 0.0),
                            PointLightId::new(entity_id.0),
                            Vector3::new(0.0, 0.0, 0.0),
                            Color::WHITE,
                            0.0,
                            true,
                        )),
                        entity_id,
                    );
                }

                self.walkers.push(BenchmarkWalker {
                    entity_index,
                    from,
                    to,
                    next_is_to: false,
                });
            }
        }
    }

    pub fn update_walkers(
        &mut self,
        map: &Map,
        path_finder: &mut PathFinder,
        state_ctx: &mut rust_state::Context<ClientState>,
        client_tick: ragnarok_packets::ClientTick,
    ) {
        let entities = state_ctx.follow_mut(crate::state::client_state().entities());

        for walker in &mut self.walkers {
            if let Some(entity) = entities.get_mut(walker.entity_index) {
                if entity.stopped_moving() {
                    let target = if walker.next_is_to { walker.to } else { walker.from };
                    let from = entity.get_tile_position();
                    entity.move_from_to(map, path_finder, from, target, client_tick);
                    walker.next_is_to = !walker.next_is_to;
                }
            }
        }
    }
}

impl BenchmarkMetrics {
    fn new() -> Self {
        Self {
            system: System::new(),
            pid: get_current_pid().ok(),
            last_refresh: Instant::now() - Duration::from_secs(1),
            frame_time_ms: 0.0,
            frame_time_avg_ms: 0.0,
            frames_per_second: 0,
            memory_mb: None,
        }
    }

    pub fn update(&mut self, delta_time: f64, frames_per_second: usize) {
        let frame_time_ms = delta_time * 1000.0;
        self.frame_time_ms = frame_time_ms;
        if self.frame_time_avg_ms == 0.0 {
            self.frame_time_avg_ms = frame_time_ms;
        } else {
            self.frame_time_avg_ms = (self.frame_time_avg_ms * 0.9) + (frame_time_ms * 0.1);
        }

        self.frames_per_second = frames_per_second;

        if self.last_refresh.elapsed() >= Duration::from_millis(500) {
            if let Some(pid) = self.pid {
                self.system.refresh_process(pid);
                if let Some(process) = self.system.process(pid) {
                    let memory_kb = process.memory();
                    self.memory_mb = Some((memory_kb + 1023) / 1024);
                }
            }

            self.last_refresh = Instant::now();
        }
    }

    pub fn overlay_lines(&self, count: usize) -> [String; 3] {
        let frame_budget_ms = 1000.0 / 60.0;
        let cpu_percent = if frame_budget_ms > 0.0 {
            (self.frame_time_avg_ms / frame_budget_ms) * 100.0
        } else {
            0.0
        };

        let memory_line = match self.memory_mb {
            Some(value) => format!("Mem: {value} MB"),
            None => "Mem: N/A".to_string(),
        };

        [
            format!("FPS: {}", self.frames_per_second),
            format!("Frame: {:.2} ms ({:.0}%)", self.frame_time_avg_ms, cpu_percent),
            format!("{memory_line} | Monsters: {count}"),
        ]
    }
}

fn collect_spawn_positions(map: &Map, count: usize) -> Vec<TilePosition> {
    let width = map.width() as i32;
    let height = map.height() as i32;
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let grid = (count as f32).sqrt().ceil() as i32;
    let spacing = 2;
    let center_x = width / 2;
    let center_y = height / 2;
    let start_x = center_x - (grid * spacing / 2);
    let start_y = center_y - (grid * spacing / 2);

    let mut positions = Vec::with_capacity(count);

    for index in 0..(grid * grid) {
        if positions.len() >= count {
            break;
        }

        let x = start_x + (index % grid) * spacing;
        let y = start_y + (index / grid) * spacing;

        if let Some(position) = find_walkable_near(map, width, height, x, y, 4) {
            positions.push(position);
        }
    }

    positions
}

fn pick_target(map: &Map, from: TilePosition, index: i32) -> TilePosition {
    let width = map.width() as i32;
    let height = map.height() as i32;
    let offset = if index % 2 == 0 {
        BENCHMARK_WALK_OFFSET
    } else {
        -BENCHMARK_WALK_OFFSET
    };
    let offset_y = if index % 3 == 0 {
        BENCHMARK_WALK_OFFSET
    } else {
        -BENCHMARK_WALK_OFFSET
    };

    find_walkable_near(
        map,
        width,
        height,
        from.x as i32 + offset,
        from.y as i32 + offset_y,
        6,
    )
    .unwrap_or(from)
}

fn find_walkable_near(
    map: &Map,
    width: i32,
    height: i32,
    start_x: i32,
    start_y: i32,
    max_radius: i32,
) -> Option<TilePosition> {
    for radius in 0..=max_radius {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }

                let x = start_x + dx;
                let y = start_y + dy;
                if x < 0 || y < 0 || x >= width || y >= height {
                    continue;
                }

                let position = TilePosition {
                    x: x as u16,
                    y: y as u16,
                };

                if map.is_walkable(position) {
                    return Some(position);
                }
            }
        }
    }

    None
}
