//! Terrarium: a slow Ferrite garden in a jar, drawn in text.
//!
//! It grows by your clock, in daylight, on damp soil — over days, not
//! minutes. Leave it open or don't; when you come back it has carried on
//! without you. Water it now and then. There's nothing to optimise.
//!
//!     cargo run
//!     cargo run -- --days 30      # preview a month-old jar (not saved)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod settings_view;
mod sim;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Local, Offset};
use ferrite_design::prelude::*;
use gpui::{App, AppContext as _, ClickEvent, Context, Entity, Hsla, IntoElement, KeyBinding, Render, Subscription, Task, Window, div, px, size};

use settings::Settings;
use sim::{Ink, Jar, Life};

/// How often the jar is looked at again. Nothing changes faster than this.
const TICK: Duration = Duration::from_secs(30);
/// Wetter than this, watering is refused (it would drown).
const DAMP: f32 = 0.85;

const APPEARANCES: [(&str, &str); 3] = [("dark", "Dark"), ("light", "Light"), ("system", "System")];

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn utc_offset() -> i64 {
    Local::now().offset().fix().local_minus_utc() as i64
}

fn fresh_seed() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7)
}

struct Terrarium {
    settings: Settings,
    settings_open: bool,
    jar: Jar,
    /// `--days`: a made-up jar to look at, never saved.
    preview: bool,
    now: i64,
    life: Life,
    /// Bumped by each watering, for the shake.
    watered: u64,
    confirm_new: bool,
    _tick: Task<()>,
    palette: Entity<CommandPalette>,
    toaster: Entity<Toaster>,
    _appearance: Subscription,
}

