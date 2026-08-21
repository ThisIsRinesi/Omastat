use ratatui::style::Color;
use serde_json::Value as JsonValue;
use std::{fs, path::PathBuf};
use toml::Value as TomlValue;

#[derive(Debug, Clone)]
pub(super) struct Theme {
    pub(super) bg: Color,
    pub(super) panel: Color,
    pub(super) panel_alt: Color,
    pub(super) selection: Color,
    pub(super) text: Color,
    pub(super) muted: Color,
    pub(super) dim: Color,
    pub(super) border: Color,
    pub(super) primary: Color,
    pub(super) secondary: Color,
    pub(super) tertiary: Color,
    pub(super) success: Color,
    pub(super) warn: Color,
    pub(super) danger: Color,
}

impl Theme {
    pub(super) fn load() -> Self {
        read_noctalia_theme()
            .or_else(read_skwd_wall_theme)
            .or_else(read_omarchy_theme)
            .unwrap_or_else(Self::fallback)
    }

    pub(super) fn fallback() -> Self {
        Self::from_palette(
            Rgb::new(5, 8, 14),
            Rgb::new(232, 245, 255),
            Rgb::new(34, 211, 238),
            Rgb::new(167, 139, 250),
            Rgb::new(255, 73, 198),
            Rgb::new(255, 83, 112),
            Rgb::new(88, 110, 130),
        )
    }

    fn from_palette(
        bg: Rgb,
        text: Rgb,
        primary: Rgb,
        secondary: Rgb,
        tertiary: Rgb,
        danger: Rgb,
        outline: Rgb,
    ) -> Self {
        let fallback = Self::fallback_accents();
        let primary = if primary.saturation() < 0.08 {
            fallback.0
        } else {
            primary
        };
        let secondary = if secondary.saturation() < 0.08 {
            fallback.1
        } else {
            secondary
        };
        let tertiary = if tertiary.saturation() < 0.08 {
            fallback.2
        } else {
            tertiary
        };

        Self {
            bg: bg.color(),
            panel: bg.mix(text, 0.035).color(),
            panel_alt: bg.mix(primary, 0.14).color(),
            selection: bg.mix(primary, 0.26).color(),
            text: text.color(),
            muted: bg.mix(text, 0.62).color(),
            dim: bg.mix(text, 0.28).color(),
            border: bg.mix(outline, 0.72).color(),
            primary: primary.color(),
            secondary: secondary.color(),
            tertiary: tertiary.color(),
            success: Rgb::new(89, 255, 184).mix(secondary, 0.35).color(),
            warn: Rgb::new(255, 220, 92).mix(tertiary, 0.25).color(),
            danger: danger.color(),
        }
    }

    fn fallback_accents() -> (Rgb, Rgb, Rgb) {
        (
            Rgb::new(34, 211, 238),
            Rgb::new(167, 139, 250),
            Rgb::new(255, 73, 198),
        )
    }
}

#[derive(Clone, Copy)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    fn parse(value: &str) -> Option<Self> {
        let value = value.trim().strip_prefix('#').unwrap_or(value.trim());
        if value.len() != 6 && value.len() != 8 {
            return None;
        }
        Some(Self {
            r: u8::from_str_radix(&value[0..2], 16).ok()?,
            g: u8::from_str_radix(&value[2..4], 16).ok()?,
            b: u8::from_str_radix(&value[4..6], 16).ok()?,
        })
    }

    fn color(self) -> Color {
        Color::Rgb(self.r, self.g, self.b)
    }

    fn mix(self, other: Self, amount: f64) -> Self {
        let amount = amount.clamp(0.0, 1.0);
        let inv = 1.0 - amount;
        Self {
            r: (self.r as f64 * inv + other.r as f64 * amount).round() as u8,
            g: (self.g as f64 * inv + other.g as f64 * amount).round() as u8,
            b: (self.b as f64 * inv + other.b as f64 * amount).round() as u8,
        }
    }

    fn saturation(self) -> f64 {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        if max <= f64::EPSILON {
            0.0
        } else {
            (max - min) / max
        }
    }
}

fn read_noctalia_theme() -> Option<Theme> {
    let path = dirs::config_dir()?.join("noctalia/colors.json");
    let contents = fs::read_to_string(path).ok()?;
    let value: JsonValue = serde_json::from_str(&contents).ok()?;
    Some(Theme::from_palette(
        json_color(&value, &["dark", "mSurface"])?,
        json_color(&value, &["dark", "mOnSurface"])?,
        json_color(&value, &["dark", "mPrimary"])?,
        json_color(&value, &["dark", "mSecondary"])?,
        json_color(&value, &["dark", "mTertiary"])?,
        json_color(&value, &["dark", "mError"])?,
        json_color(&value, &["dark", "mOutline"])?,
    ))
}

fn read_skwd_wall_theme() -> Option<Theme> {
    for path in skwd_wall_theme_paths() {
        let Ok(contents) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<JsonValue>(&contents) else {
            continue;
        };
        if let Some(theme) = theme_from_material_value(&value) {
            return Some(theme);
        }
    }
    None
}

fn skwd_wall_theme_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("omastat/theme/colors.json"));
        paths.push(config_dir.join("omastat/theme/matugen.json"));
        paths.push(config_dir.join("skwd-wall/colors.json"));
    }
    if let Some(cache_dir) = dirs::cache_dir() {
        paths.push(cache_dir.join("skwd/colors.json"));
        paths.push(cache_dir.join("skwd-wall/colors.json"));
    }
    paths
}

