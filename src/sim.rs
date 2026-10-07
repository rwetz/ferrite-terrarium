//! The jar's life, replayed from scratch every time you look: hour by hour
//! from the day it was planted, with the sun up by your local clock and the
//! soil drying between waterings. Growth only happens in daylight on damp
//! soil, so a jar left dry wilts and waits; water brings it back. Nothing
//! dies. Everything here is pure and deterministic: the same jar and the
//! same moment always look the same.

pub const HOUR: i64 = 3600;
pub const DAY: i64 = 24 * HOUR;
/// Soil moisture halves every this many hours.
const HALF_LIFE_H: f32 = 60.;
/// Below this the plants wilt and stop growing.
pub const DRY: f32 = 0.15;
/// The most waterings kept (the oldest stop mattering long before this).
pub const KEEP_WATERINGS: usize = 64;

/// What's saved: everything else is derived.
#[derive(Clone, Debug, PartialEq)]
pub struct Jar {
    pub seed: u64,
    /// Unix seconds.
    pub planted: i64,
    /// Unix seconds, oldest first.
    pub waterings: Vec<i64>,
}

impl Jar {
    pub fn new(seed: u64, now: i64) -> Self {
        // A new jar comes watered.
        Jar { seed, planted: now, waterings: vec![now] }
    }

