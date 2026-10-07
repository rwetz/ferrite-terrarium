//! The look and the jar, saved as plain `key = value` lines in
//! `<config>/ferrite/terrarium.conf`. The jar is just its seed, when it was
//! planted and when it was watered; its life is replayed from those.

use std::path::PathBuf;

use crate::sim::Jar;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub scheme: String,
    /// `dark`, `light` or `system`.
    pub appearance: String,
    pub fps: u32,
    pub jar: Option<Jar>,
}

impl Default for Settings {
    fn default() -> Self {
        Self { scheme: "verdigris".into(), appearance: "dark".into(), fps: 25, jar: None }
    }
}

impl Settings {
    pub fn parse(src: &str) -> Self {
        let mut s = Self::default();
        let (mut seed, mut planted, mut waterings) = (None, None, Vec::new());
        for line in src.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "scheme" if !v.is_empty() => s.scheme = v.into(),
                "appearance" if matches!(v, "dark" | "light" | "system") => s.appearance = v.into(),
                "fps" => s.fps = v.parse().map(|f: u32| f.clamp(12, 240)).unwrap_or(s.fps),
                "seed" => seed = v.parse().ok(),
                "planted" => planted = v.parse().ok(),
                "watered" => waterings.extend(v.parse::<i64>().ok()),
                _ => {}
            }
        }
        if let (Some(seed), Some(planted)) = (seed, planted) {
            waterings.sort_unstable();
            waterings.retain(|&w| w >= planted);
            s.jar = Some(Jar { seed, planted, waterings });
        }
        s
    }

    pub fn serialize(&self) -> String {
        let mut out = format!("# Terrarium.\nscheme = {}\nappearance = {}\nfps = {}\n", self.scheme, self.appearance, self.fps);
        if let Some(jar) = &self.jar {
            out.push_str(&format!("\n# The jar. Its life is replayed from these; editing them is cheating.\nseed = {}\nplanted = {}\n", jar.seed, jar.planted));
            for w in &jar.waterings {
                out.push_str(&format!("watered = {w}\n"));
            }
        }
        out
    }

    pub fn path() -> Option<PathBuf> {
        let base = if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        };
        base.map(|b| b.join("ferrite").join("terrarium.conf"))
    }

    pub fn load() -> Self {
        Self::path().and_then(|p| std::fs::read_to_string(p).ok()).map(|s| Self::parse(&s)).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("no config directory")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, self.serialize()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let s = Settings {
            scheme: "phosphor".into(),
            appearance: "light".into(),
            fps: 60,
            jar: Some(Jar { seed: 99, planted: 1_790_000_000, waterings: vec![1_790_000_000, 1_790_100_000] }),
        };
        assert_eq!(Settings::parse(&s.serialize()), s);
    }

    #[test]
    fn no_jar_until_planted() {
        assert_eq!(Settings::parse("scheme = mono\n").jar, None);
        assert_eq!(Settings::parse("seed = 4\n").jar, None, "a seed alone isn't a jar");
    }

    #[test]
    fn waterings_before_planting_are_dropped() {
        let s = Settings::parse("seed = 1\nplanted = 100\nwatered = 300\nwatered = 50\nwatered = 200\n");
        assert_eq!(s.jar.unwrap().waterings, vec![200, 300]);
    }
}
