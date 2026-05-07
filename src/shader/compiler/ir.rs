use super::syntax::UsagiShaderModule;

/// Initial backend-neutral compiler boundary.
///
/// Today this wraps the parsed syntax module after validation. As semantic
/// checking grows, this module should own the checked ABT/IR that GLSL and
/// future non-GLSL backends consume.
pub(super) struct ShaderIr<'module, 'src> {
    module: &'module UsagiShaderModule<'src>,
}

pub(super) fn lower<'module, 'src>(
    module: &'module UsagiShaderModule<'src>,
) -> ShaderIr<'module, 'src> {
    ShaderIr { module }
}

impl<'module, 'src> ShaderIr<'module, 'src> {
    pub(super) fn module(&self) -> &'module UsagiShaderModule<'src> {
        self.module
    }
}