    pub fn water(&mut self, now: i64) {
        self.waterings.push(now);
        if self.waterings.len() > KEEP_WATERINGS {
            self.waterings.remove(0);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Life {
    pub age_days: f32,
    pub moisture: f32,
    /// Hours of good growth, accumulated.
    pub growth: f32,
    pub wilting: bool,
    /// 0–1, the sun now.
    pub light: f32,
    /// What happened, as (day, note), oldest first.
    pub journal: Vec<(u32, String)>,
}

/// How bright it is at a local hour of day (0–24): a smooth day 6–20.
pub fn light(hour: f32) -> f32 {
    let x = ((hour - 13.) / 7.).abs();
    (1. - x * x).clamp(0., 1.).sqrt()
}

/// The growth stages, by hours of growth, and what the journal says.
pub const STAGES: [(f32, &str); 8] = [
    (4., "a sprout broke the soil"),
    (24., "the first true leaves opened"),
    (70., "a second shoot came up"),
    (130., "moss started to creep"),
    (190., "a bud formed on the tallest stem"),
    (240., "it flowered"),
    (320., "a third shoot came up"),
    (480., "the moss reached the glass"),
];

pub fn stage_name(growth: f32) -> &'static str {
    match growth {
        g if g < 4. => "seed",
        g if g < 24. => "sprout",
        g if g < 70. => "seedling",
        g if g < 190. => "young plant",
        g if g < 240. => "budding",
        g if g < 480. => "in flower",
        _ => "overgrown",
    }
}

/// Replay the jar up to `now`. `utc_offset` (seconds) puts the sun on
/// your local clock.
pub fn live(jar: &Jar, now: i64, utc_offset: i64) -> Life {
    let decay = 0.5f32.powf(1. / HALF_LIFE_H);
    let mut moisture = 1.;
    let mut growth = 0f32;
    let mut journal = Vec::new();
    let mut stage = 0;
    let mut wilting = false;
    let mut wilts = 0;
    let mut w = jar.waterings.iter().peekable();
    let hours = ((now - jar.planted).max(0) / HOUR) as usize;
    for h in 0..hours {
        let t = jar.planted + h as i64 * HOUR;
        let day = (h / 24) as u32;
        moisture *= decay;
        while w.peek().is_some_and(|&&at| at <= t + HOUR) {
            w.next();
            if wilting {
                journal.push((day, "watered; it perked up".to_string()));
            }
            moisture = 1.;
            wilting = false;
        }
        if !wilting && moisture < DRY {
            wilting = true;
            wilts += 1;
            // Say so the first few times; after that it's just how it goes.
            if wilts <= 3 {
                journal.push((day, "the soil dried out and it wilted".to_string()));
            }
        }
        let local = ((t + utc_offset).rem_euclid(DAY)) as f32 / HOUR as f32;
        if !wilting {
            growth += light(local) * (moisture / 0.4).min(1.);
        }
        while stage < STAGES.len() && growth >= STAGES[stage].0 {
            journal.push((day, STAGES[stage].1.to_string()));
            stage += 1;
        }
    }
    let local_now = ((now + utc_offset).rem_euclid(DAY)) as f32 / HOUR as f32;
    Life { age_days: (now - jar.planted).max(0) as f32 / DAY as f32, moisture, growth, wilting, light: light(local_now), journal }
}

// ── Drawing ─────────────────────────────────────────────────────────────────

/// What a cell is, for colouring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    Air,
    Stem,
    Leaf,
    Flower,
    Moss,
    Soil,
    Stone,
    Snail,
    Dust,
}

pub const COLS: usize = 60;
pub const ROWS: usize = 22;
const SOIL: usize = 4;

pub type Grid = Vec<Vec<(char, Ink)>>;

/// A tiny deterministic random stream.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // splitmix64
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

/// How tall a stem is after `g` hours of its own growth, out of `max` rows.
fn height(g: f32, max: f32) -> usize {
    if g <= 0. { 0 } else { (max * (1. - (-g / 140.).exp())).ceil() as usize }
}

/// The jar's inside as characters. `minute` (minutes since the epoch)
/// moves the snail and the dust; nothing else changes between growth.
pub fn draw(jar: &Jar, life: &Life, minute: i64) -> Grid {
    let mut grid: Grid = vec![vec![(' ', Ink::Air); COLS]; ROWS];
    let mut rng = Rng(jar.seed);
    let ground = ROWS - SOIL;

    // Soil: a surface line, then texture, stones from the seed.
    for (y, row) in grid.iter_mut().enumerate().skip(ground) {
        for cell in row.iter_mut() {
            *cell = if y == ground {
                ('_', Ink::Soil)
            } else {
                let r = rng.below(10);
                (if r < 2 { ':' } else if r < 4 { '.' } else if r < 5 { ',' } else { ' ' }, Ink::Soil)
            };
        }
    }
    for _ in 0..5 {
        let (x, y) = (rng.below(COLS as u64) as usize, ground + 1 + rng.below(SOIL as u64 - 1) as usize);
        grid[y][x] = ('o', Ink::Stone);
    }

    // Moss creeps out from a seeded point along the surface.
    let moss_at = 6 + rng.below(COLS as u64 - 12) as usize;
    let spread = ((life.growth - STAGES[3].0).max(0.) / 12.) as usize;
    let (from, to) = (moss_at.saturating_sub(spread), (moss_at + spread).min(COLS));
    for (x, cell) in grid[ground].iter_mut().enumerate().take(to).skip(from) {
        *cell = (if (x + moss_at).is_multiple_of(3) { '~' } else { '"' }, Ink::Moss);
    }

    // Up to three stems, each starting when its shoot appears.
    let starts = [0., STAGES[2].0, STAGES[6].0];
    let maxes = [ROWS as f32 - SOIL as f32 - 3., (ROWS - SOIL) as f32 * 0.62, (ROWS - SOIL) as f32 * 0.45];
    let mut xs = [COLS / 2 - 4 + rng.below(8) as usize, 8 + rng.below(10) as usize, COLS - 18 + rng.below(10) as usize];
    for i in 0..3 {
        let g = life.growth - starts[i];
        let h = height(g, maxes[i]);
        if h == 0 {
            continue;
        }
        let mut stem = Rng(jar.seed ^ (i as u64 + 1).wrapping_mul(0x51_7cc1_b727_220a));
        let mut x = xs[i];
        let leaf_every = 2 + stem.below(2) as usize;
        let mut left = stem.below(2) == 0;
        for k in 0..h {
            let y = ground - 1 - k;
            // Lean a little now and then; wilting stems lean more.
            let lean = stem.below(if life.wilting { 3 } else { 6 });
            let (c, nx) = match lean {
                0 if x > 2 => ('\\', x - 1),
                1 if x < COLS - 3 => ('/', x + 1),
                _ => ('|', x),
            };
            grid[y][x] = (if k == 0 { '|' } else { c }, Ink::Stem);
            let top = k + 1 == h;
            if !top && k > 0 && k % leaf_every == 0 {
                let leaf = if life.wilting {
                    if left { ',' } else { '.' }
                } else if g < 30. {
                    '\''
                } else if left {
                    '<'
                } else {
                    '>'
                };
                let lx = if left { x.saturating_sub(1) } else { (x + 1).min(COLS - 1) };
                if grid[y][lx].1 == Ink::Air {
                    grid[y][lx] = (leaf, Ink::Leaf);
                }
                // Mature leaves get a second cell.
                let lx2 = if left { lx.saturating_sub(1) } else { (lx + 1).min(COLS - 1) };
                if g > 90. && !life.wilting && grid[y][lx2].1 == Ink::Air {
                    grid[y][lx2] = (if left { '(' } else { ')' }, Ink::Leaf);
                }
                left = !left;
            }
            if top {
                // The tip: a bud, then a flower on the tallest stem.
                let tip = match (i, life.growth) {
                    (0, g) if g >= STAGES[5].0 && !life.wilting => Some('*'),
                    (0, g) if g >= STAGES[4].0 => Some('o'),
                    _ => None,
                };
                if let Some(t) = tip
                    && y > 0
                {
                    grid[y - 1][x] = (t, Ink::Flower);
                    if t == '*' {
                        if x > 0 {
                            grid[y - 1][x - 1] = ('(', Ink::Flower);
                        }
                        if x + 1 < COLS {
                            grid[y - 1][x + 1] = (')', Ink::Flower);
                        }
                    }
                }
            }
            x = nx;
        }
        xs[i] = x;
    }

    // The snail crawls the surface, there and back, a cell every few
    // minutes; at night it sleeps where it is.
    let span = (COLS - 3) as i64;
    let crawled = if life.light > 0. { minute } else { minute - minute.rem_euclid(DAY / 60) };
    let pos = (crawled / 4).rem_euclid(span * 2);
    let (sx, facing_right) = if pos < span { (pos as usize, true) } else { ((span * 2 - pos) as usize, false) };
    let shell = ground - 1;
    if grid[shell][sx + 1].1 == Ink::Air && grid[shell][sx].1 == Ink::Air {
        if facing_right {
            grid[shell][sx] = ('@', Ink::Snail);
            grid[shell][sx + 1] = ('_', Ink::Snail);
        } else {
            grid[shell][sx] = ('_', Ink::Snail);
            grid[shell][sx + 1] = ('@', Ink::Snail);
        }
        if life.light == 0. && shell > 0 {
            grid[shell - 1][sx + 1] = ('z', Ink::Snail);
        }
    }

    // A few motes of dust in the light.
    if life.light > 0.3 {
        let mut dust = Rng(minute as u64 / 2);
        for _ in 0..3 {
            let (x, y) = (dust.below(COLS as u64) as usize, dust.below((ground - 2) as u64) as usize);
            if grid[y][x].1 == Ink::Air {
                grid[y][x] = ('.', Ink::Dust);
            }
        }
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_790_000_000;

    fn count(grid: &Grid, ink: Ink) -> usize {
        grid.iter().flatten().filter(|c| c.1 == ink).count()
    }

    #[test]
    fn daylight_curve() {
        assert_eq!(light(13.), 1.);
        assert_eq!(light(3.), 0.);
        assert_eq!(light(23.), 0.);
        assert!(light(8.) > 0. && light(8.) < 1.);
    }

    #[test]
    fn a_new_jar_is_a_seed() {
        let jar = Jar::new(7, T0);
        let life = live(&jar, T0, 0);
        assert_eq!(life.growth, 0.);
        assert_eq!(life.moisture, 1.);
        assert!(life.journal.is_empty());
        assert_eq!(stage_name(life.growth), "seed");
        assert_eq!(count(&draw(&jar, &life, 0), Ink::Stem), 0);
    }

    #[test]
    fn a_watered_jar_grows_and_flowers() {
        let mut jar = Jar::new(7, T0);
        for d in (2..40).step_by(2) {
            jar.water(T0 + d * DAY);
        }
        let week = live(&jar, T0 + 7 * DAY, 0);
        let month = live(&jar, T0 + 30 * DAY, 0);
        assert!(month.growth > week.growth && week.growth > 24.);
        assert!(!month.wilting);
        assert!(month.journal.iter().any(|(_, n)| n == "it flowered"), "{:?}", month.journal);
        let grid = draw(&jar, &month, 0);
        assert!(count(&grid, Ink::Flower) > 0);
        assert!(count(&grid, Ink::Stem) > count(&draw(&jar, &week, 0), Ink::Stem));
    }

    #[test]
    fn a_dry_jar_wilts_and_waits() {
        let jar = Jar::new(7, T0);
        let life = live(&jar, T0 + 20 * DAY, 0);
        assert!(life.wilting);
        assert!(life.moisture < DRY);
        let later = live(&jar, T0 + 40 * DAY, 0);
        assert_eq!(later.growth, life.growth, "no growth while wilted");
        assert_eq!(later.journal.iter().filter(|(_, n)| n.contains("wilted")).count(), 1);
    }

    #[test]
    fn water_revives_it() {
        let mut jar = Jar::new(7, T0);
        jar.water(T0 + 20 * DAY);
        let life = live(&jar, T0 + 21 * DAY, 0);
        assert!(!life.wilting);
        assert!(life.journal.iter().any(|(_, n)| n.contains("perked up")));
    }

    #[test]
    fn same_jar_same_picture() {
        let jar = Jar::new(42, T0);
        let life = live(&jar, T0 + 12 * DAY, 0);
        assert_eq!(draw(&jar, &life, 100), draw(&jar, &life, 100));
        assert_ne!(draw(&jar, &life, 100), draw(&Jar { seed: 43, ..jar.clone() }, &life, 100));
        let g = draw(&jar, &life, 100);
        assert_eq!(g.len(), ROWS);
        assert!(g.iter().all(|r| r.len() == COLS));
    }

    #[test]
    fn the_snail_moves_by_day() {
        let jar = Jar::new(1, T0);
        let mut life = live(&jar, T0, 0);
        life.light = 1.;
        let at = |m: i64| draw(&jar, &life, m)[ROWS - SOIL - 1].iter().position(|c| c.0 == '@');
        assert_ne!(at(0), at(40));
    }

    #[test]
    fn waterings_are_capped() {
        let mut jar = Jar::new(1, T0);
        for i in 0..200 {
            jar.water(T0 + i);
        }
        assert_eq!(jar.waterings.len(), KEEP_WATERINGS);
    }
}