impl Terrarium {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut settings = Settings::load();
        let now = unix_now();
        let days: Option<i64> = std::env::args().skip_while(|a| a != "--days").nth(1).and_then(|d| d.parse().ok());
        let (jar, preview) = match days {
            Some(d) => {
                let planted = now - d * sim::DAY;
                let mut jar = Jar::new(fresh_seed(), planted);
                for k in (3..=d).step_by(3) {
                    jar.water(planted + k * sim::DAY);
                }
                (jar, true)
            }
            None => (settings.jar.clone().unwrap_or_else(|| Jar::new(fresh_seed(), now)), false),
        };
        let first_jar = !preview && settings.jar.is_none();
        if !preview {
            settings.jar = Some(jar.clone());
        }

        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |this, cx| this.look(cx)).is_err() {
                    break;
                }
            }
        });
        let mut t = Self {
            life: sim::live(&jar, now, utc_offset()),
            settings,
            settings_open: std::env::args().any(|arg| arg == "--settings"),
            jar,
            preview,
            now,
            watered: 0,
            confirm_new: false,
            _tick: tick,
            palette: cx.new(|cx| CommandPalette::new(window, cx)),
            toaster: cx.new(|_| Toaster::new()),
            _appearance: theme::follow_system(window),
        };
        // Shared launch defaults apply until this app has saved its own preferences.
        t.apply_look(window, cx);
        theme::apply_env(cx);
        if Settings::path().is_some_and(|path| path.exists()) {
            t.apply_look(window, cx);
        } else {
            t.settings.scheme = theme::scheme(cx).key.into();
            t.settings.appearance = match theme::appearance(cx) {
                Appearance::Light => "light", Appearance::System => "system", Appearance::Dark => "dark",
            }.into();
            t.settings.fps = motion::fps();
            t.apply_look(window, cx);
        }
        t.set_commands(cx);
        if first_jar {
            t.save(cx);
            t.toast(toast("A jar, planted").success().message("Check back tomorrow. Water it every few days."), cx);
        }
        t
    }

    // ── Settings ─────────────────────────────────────────────────────────

    fn apply_look(&self, window: &mut Window, cx: &mut App) {
        let s = &self.settings;
        if let Some(scheme) = schemes::by_key(&s.scheme) {
            theme::set_scheme(scheme, cx);
        }
        let appearance = match s.appearance.as_str() {
            "light" => Appearance::Light,
            "system" => Appearance::System,
            _ => Appearance::Dark,
        };
        theme::set_appearance(appearance, window, cx);
        motion::set_fps(s.fps);
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if let Err(err) = self.settings.save() {
            self.toast(toast("Couldn't save the jar").danger().message(err), cx);
        }
        cx.notify();
    }

    fn change(&mut self, f: impl FnOnce(&mut Settings), window: &mut Window, cx: &mut Context<Self>) {
        f(&mut self.settings);
        self.apply_look(window, cx);
        self.save(cx);
    }

    fn toast(&self, t: Toast, cx: &mut Context<Self>) {
        self.toaster.update(cx, |toaster, cx| toaster.push(t, cx));
    }

    fn set_commands(&self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let run = |f: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let weak = weak.clone();
            move |window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this, cx| f(this, window, cx));
            }
        };
        let mut commands = vec![
            command("Settings").icon(Icon::Sliders).shortcut("Ctrl+,").on_run(run(|this, _, cx| { this.settings_open = true; cx.notify(); })),
            command("Water").group("Jar").icon(Icon::Plus).shortcut("W").on_run(run(|this, _, cx| this.water(cx))),
            command("Start a new jar…").group("Jar").icon(Icon::Refresh).on_run(run(|this, _, cx| {
                this.confirm_new = true;
                cx.notify();
            })),
            command("Stepped motion (25 fps)").group("Theme").on_run(run(|this, window, cx| this.change(|s| s.fps = 25, window, cx))),
            command("Smooth motion (240 fps)").group("Theme").on_run(run(|this, window, cx| this.change(|s| s.fps = 240, window, cx))),
        ];
        for (key, label) in APPEARANCES {
            let weak = weak.clone();
            commands.push(command(format!("{label} theme")).group("Theme").on_run(move |window, cx| {
                let _ = weak.update(cx, |this, cx| this.change(|s| s.appearance = key.into(), window, cx));
            }));
        }
        for scheme in SCHEMES {
            let weak = weak.clone();
            commands.push(command(format!("Scheme: {}", scheme.name)).group("Theme").on_run(move |window, cx| {
                let _ = weak.update(cx, |this, cx| this.change(|s| s.scheme = scheme.key.into(), window, cx));
            }));
        }
        self.palette.update(cx, |p, cx| p.set_commands(commands, cx));
    }

    // ── The jar ──────────────────────────────────────────────────────────

    /// Look again: replay the jar up to now.
    fn look(&mut self, cx: &mut Context<Self>) {
        self.now = unix_now();
        self.life = sim::live(&self.jar, self.now, utc_offset());
        cx.notify();
    }

    fn water(&mut self, cx: &mut Context<Self>) {
        if self.life.moisture > DAMP {
            self.toast(toast("Still damp").message("It doesn't need water yet."), cx);
            return;
        }
        self.jar.water(unix_now());
        self.watered += 1;
        if !self.preview {
            self.settings.jar = Some(self.jar.clone());
            self.save(cx);
        }
        self.look(cx);
    }

    fn new_jar(&mut self, cx: &mut Context<Self>) {
        self.jar = Jar::new(fresh_seed(), unix_now());
        self.watered = 0;
        if !self.preview {
            self.settings.jar = Some(self.jar.clone());
            self.save(cx);
        }
        self.look(cx);
        self.toast(toast("A new jar, planted").success(), cx);
    }

    // ── Views ────────────────────────────────────────────────────────────

    fn ink(&self, ink: Ink, cx: &App) -> Hsla {
        let p = palette(cx);
        hsla(match ink {
            Ink::Air | Ink::Dust | Ink::Soil => p.fg_faint,
            Ink::Stem | Ink::Moss | Ink::Stone => p.fg_dim,
            Ink::Leaf | Ink::Snail => p.fg,
            Ink::Flower => p.warning,
        })
    }

    /// The jar's inside, as runs of same-coloured characters per row.
    fn jar_view(&self, scale: Scale, window: &mut Window, cx: &App) -> impl IntoElement + use<> {
        let grid = sim::draw(&self.jar, &self.life, self.now / 60);
        let rows: Vec<_> = grid
            .iter()
            .map(|row| {
                let mut runs: Vec<(Ink, String)> = Vec::new();
                for &(c, ink) in row {
                    // Spaces join whatever run they're in; colour only matters for ink.
                    match runs.last_mut() {
                        Some((last, s)) if *last == ink || c == ' ' => s.push(c),
                        _ => runs.push((ink, c.to_string())),
                    }
                }
                div()
                    .flex()
                    .flex_row()
                    .display(scale, window)
                    .whitespace_nowrap()
                    .children(runs.into_iter().map(|(ink, s)| div().text_color(self.ink(ink, cx)).child(s)))
            })
            .collect();
        div().flex().flex_col().children(rows)
    }

    fn log_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = palette(cx);
        let life = &self.life;
        let day = life.age_days.floor() as u32 + 1;
        let line = |label: &str, value: String| {
            div()
                .flex()
                .flex_row()
                .display(Scale::X1, window)
                .whitespace_nowrap()
                // In font cells, not px: the face snaps to whole device pixels.
                .child(div().flex_none().w(cells(10., window)).text_color(hsla(p.fg_dim)).child(label.to_uppercase()))
                .child(div().text_color(hsla(p.fg)).child(value.to_uppercase()))
        };
        let since_water = self.jar.waterings.last().map(|w| (self.now - w).max(0));
        let ago = |s: i64| match s {
            s if s < sim::HOUR => "just now".to_string(),
            s if s < sim::DAY => format!("{}h ago", s / sim::HOUR),
            s => format!("{}d ago", s / sim::DAY),
        };
        let journal: Vec<_> = life
            .journal
            .iter()
            .rev()
            .take(9)
            .map(|(d, note)| {
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .body(text::SM)
                    .child(div().w(px(56.)).flex_none().text_color(hsla(p.fg_faint)).child(format!("day {}", d + 1)))
                    .child(div().min_w_0().text_color(hsla(p.fg_dim)).child(note.clone()))
            })
            .collect();
        let mood = if life.wilting {
            "thirsty"
        } else if life.light > 0. {
            "growing"
        } else {
            "resting"
        };
        let rows = vec![
            line("day", day.to_string()),
            line("stage", sim::stage_name(life.growth).into()),
            line("mood", mood.into()),
            line("watered", since_water.map(ago).unwrap_or_else(|| "never".into())),
        ];
        div()
            .flex()
            .flex_col()
            .gap_3()
            .w(cells(LOG_COLS as f32, window))
            .children(rows)
            .child(ascii_gauge(life.moisture).label("soil").cells(16))
            .child(ascii_gauge(life.light).label("sun").cells(16))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(ascii_button("water", "Water").primary().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.water(cx))))
                    .child(ascii_button("new-jar", "New jar").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.confirm_new = true;
                        cx.notify();
                    }))),
            )
            .child(ascii_rule(Some("journal")))
            .child(if journal.is_empty() {
                div().body(text::SM).text_color(hsla(p.fg_dim)).child("Nothing yet. Seeds take a while.").into_any_element()
            } else {
                div().flex().flex_col().gap_1().children(journal).into_any_element()
            })
    }
}

