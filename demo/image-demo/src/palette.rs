use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

pub const OPAQUE: u8 = 255;
pub const BLUSH_ALPHA: u8 = 70;
pub const HAIR_SHINE_ALPHA: u8 = 110;

impl Rgba {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, OPAQUE)
    }

    pub fn from_hex(hex: &str) -> Result<Self, ParseColorError> {
        let body = hex.strip_prefix('#').unwrap_or(hex);
        if body.len() != 6 && body.len() != 8 {
            return Err(ParseColorError::Length(hex.to_string()));
        }
        let rgb = (0..6)
            .step_by(2)
            .map(|i| u8::from_str_radix(&body[i..i + 2], 16))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ParseColorError::Digit(hex.to_string()))?;
        let alpha = if body.len() == 8 {
            u8::from_str_radix(&body[6..8], 16)
                .map_err(|_| ParseColorError::Digit(hex.to_string()))?
        } else {
            OPAQUE
        };
        Ok(Self::new(rgb[0], rgb[1], rgb[2], alpha))
    }

    pub fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    pub fn components(self) -> [f32; 4] {
        [
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
            f32::from(self.a) / 255.0,
        ]
    }

    pub fn to_svg(self) -> String {
        if self.a == OPAQUE {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    pub fn darken(self, amount: f32) -> Self {
        let clamp = |v: f32| v.max(0.0).round() as u8;
        Self::new(
            clamp(f32::from(self.r) * (1.0 - amount)),
            clamp(f32::from(self.g) * (1.0 - amount)),
            clamp(f32::from(self.b) * (1.0 - amount)),
            self.a,
        )
    }

    pub fn lerp_to(self, target: Self, t: f32) -> Self {
        let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        Self::new(
            mix(self.r, target.r),
            mix(self.g, target.g),
            mix(self.b, target.b),
            self.a,
        )
    }

    pub fn lighten(self, amount: f32) -> Self {
        let clamp = |v: f32| v.min(255.0).round() as u8;
        let mix = |c: u8| f32::from(c) + (255.0 - f32::from(c)) * amount;
        Self::new(
            clamp(mix(self.r)),
            clamp(mix(self.g)),
            clamp(mix(self.b)),
            self.a,
        )
    }
}

#[derive(Debug)]
pub enum ParseColorError {
    Length(String),
    Digit(String),
}

impl fmt::Display for ParseColorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(s) => write!(f, "hex color `{s}` must have 6 or 8 digits"),
            Self::Digit(s) => write!(f, "hex color `{s}` has non-hex digits"),
        }
    }
}

impl std::error::Error for ParseColorError {}

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub skin: Rgba,
    pub skin_shade: Rgba,
    pub hair_main: Rgba,
    pub hair_shade: Rgba,
    pub hair_shine: Rgba,
    pub eye_iris: Rgba,
    pub eye_white: Rgba,
    pub clothes_main: Rgba,
    pub clothes_shade: Rgba,
    pub clothes_accent: Rgba,
    pub line: Rgba,
    pub blush: Rgba,
    pub bg_top: Rgba,
    pub bg_bottom: Rgba,
}

impl Palette {
    pub fn from_hexes(values: [&str; 14]) -> Result<Self, ParseColorError> {
        let mut it = values.into_iter().map(Rgba::from_hex);
        let mut next = || it.next().transpose();
        let mut take = || {
            next()?.ok_or(ParseColorError::Length(String::from(
                "missing palette entry",
            )))
        };
        Ok(Self {
            skin: take()?,
            skin_shade: take()?,
            hair_main: take()?,
            hair_shade: take()?,
            hair_shine: take()?,
            eye_iris: take()?,
            eye_white: take()?,
            clothes_main: take()?,
            clothes_shade: take()?,
            clothes_accent: take()?,
            line: take()?,
            blush: take()?,
            bg_top: take()?,
            bg_bottom: take()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_six_digit_hex() {
        let c = Rgba::from_hex("#ff8800").expect("valid hex");
        assert_eq!(c, Rgba::opaque(255, 136, 0));
    }

    #[test]
    fn parses_eight_digit_hex_without_prefix() {
        let c = Rgba::from_hex("ff880040").expect("valid hex");
        assert_eq!(c.a, 0x40);
    }

    #[test]
    fn rejects_bad_length() {
        assert!(Rgba::from_hex("#fff").is_err());
    }

    #[test]
    fn rejects_bad_digit() {
        assert!(Rgba::from_hex("#zz8800").is_err());
    }

    #[test]
    fn darken_lighten_stay_in_range() {
        let c = Rgba::opaque(10, 128, 240);
        assert!(c.darken(0.5).g < 128);
        assert!(c.lighten(0.5).r > 10);
        assert_eq!(c.lighten(1.0), Rgba::opaque(255, 255, 255));
    }

    #[test]
    fn palette_from_hexes_roundtrip() {
        let p = Palette::from_hexes([
            "#ffd9c0", "#f0b9a0", "#5a3866", "#3d2547", "#8a6ba0", "#4a90d9", "#ffffff", "#ff7eb6",
            "#d94f8e", "#ffe066", "#33202a", "#ff9aa8", "#ffe9d6", "#ffc9a3",
        ])
        .expect("valid palette");
        assert_eq!(p.skin, Rgba::opaque(0xff, 0xd9, 0xc0));
    }
}
