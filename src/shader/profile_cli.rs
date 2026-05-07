//! Shared native CLI target selectors for shader tooling.

use super::ShaderProfile;
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ShaderProfileTarget {
    /// Use the desktop runtime target.
    Desktop,
    /// Use the web runtime target.
    Web,
    /// Use the staged desktop GLSL 440 target.
    #[value(name = "glsl440")]
    Glsl440,
    /// Use every supported generic shader profile.
    All,
}

impl ShaderProfileTarget {
    pub(crate) fn profiles(self) -> Vec<ShaderProfile> {
        match self {
            Self::Desktop => vec![ShaderProfile::DesktopGlsl330],
            Self::Web => vec![ShaderProfile::WebGlslEs100],
            Self::Glsl440 => vec![ShaderProfile::DesktopGlsl440],
            Self::All => ShaderProfile::ALL.to_vec(),
        }
    }

    pub(crate) fn is_all(self) -> bool {
        self == Self::All
    }
}
