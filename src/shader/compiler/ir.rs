use super::opt;
use super::syntax::{ShaderSource, UsagiShaderModule};

/// Initial backend-neutral compiler boundary.
///
/// Today this wraps the parsed syntax module after validation. As semantic
/// checking grows, this module should own the checked ABT/IR that GLSL and
/// future non-GLSL backends consume.
pub(super) struct ShaderIr<'module, 'src> {
    module: &'module UsagiShaderModule<'src>,
    source: ShaderSource,
}

pub(super) fn lower<'module, 'src>(
    module: &'module UsagiShaderModule<'src>,
) -> ShaderIr<'module, 'src> {
    ShaderIr {
        module,
        source: opt::optimized_source(module),
    }
}

impl<'module, 'src> ShaderIr<'module, 'src> {
    pub(super) fn module(&self) -> &'module UsagiShaderModule<'src> {
        self.module
    }

    pub(super) fn source(&self) -> &ShaderSource {
        &self.source
    }
}
