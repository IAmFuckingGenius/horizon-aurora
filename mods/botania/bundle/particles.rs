// Botania's own particles for Horizon Aurora: a modified port of Vazkii's Botania (Botania
// License, see LICENSE and NOTICE.md). Botania draws its wisps and sparkles on the client, with a
// colour, a size, a lifetime and motion each (`FXWisp`, `FXSparkle`), from the bursts, pools and
// flowers it syncs. The port's bursts, beams and stars live on the server only, so the server
// tells each player near what happened (one mod message a level tick, `botania:fx`, records of
// `botania_core::fx`), and the client half turns each record into the particles Botania's client
// adds: compiled particle controllers (`ParticleEngine::controlled`) with Botania's sprites
// (`botania:wisp`, `botania:sparkle` in the content mod), additive blending and its motion. A
// vanilla client is never sent mod messages; a Horizon client without the bundle ignores them.

use super::Game;
use botania_core::fx::{self, Record, Rng};
use horizon_client::particles::{Particle, ParticleController, ParticleLevel, ParticleUpdate};
use horizon_core::random::{LegacyRandom, RandomSource};
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

// The content mod's particle definitions (`assets/botania/particles/*.json`).
const WISP: &str = "botania:wisp";
const SPARKLE: &str = "botania:sparkle";

// A player is sent what happens within this many blocks of where they stand (the client draws
// nothing farther than 32 from its camera), plus what a record reaches beyond its anchor.
const REACH: f64 = 36.0;
// A message's records stop short of the 30 000 bytes a mod message may carry (a record is at most
// 73 bytes), and a player is sent at most this many a level tick.
const MESSAGE_BYTES: usize = 29_000;
const MESSAGES_PER_TICK: usize = 4;
// Records waiting for their level's tick to end; past this the oldest half goes (a level that
// stopped ticking with records queued).
const MAX_QUEUED: usize = 8192;
// The pool's charging animations under way on the client (Botania keeps one per item in a pool).
const MAX_FLOWS: usize = 512;
// `displayChargingParticles`: an event keeps an item's animation going for 15 ticks.
const FLOW_TICKS: i32 = 15;
// The Thundercaller's bolt is drawn as a line of glowing dots, one every half their width (at
// least this far apart).
const BOLT_DOT_STEP: f64 = 0.03;

// ---- random numbers -------------------------------------------------------------------------

// `Math.random()` for the effects, on both sides: a xorshift of the module's own.
static RANDOM: Mutex<u64> = Mutex::new(0);

fn random_bits() -> u64 {
    let mut s = RANDOM.lock().unwrap_or_else(PoisonError::into_inner);
    if *s == 0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        *s = (now ^ 0x2545_F491_4F6C_DD1D) | 1;
    }
    let mut x = *s;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *s = x;
    x
}

struct MathRandom;

