use super::emit_glsl::{self, GlslTarget};
use super::syntax::{Token, UsagiShaderModule, is_code_token};
use super::{CompileError, CompileResult, ShaderProfile};

pub(super) fn validate(
    module: &UsagiShaderModule<'_>,
    profile: ShaderProfile,
) -> CompileResult<()> {
    validate_target_tokens(&module.tokens, emit_glsl::target(profile))
}

fn validate_target_tokens(tokens: &[Token<'_>], target: &GlslTarget) -> CompileResult<()> {
    for token in tokens {
        if !is_code_token(token) {
            continue;
        }
        match token.text {
            "in" | "out" if !target.supports_desktop_interface_qualifiers => {
                return Err(CompileError::at(
                    format!(
                        "{} generic shaders do not support desktop interface qualifier '{}'",
                        target.name, token.text
                    ),
                    token.span,
                ));
            }
            "varying" if !target.supports_es_varying_qualifier => {
                return Err(CompileError::at(
                    format!(
                        "{} generic shaders do not support GLSL ES interface qualifier 'varying'",
                        target.name
                    ),
                    token.span,
                ));
            }
            "layout" if !target.supports_layout_qualifier => {
                return Err(CompileError::at(
                    format!(
                        "{} generic shaders do not support layout qualifiers",
                        target.name
                    ),
                    token.span,
                ));
            }
            "precision" if !target.supports_precision_directive => {
                return Err(CompileError::at(
                    format!(
                        "{} generic shaders do not support precision declarations",
                        target.name
                    ),
                    token.span,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::syntax::UsagiShaderModule;
    use super::*;

    fn validate_fragment(src: &str, profile: ShaderProfile) -> Result<(), String> {
        let module = UsagiShaderModule::parse(src)?;
        validate(&module, profile).map_err(|err| err.render(src, module.source_offset))
    }

    #[test]
    fn target_validation_rejects_desktop_interface_qualifier_for_es_100() {
        let err = validate_fragment(
            "out vec4 customColor;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::WebGlslEs100,
        )
        .unwrap_err();

        assert!(err.contains("GLSL ES 100"));
        assert!(err.contains("desktop interface qualifier 'out'"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_es_varying_qualifier_for_desktop() {
        let err = validate_fragment(
            "varying vec2 customUv;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("GLSL 330"));
        assert!(err.contains("varying"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_layout_qualifier_for_330() {
        let err = validate_fragment(
            "layout(location = 0) out vec4 customColor;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl330,
        )
        .unwrap_err();

        assert!(err.contains("GLSL 330"));
        assert!(err.contains("layout qualifiers"));
        assert!(err.contains("line 1, column 1"));
    }

    #[test]
    fn target_validation_rejects_precision_declaration_for_desktop() {
        let err = validate_fragment(
            "precision mediump float;\nvec4 usagi_main(vec2 uv, vec4 color) { return color; }\n",
            ShaderProfile::DesktopGlsl440,
        )
        .unwrap_err();

        assert!(err.contains("GLSL 440"));
        assert!(err.contains("precision declarations"));
        assert!(err.contains("line 1, column 1"));
    }
}
