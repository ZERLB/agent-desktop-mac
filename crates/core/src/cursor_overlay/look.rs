use serde::{Deserialize, Serialize};

/// How the built-in cursor arrow is drawn. `Solid` paints the body and the outline in
/// opaque colours. `Glass` draws a translucent body tinted with the fill colour, a
/// light outline and a soft shadow, so what lies under the cursor stays visible.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorLook {
    #[default]
    Solid,
    Glass,
}

impl CursorLook {
    /// The look a `--look` value names, ignoring case.
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "solid" => Some(Self::Solid),
            "glass" => Some(Self::Glass),
            _ => None,
        }
    }

    pub const fn is_solid(&self) -> bool {
        matches!(self, Self::Solid)
    }

    pub const fn is_glass(&self) -> bool {
        matches!(self, Self::Glass)
    }
}

#[cfg(test)]
#[path = "look_tests.rs"]
mod tests;
