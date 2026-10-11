use crate::background::BackgroundKind;
use crate::character::{Accessory, HairStyle};
use crate::expression::Expression;
use crate::palette::Palette;
use crate::pose::Pose;

#[derive(Clone, Copy, Debug)]
pub struct CharacterConfig {
    pub palette: Palette,
    pub expression: Expression,
    pub pose: Pose,
    pub background: BackgroundKind,
    pub hair: HairStyle,
    pub accessory: Accessory,
}

impl Default for CharacterConfig {
    fn default() -> Self {
        Self {
            palette: preset_sakura(),
            expression: Expression::Happy,
            pose: Pose::Wave,
            background: BackgroundKind::Rays,
            hair: HairStyle::TwinTails,
            accessory: Accessory::StarPin,
        }
    }
}

pub const fn preset_sakura() -> Palette {
    Palette {
        skin: crate::palette::Rgba::opaque(0xff, 0xe3, 0xcf),
        skin_shade: crate::palette::Rgba::opaque(0xf3, 0xc4, 0xad),
        hair_main: crate::palette::Rgba::opaque(0xf5, 0x8f, 0xb8),
        hair_shade: crate::palette::Rgba::opaque(0xd9, 0x6a, 0x9a),
        hair_shine: crate::palette::Rgba::opaque(0xff, 0xd1, 0xe3),
        eye_iris: crate::palette::Rgba::opaque(0x6c, 0x4f, 0xbb),
        eye_white: crate::palette::Rgba::opaque(0xff, 0xff, 0xff),
        clothes_main: crate::palette::Rgba::opaque(0xff, 0x9e, 0xc6),
        clothes_shade: crate::palette::Rgba::opaque(0xe0, 0x6f, 0x9f),
        clothes_accent: crate::palette::Rgba::opaque(0xff, 0xe0, 0x6b),
        line: crate::palette::Rgba::opaque(0x3a, 0x24, 0x33),
        blush: crate::palette::Rgba::opaque(0xff, 0x9e, 0xa8),
        bg_top: crate::palette::Rgba::opaque(0xff, 0xf0, 0xf4),
        bg_bottom: crate::palette::Rgba::opaque(0xff, 0xd1, 0xe0),
    }
}

pub fn preset_ocean() -> Palette {
    palette_from_hexes([
        "#ffe3cf", "#f3c4ad", "#3f7fd9", "#2c5aa8", "#8fc3ff", "#2ec4b6", "#ffffff", "#5aa9e6",
        "#3f7fd9", "#ffe066", "#232d3f", "#ff9aa8", "#dff3ff", "#9ad0ff",
    ])
    .expect("ocean hexes are valid")
}

pub fn preset_mint() -> Palette {
    palette_from_hexes([
        "#ffe3cf", "#f3c4ad", "#57cc99", "#38a3a5", "#c7f9cc", "#22577a", "#ffffff", "#80ed99",
        "#57cc99", "#ffd166", "#1f2d2a", "#ffa5ab", "#eafaf1", "#b8f2e6",
    ])
    .expect("mint hexes are valid")
}

pub fn preset_earthtone() -> Palette {
    palette_from_hexes([
        "#f2d6b3", "#e0b894", "#8c6a4f", "#6b4f3a", "#d9b98a", "#7a5c3e", "#faf3e3", "#c98f4e",
        "#a06f3d", "#e3b23c", "#3e2f23", "#d98f6a", "#e8dcc0", "#cbb89a",
    ])
    .expect("earthtone hexes are valid")
}

pub const fn preset_steins() -> Palette {
    Palette {
        skin: crate::palette::Rgba::opaque(0xff, 0xe8, 0xd6),
        skin_shade: crate::palette::Rgba::opaque(0xf0, 0xc8, 0xb0),
        hair_main: crate::palette::Rgba::opaque(0xc0, 0x3a, 0x2b),
        hair_shade: crate::palette::Rgba::opaque(0x8e, 0x24, 0x1c),
        hair_shine: crate::palette::Rgba::opaque(0xe8, 0x6a, 0x4a),
        eye_iris: crate::palette::Rgba::opaque(0x6b, 0x4f, 0xc7),
        eye_white: crate::palette::Rgba::opaque(0xff, 0xff, 0xff),
        clothes_main: crate::palette::Rgba::opaque(0xf5, 0xf2, 0xea),
        clothes_shade: crate::palette::Rgba::opaque(0xd8, 0xd2, 0xc4),
        clothes_accent: crate::palette::Rgba::opaque(0xa8, 0x24, 0x20),
        line: crate::palette::Rgba::opaque(0x1c, 0x18, 0x14),
        blush: crate::palette::Rgba::opaque(0xe8, 0x8a, 0x86),
        bg_top: crate::palette::Rgba::opaque(0xc9, 0xc2, 0xac),
        bg_bottom: crate::palette::Rgba::opaque(0x4a, 0x46, 0x38),
    }
}

pub fn preset_night() -> Palette {
    palette_from_hexes([
        "#ffe3cf", "#f3c4ad", "#8e7dbe", "#5d5390", "#c3b8ee", "#ffd166", "#ffffff", "#3d348b",
        "#2a2464", "#f7b801", "#191528", "#ff8fa3", "#191528", "#43397a",
    ])
    .expect("night hexes are valid")
}

pub fn palette_from_hexes(values: [&str; 14]) -> Result<Palette, crate::palette::ParseColorError> {
    Palette::from_hexes(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_valid_preset() {
        let cfg = CharacterConfig::default();
        assert_eq!(cfg.expression.name(), "happy");
        assert_eq!(cfg.hair.name(), "twin_tails");
    }

    #[test]
    fn steins_preset_matches_sample() {
        let p = preset_steins();
        assert_eq!(p.hair_main, crate::palette::Rgba::opaque(0xc0, 0x3a, 0x2b));
        assert_eq!(
            p.clothes_main,
            crate::palette::Rgba::opaque(0xf5, 0xf2, 0xea)
        );
        assert_eq!(p.line, crate::palette::Rgba::opaque(0x1c, 0x18, 0x14));
    }

    #[test]
    fn presets_parse() {
        assert_eq!(
            preset_ocean().hair_main,
            crate::palette::Rgba::opaque(0x3f, 0x7f, 0xd9)
        );
        assert_eq!(
            preset_mint().skin,
            crate::palette::Rgba::opaque(0xff, 0xe3, 0xcf)
        );
        assert_eq!(
            preset_night().bg_top,
            crate::palette::Rgba::opaque(0x19, 0x15, 0x28)
        );
    }
}