impl Rng for MathRandom {
    fn float(&mut self) -> f32 {
        (random_bits() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn double(&mut self) -> f64 {
        (random_bits() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn gaussian(&mut self) -> f64 {
        // Box-Muller.
        let u = self.double().max(f64::MIN_POSITIVE);
        let v = self.double();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

// A particle's own `random`.
struct Own<'a>(&'a mut LegacyRandom);

impl Rng for Own<'_> {
    fn float(&mut self) -> f32 {
        self.0.next_float()
    }
    fn double(&mut self) -> f64 {
        self.0.next_double()
    }
    fn gaussian(&mut self) -> f64 {
        self.0.next_gaussian()
    }
}

// ---- the server: what happened, sent at the end of the level's tick ---------------------------

struct Queued {
    dim: &'static str,
    record: Record,
}

static QUEUE: LazyLock<Mutex<Vec<Queued>>> = LazyLock::new(Mutex::default);

fn queue() -> MutexGuard<'static, Vec<Queued>> {
    QUEUE.lock().unwrap_or_else(PoisonError::into_inner)
}

// Records an effect of this level's tick for the players near it (a server's hooks only).
pub(super) fn send(game: &Game<'_>, records: impl IntoIterator<Item = Record>) {
    if !game.native_authoritative() {
        return;
    }
    let dim = game.dim.id();
    let mut q = queue();
    for record in records {
        if q.len() >= MAX_QUEUED {
            q.drain(..MAX_QUEUED / 2);
        }
        q.push(Queued { dim, record });
    }
}

// The per-burst part of a trail's size, for a burst just shot (its UUID's `nextInt(9001)`).
pub(super) fn wobble() -> f32 {
    fx::burst_wobble((MathRandom.double() * 9001.0) as i32)
}

// `ManaPoolBlockEntity`: the wisp a pool breathes now and then (`clientTick`), and the sparkles
// of an infusion (`craftingEffect`, once a tick however many items it made).
pub(super) fn pool(game: &Game<'_>, pos: [i32; 3], mana: i32, max_mana: i32, crafted: bool) {
    let mut records = Vec::new();
    if let Some(w) = fx::pool_wisp(pos, mana, max_mana, &mut MathRandom) {
        records.push(Record::Wisp(w));
    }
    if crafted {
        records.push(Record::Craft {
            block: pos,
            height: 0.75,
        });
    }
    send(game, records);
}

// `GeneratingFlowerBlockEntity.tickFlower`: a fuller flower sparkles more often.
pub(super) fn generating(game: &Game<'_>, pos: [i32; 3], mana: i32, max_mana: i32, color: u32) {
    if let Some(s) = fx::generating_sparkle(pos, mana, max_mana, color, &mut MathRandom) {
        send(game, [Record::Sparkle(s)]);
    }
}

// `PureDaisyBlockEntity`: sparkles over the blocks it is turning, every tick, and wisps over the
// block it turned now.
pub(super) fn daisy(
    game: &Game<'_>,
    pos: [i32; 3],
    ticks_remaining: &[i32; 8],
    turned: Option<[i32; 3]>,
) {
    let mask = ticks_remaining
        .iter()
        .enumerate()
        .filter(|(_, t)| **t > 0)
        .fold(0u8, |m, (i, _)| m | (1 << i));
    let mut records = Vec::new();
    if mask != 0 {
        records.push(Record::Daisy { daisy: pos, mask });
    }
    if let Some(block) = turned {
        records.push(Record::DaisyDone { block });
    }
    send(game, records);
}

// `PetalApothecaryBlockEntity.clientTick` for the items in it (up to the first empty slot): each
// one in 33 ticks a sparkle of the petal's colour over the water, and now and then a splash; the
// sparkles of a craft.
pub(super) fn apothecary<'s>(
    game: &mut Game<'_>,
    pos: [i32; 3],
    items: impl Iterator<Item = &'s str>,
    crafted: bool,
) {
    let mut records = Vec::new();
    let mut splash = false;
    for item in items {
        if let Some((s, splashes)) = fx::apothecary_sparkle(pos, item, &mut MathRandom) {
            records.push(Record::Sparkle(s));
            splash |= splashes;
        }
    }
    if crafted {
        records.push(Record::Craft {
            block: pos,
            height: 1.0,
        });
    }
    send(game, records);
    if splash && game.native_authoritative() {
        let at = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        // GENERIC_SPLASH at 0.1, pitch 10 (clamped to 2 as the client clamps it).
        game.native_play_sound_in_place("minecraft:entity.generic.splash", at, 4, 0.1, 2.0);
    }
}

// After `dimension`'s tick (the last of the module's `level_tick_post` hooks): its records leave
// for the players in it, each player sent the ones near them in one message.
pub(super) fn flush(game: &mut Game<'_>, dimension: &str) {
    if !game.native_authoritative() {
        return;
    }
    let mine: Vec<Record> = {
        let mut q = queue();
        if q.is_empty() {
            return;
        }
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *q)
            .into_iter()
            .partition(|e| e.dim == dimension);
        *q = rest;
        mine.into_iter().map(|e| e.record).collect()
    };
    if mine.is_empty() {
        return;
    }
    let anchors: Vec<([f64; 3], f64)> = mine.iter().map(Record::anchor).collect();
    for player in game.native_server_players() {
        if player.dimension != dimension {
            continue;
        }
        let mut data = vec![fx::FORMAT];
        let mut sent = 0;
        for (record, (at, beyond)) in mine.iter().zip(&anchors) {
            let reach = REACH + beyond;
            let d: f64 = (0..3).map(|k| (player.position[k] - at[k]).powi(2)).sum();
            if d > reach * reach {
                continue;
            }
            record.encode(&mut data);
            if data.len() >= MESSAGE_BYTES {
                let _ = game.native_net_send_to_player(player.uuid, fx::CHANNEL, &data);
                data.truncate(1);
                sent += 1;
                if sent == MESSAGES_PER_TICK {
                    break;
                }
            }
        }
        if data.len() > 1 && sent < MESSAGES_PER_TICK {
            let _ = game.native_net_send_to_player(player.uuid, fx::CHANNEL, &data);
        }
    }
}

// ---- the client: the records become Botania's particles ---------------------------------------

// `native_net_from_server`: a message of this module's server half.
pub(super) fn from_server(game: &mut Game<'_>, channel: &str, data: &[u8]) {
    if channel != fx::CHANNEL || !game.native_window() {
        return;
    }
    for record in fx::decode(data) {
        draw(game, record);
    }
}

fn draw(game: &mut Game<'_>, record: Record) {
    let r = &mut MathRandom;
    match record {
        Record::Burst {
            pos,
            prev,
            motion,
            color,
            size,
            wobble,
        } => {
            let motion = motion.map(f64::from);
            for w in fx::burst_trail(pos, prev, motion, color, size, wobble, r) {
                wisp(game, &w);
            }
        }
        Record::BurstDeath { pos, color, size } => {
            let (wisps, s) = fx::burst_death(pos, color, size, r);
            for w in &wisps {
                wisp(game, w);
            }
            sparkle(game, &s);
        }
        Record::Wisp(w) => wisp(game, &w),
        Record::Sparkle(s) => sparkle(game, &s),
        Record::Craft { block, height } => {
            for s in fx::craft_sparkles(block, f64::from(height), r) {
                sparkle(game, &s);
            }
        }
        Record::Flow {
            pool,
            param,
            relative,
            charging,
        } => start_flow(pool, param, relative, charging),
        Record::Beam { from, to } => {
            let hue = r.double() as f32;
            for s in fx::beam(from, to, hue) {
                sparkle(game, &s);
            }
        }
        Record::Star { pos } => {
            for s in fx::star_sparkles(pos, r) {
                sparkle(game, &s);
            }
        }
        Record::Bolt { from, to } => bolt(game, from, to),
        Record::Daisy { daisy, mask } => {
            for s in fx::daisy_sparkles(daisy, mask, r) {
                sparkle(game, &s);
            }
        }
        Record::DaisyDone { block } => {
            for w in fx::daisy_wisps(block, r) {
                wisp(game, &w);
            }
        }
    }
}

// `FXWisp`: a soft glowing dot, additive and full bright, swelling and shrinking over 28 to 40
// ticks (times its `maxAgeMul`), drifting with its own drag and gravity.
#[derive(Clone, Debug)]
struct WispMotion {
    quad: f32,
    half_life: i32,
    gravity: f32,
}

impl ParticleController for WispMotion {
    fn tick(&mut self, p: &mut Particle, level: &ParticleLevel<'_>) -> ParticleUpdate {
        if p.advance_age() {
            p.velocity[1] -= f64::from(self.gravity);
            let v = p.velocity;
            p.move_colliding(level, v);
            if self.gravity == 0.0 {
                for c in &mut p.velocity {
                    *c *= fx::WISP_DRAG;
                }
            }
            p.size = fx::wisp_size(self.quad, p.age, self.half_life);
        }
        ParticleUpdate::default()
    }
}

fn wisp(game: &mut Game<'_>, w: &fx::Wisp) {
    let r = &mut MathRandom;
    let quad = fx::wisp_quad_size(r.float(), w.size);
    let lifetime = fx::wisp_lifetime(r.double(), w.max_age_mul);
    let motion = WispMotion {
        quad,
        half_life: lifetime / 2,
        gravity: w.gravity,
    };
    let Some(mut p) = game.particles.controlled(WISP, w.pos, true, motion) else {
        return;
    };
    p.velocity = w.vel;
    p.rgba = [w.rgb[0], w.rgb[1], w.rgb[2], fx::WISP_ALPHA];
    p.lifetime = lifetime;
    p.size = 0.0;
    p.set_box_size(fx::BOX_SIZE, fx::BOX_SIZE);
    p.set_physics(!w.no_clip);
    p.set_additive(true);
    p.set_full_bright(true);
    game.particles.push(p);
}

// `FXSparkle`: a four-sprite twinkle, additive, shrinking over `3 * m` ticks, jumping about its
// block unless it passes through blocks.
#[derive(Clone, Debug)]
struct SparkleMotion {
    quad: f32,
    physics: bool,
    fake: bool,
}

impl ParticleController for SparkleMotion {
    fn tick(&mut self, p: &mut Particle, level: &ParticleLevel<'_>) -> ParticleUpdate {
        // `setSpriteFromAge` comes before the age goes up.
        let frame = fx::sprite_from_age(p.age, p.lifetime, fx::SPARKLE_SPRITES);
        if p.advance_age() {
            if self.physics {
                let middle = [
                    p.position[0],
                    p.position[1] + f64::from(fx::BOX_SIZE) / 2.0,
                    p.position[2],
                ];
                let full = |b: [i32; 3]| {
                    level
                        .facts
                        .get(level.block(b[0], b[1], b[2]))
                        .full_collision
                };
                let v = fx::sparkle_wiggle(middle, full, &mut Own(p.random()));
                p.velocity = v;
            }
            let v = p.velocity;
            p.move_colliding(level, v);
            for c in &mut p.velocity {
                *c *= fx::SPARKLE_DRAG;
            }
            if p.on_ground {
                p.velocity[0] *= fx::SPARKLE_GROUND_DRAG;
                p.velocity[2] *= fx::SPARKLE_GROUND_DRAG;
            }
            if self.fake && p.age > 1 {
                p.remove();
            }
            p.size = fx::sparkle_size(self.quad, p.age, p.lifetime);
        }
        ParticleUpdate {
            frame: Some(frame),
            ..Default::default()
        }
    }
}

fn sparkle(game: &mut Game<'_>, s: &fx::Sparkle) {
    let quad = fx::sparkle_quad_size(MathRandom.float(), s.size);
    let lifetime = fx::sparkle_lifetime(s.m);
    let physics = !s.fake && !s.no_clip;
    let motion = SparkleMotion {
        quad,
        physics,
        fake: s.fake,
    };
    let Some(mut p) = game.particles.controlled(SPARKLE, s.pos, true, motion) else {
        return;
    };
    p.velocity = [0.0; 3];
    p.rgba = [s.rgb[0], s.rgb[1], s.rgb[2], fx::SPARKLE_ALPHA];
    p.lifetime = lifetime;
    p.size = fx::sparkle_size(quad, 0, lifetime);
    p.set_box_size(fx::BOX_SIZE, fx::BOX_SIZE);
    p.set_physics(physics);
    p.set_additive(true);
    game.particles.push(p);
}

// One dot of a bolt's segment: hidden until the bolt's growing end reaches it, then shown until
// its fading start does (`FadeFunction.fade(0.5)`).
#[derive(Clone, Debug)]
struct BoltDot {
    delay: i32,
}

impl ParticleController for BoltDot {
    fn tick(&mut self, p: &mut Particle, _level: &ParticleLevel<'_>) -> ParticleUpdate {
        if self.delay > 0 {
            self.delay -= 1;
            if self.delay == 0 {
                p.set_visible(true);
            }
        } else {
            p.advance_age();
        }
        ParticleUpdate::default()
    }
}

// `ClientProxy.lightningFX`: Botania's bolt (`BoltRenderer`), its untextured quads drawn as a line
// of wisp sprites of the segment's width in the bolt's colour.
fn bolt(game: &mut Game<'_>, from: [f64; 3], to: [f64; 3]) {
    let segments = fx::bolt(from, to, &mut MathRandom);
    let n = segments.len();
    for (i, s) in segments.iter().enumerate() {
        let (on, off) = fx::bolt_window(i, n);
        if off <= on {
            continue;
        }
        let d = [
            s.to[0] - s.from[0],
            s.to[1] - s.from[1],
            s.to[2] - s.from[2],
        ];
        let length = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let step = f64::from(s.size).max(BOLT_DOT_STEP);
        let dots = ((length / step).ceil() as usize).clamp(1, 256);
        for k in 0..dots {
            let t = k as f64 / dots as f64;
            let at = [
                s.from[0] + d[0] * t,
                s.from[1] + d[1] * t,
                s.from[2] + d[2] * t,
            ];
            let Some(mut p) = game
                .particles
                .controlled(WISP, at, true, BoltDot { delay: on })
            else {
                continue;
            };
            p.velocity = [0.0; 3];
            p.rgba = fx::BOLT_COLOR;
            p.lifetime = off - on;
            p.size = s.size;
            p.set_physics(false);
            p.set_additive(true);
            p.set_full_bright(true);
            if on > 0 {
                p.set_visible(false);
            }
            game.particles.push(p);
        }
    }
}

// ---- the pool's charging animation on the client -------------------------------------------

// `chargingParticles` and `drainingParticles`: an item's animation runs 15 ticks from its pool's
// last event, a wisp every second tick.
struct Flow {
    pool: [i32; 3],
    param: u8,
    charging: bool,
    relative: f32,
    left: i32,
}

static FLOWS: LazyLock<Mutex<Vec<Flow>>> = LazyLock::new(Mutex::default);

fn flows() -> MutexGuard<'static, Vec<Flow>> {
    FLOWS.lock().unwrap_or_else(PoisonError::into_inner)
}

fn start_flow(pool: [i32; 3], param: u8, relative: f32, charging: bool) {
    let mut f = flows();
    if let Some(flow) = f
        .iter_mut()
        .find(|f| f.pool == pool && f.param == param && f.charging == charging)
    {
        flow.left = FLOW_TICKS;
        flow.relative = relative;
    } else if f.len() < MAX_FLOWS {
        f.push(Flow {
            pool,
            param,
            charging,
            relative,
            left: FLOW_TICKS,
        });
    }
}

// `native_tick` in the window: the charging animations under way go on a tick.
pub(super) fn client_tick(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let due: Vec<fx::Wisp> = {
        let mut f = flows();
        if f.is_empty() {
            return;
        }
        let mut due = Vec::new();
        for flow in f.iter_mut() {
            flow.left -= 1;
            if flow.left % 2 == 0 {
                due.push(fx::flow_wisp(
                    flow.pool,
                    i32::from(flow.param),
                    flow.relative,
                    flow.charging,
                    &mut MathRandom,
                ));
            }
        }
        f.retain(|flow| flow.left > 0);
        due
    };
    for w in &due {
        wisp(game, w);
    }
}