fn theme_from_material_value(value: &JsonValue) -> Option<Theme> {
    Some(Theme::from_palette(
        material_color(
            value,
            &[
                "surface",
                "background",
                "surface_container_lowest",
                "surfaceContainerLowest",
                "mSurface",
            ],
        )?,
        material_color(
            value,
            &[
                "on_surface",
                "onSurface",
                "on_background",
                "onBackground",
                "mOnSurface",
            ],
        )?,
        material_color(value, &["primary", "mPrimary"])?,
        material_color(
            value,
            &[
                "secondary",
                "primary_container",
                "primaryContainer",
                "mSecondary",
            ],
        )?,
        material_color(
            value,
            &[
                "tertiary",
                "secondary_container",
                "secondaryContainer",
                "mTertiary",
            ],
        )?,
        material_color(value, &["error", "mError"])?,
        material_color(
            value,
            &[
                "outline",
                "outline_variant",
                "outlineVariant",
                "surface_variant",
                "surfaceVariant",
                "mOutline",
            ],
        )?,
    ))
}

fn material_color(value: &JsonValue, names: &[&str]) -> Option<Rgb> {
    let mut roots = vec![value];
    for key in ["colors", "schemes", "md3", "palette"] {
        if let Some(root) = value.get(key) {
            roots.push(root);
        }
    }

    for root in roots {
        for name in names {
            if let Some(color) = root.get(*name).and_then(parse_color_node) {
                return Some(color);
            }
            for mode in ["default", "dark", "light"] {
                if let Some(color) = root
                    .get(mode)
                    .and_then(|scheme| scheme.get(*name))
                    .and_then(parse_color_node)
                {
                    return Some(color);
                }
                if let Some(color) = root
                    .get(*name)
                    .and_then(|token| token.get(mode))
                    .and_then(parse_color_node)
                {
                    return Some(color);
                }
            }
        }
    }

    None
}

fn parse_color_node(value: &JsonValue) -> Option<Rgb> {
    value
        .as_str()
        .and_then(Rgb::parse)
        .or_else(|| {
            value
                .get("hex")
                .and_then(|value| value.as_str())
                .and_then(Rgb::parse)
        })
        .or_else(|| {
            value
                .get("value")
                .and_then(|value| value.as_str())
                .and_then(Rgb::parse)
        })
}

fn read_omarchy_theme() -> Option<Theme> {
    let path = dirs::state_dir()?.join("omarchy/current/theme/colors.toml");
    let contents = fs::read_to_string(path).ok()?;
    let value: TomlValue = toml::from_str(&contents).ok()?;
    Some(Theme::from_palette(
        toml_color(&value, &["background"])?,
        toml_color(&value, &["foreground"])?,
        toml_color(&value, &["accent"])?,
        toml_color(&value, &["blue"])
            .or_else(|| toml_color(&value, &["cyan"]))
            .unwrap_or_else(|| Rgb::new(167, 139, 250)),
        toml_color(&value, &["magenta"])
            .or_else(|| toml_color(&value, &["yellow"]))
            .unwrap_or_else(|| Rgb::new(255, 73, 198)),
        toml_color(&value, &["red"]).unwrap_or_else(|| Rgb::new(255, 83, 112)),
        toml_color(&value, &["muted"]).unwrap_or_else(|| Rgb::new(88, 110, 130)),
    ))
}

fn json_color(value: &JsonValue, path: &[&str]) -> Option<Rgb> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().and_then(Rgb::parse)
}

fn toml_color(value: &TomlValue, path: &[&str]) -> Option<Rgb> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str().and_then(Rgb::parse)
}

#[cfg(test)]
mod tests {
    use super::{Color, JsonValue, Rgb, theme_from_material_value};

    #[test]
    fn parses_nested_matugen_dark_colors() {
        let value: JsonValue = serde_json::json!({
            "colors": {
                "dark": {
                    "surface": "#101112",
                    "on_surface": "#f0f1f2",
                    "primary": "#7dd3fc",
                    "secondary": "#c084fc",
                    "tertiary": "#f9a8d4",
                    "error": "#f87171",
                    "outline": "#64748b"
                }
            }
        });

        let theme = theme_from_material_value(&value).unwrap();

        assert_eq!(theme.bg, Color::Rgb(16, 17, 18));
        assert_eq!(theme.text, Color::Rgb(240, 241, 242));
        assert_eq!(theme.primary, Color::Rgb(125, 211, 252));
    }

    #[test]
    fn parses_direct_skwd_style_colors() {
        let value: JsonValue = serde_json::json!({
            "colors": {
                "background": "#0b0c0dff",
                "onBackground": "#eff6ffff",
                "primary": { "default": { "hex": "#38bdf8" } },
                "secondary": "#a78bfa",
                "tertiary": "#f472b6",
                "error": "#fb7185",
                "outlineVariant": "#475569"
            }
        });

        let theme = theme_from_material_value(&value).unwrap();

        assert_eq!(theme.bg, Color::Rgb(11, 12, 13));
        assert_eq!(theme.primary, Color::Rgb(56, 189, 248));
        assert_eq!(theme.danger, Color::Rgb(251, 113, 133));
    }

    #[test]
    fn parses_alpha_hex_by_ignoring_alpha_channel() {
        assert_eq!(
            Rgb::parse("#112233cc").unwrap().color(),
            Color::Rgb(17, 34, 51)
        );
    }
}