impl Render for Terrarium {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let day = self.life.age_days.floor() as u32 + 1;
        let daylight = self.life.light > 0.;
        let stage = sim::stage_name(self.life.growth).to_uppercase();
        // gpui has no zoom: the jar goes up to the largest crisp multiple of
        // the face that the window has room for beside the log.
        let viewport = window.viewport_size();
        let cell = display_size(Scale::X1, window);
        let room = gpui::size(
            viewport.width - cells(LOG_COLS as f32 + 8., window) - space::ROW * 3.,
            viewport.height - chrome::TITLE_BAR_HEIGHT - px(24.) - space::ROW * 2.,
        );
        // The grid at 1×, plus the frame and its title row.
        let natural = gpui::size(cell / 2. * (sim::COLS + 4) as f32, cell * (sim::ROWS + 3) as f32);
        let scale = match whole_scale(room, natural, 3) {
            3 => Scale::X3,
            2 => Scale::X2,
            _ => Scale::X1,
        };
        let jar = ascii_box().title(if self.preview { "jar (preview)" } else { "jar" }).double().child(
            // The jar shakes a little when it's watered.
            shake("jar-shake", self.watered, self.jar_view(scale, window, cx)),
        );
        // When the log doesn't fit beside the jar it wraps underneath; the
        // jar is never squeezed.
        let log = div().flex_none().max_w_full().overflow_hidden().child(ascii_box().title("log").child(self.log_view(window, cx)));
        // Centre only what fits: a jar wider than the window starts at the
        // left edge instead of being cut on both sides.
        let fits = viewport.width >= natural.width + space::ROW * 2. + ferrite_design::components::scroll::WIDTH;
        // Never squeezed under its content (the grid is fixed-width text).
        let jar = div().flex_none().child(jar);
        let close = {
            let weak = cx.weak_entity();
            move |_: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this, cx| {
                    this.confirm_new = false;
                    cx.notify();
                });
            }
        };
        let plant = {
            let weak = cx.weak_entity();
            move |_: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this, cx| {
                    this.confirm_new = false;
                    this.new_jar(cx);
                });
            }
        };

        let drawer = self.settings_drawer(window, cx);
        let gear = Button::new("open-settings").icon(Icon::Sliders).ghost().small().tooltip("Settings · Ctrl+,")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| { this.settings_open = !this.settings_open; cx.notify(); }));
        window_frame().child(power_on_in(
            "power",
            div()
                .flex()
                .flex_col()
                .size_full()
                .bg(hsla(p.bg))
                .text_color(hsla(p.fg))
                .body(text::BASE)
                .on_action(cx.listener(|this, _: &TogglePalette, window, cx| this.palette.update(cx, |p, cx| p.toggle(window, cx))))
                .on_action(cx.listener(|this, _: &OpenSettings, _, cx| { this.settings_open = true; cx.notify(); }))
                .on_action(cx.listener(|this, _: &Water, _, cx| this.water(cx)))
                .child(self.palette.clone())
                .child(self.toaster.clone())
                .child(title_bar("Terrarium").child(gear))
                .child(drawer)
                .child(
                    // Short windows scroll from the top.
                    scroll_area("tank").flex_1().min_h_0().child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .min_h_full()
                            // Spare height goes around the jar, not under it.
                            .items_center()
                            .content_center()
                            .when(fits, |row| row.justify_center())
                            .gap(space::ROW)
                            .p(space::ROW)
                            .child(jar)
                            .child(log),
                    ),
                )
                .child(
                    dialog("new-jar-dialog")
                        .open(self.confirm_new)
                        .title("Start a new jar?")
                        .description(format!("This one is on day {day}. A new jar starts again from a seed."))
                        .danger()
                        .confirm("Plant a new jar", plant)
                        .cancel("Keep this one")
                        .on_close(close),
                )
                .child(
                    status_bar()
                        .left(format!("DAY {day}"))
                        .left(stage)
                        .right(if daylight { "DAYLIGHT" } else { "NIGHT" })
                        .right(theme::scheme(cx).name.to_uppercase())
                        .right_live(format!("{}FPS", motion::fps())),
                ),
        ))
    }
}

/// The log's width, in display cells.
const LOG_COLS: usize = 34;

/// `n` display-face cells at `Scale::X1`, on this window's display.
fn cells(n: f32, window: &Window) -> gpui::Pixels {
    display_size(Scale::X1, window) / 2. * n
}

gpui::actions!(terrarium, [Water, OpenSettings]);

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }
    gpui_platform::application().run(|cx: &mut App| {
        ferrite_design::init(Appearance::Dark, cx);
        cx.bind_keys([KeyBinding::new("ctrl-shift-p", TogglePalette, None), KeyBinding::new("w", Water, None), KeyBinding::new("ctrl-,", OpenSettings, None)]);
        let options = chrome::remembered_window_options("terrarium", "Terrarium", size(px(1180.), px(680.)), cx);
        cx.open_window(options, |window, cx| {
            chrome::square_corners(window);
            chrome::remember_window("terrarium", window, cx);
            chrome::power_off_on_close(window, cx);
            cx.new(|cx| Terrarium::new(window, cx))
        })
        .expect("failed to open the window");
        cx.activate(true);
    });
}
