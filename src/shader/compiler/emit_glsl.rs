use super::ShaderProfile;
use super::ir::ShaderIr;
use super::syntax::{IntrinsicKind, ShaderSource, SourceRewrite, Token};

pub(super) fn emit(ir: &ShaderIr<'_, '_>, profile: ShaderProfile) -> Result<String, String> {
    TargetEmitter { profile }.emit(ir)
}

struct TargetEmitter {
    profile: ShaderProfile,
}

impl TargetEmitter {
    fn emit(&self, ir: &ShaderIr<'_, '_>) -> Result<String, String> {
        let module = ir.module();
        let target = target(self.profile);
        let header = target.header();
        let footer = target.footer(module.entrypoint_name);
        let mut out = String::with_capacity(
            header.len() + source_len(&module.tokens) + footer.len() + module.items.len() * 2,
        );
        out.push_str(&header);
        emit_source(&module.tokens, &module.source, target, &mut out)?;
        out.push_str(&footer);
        Ok(out)
    }
}

pub(super) fn target(profile: ShaderProfile) -> &'static GlslTarget {
    match profile {
        ShaderProfile::DesktopGlsl330 => &GLSL_330,
        ShaderProfile::DesktopGlsl440 => &GLSL_440,
        ShaderProfile::WebGlslEs100 => &GLSL_ES_100,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GlslTarget {
    pub(super) name: &'static str,
    version_directive: &'static str,
    precision_directive: Option<&'static str>,
    varying_qualifier: &'static str,
    output_declaration: Option<&'static str>,
    fragment_output: &'static str,
    texture_function: &'static str,
    pub(super) supports_desktop_interface_qualifiers: bool,
    pub(super) supports_es_varying_qualifier: bool,
    pub(super) supports_layout_qualifier: bool,
    pub(super) supports_precision_directive: bool,
}

impl GlslTarget {
    fn header(self) -> String {
        let mut out = String::with_capacity(144);
        out.push_str(self.version_directive);
        out.push_str("\n\n");
        if let Some(precision) = self.precision_directive {
            out.push_str(precision);
            out.push_str("\n\n");
        }
        out.push_str(self.varying_qualifier);
        out.push_str(" vec2 fragTexCoord;\n");
        out.push_str(self.varying_qualifier);
        out.push_str(" vec4 fragColor;\n");
        out.push_str("uniform sampler2D texture0;\n");
        if let Some(output) = self.output_declaration {
            out.push_str(output);
            out.push('\n');
        }
        out.push('\n');
        out
    }

    fn footer(self, entrypoint: &str) -> String {
        format!(
            "\n\nvoid main() {{\n    {} = {entrypoint}(fragTexCoord, fragColor);\n}}\n",
            self.fragment_output
        )
    }
}

const GLSL_ES_100: GlslTarget = GlslTarget {
    name: "GLSL ES 100",
    version_directive: "#version 100",
    precision_directive: Some("precision mediump float;"),
    varying_qualifier: "varying",
    output_declaration: None,
    fragment_output: "gl_FragColor",
    texture_function: "texture2D",
    supports_desktop_interface_qualifiers: false,
    supports_es_varying_qualifier: true,
    supports_layout_qualifier: false,
    supports_precision_directive: true,
};

const GLSL_330: GlslTarget = GlslTarget {
    name: "GLSL 330",
    version_directive: "#version 330",
    precision_directive: None,
    varying_qualifier: "in",
    output_declaration: Some("out vec4 finalColor;"),
    fragment_output: "finalColor",
    texture_function: "texture",
    supports_desktop_interface_qualifiers: true,
    supports_es_varying_qualifier: false,
    supports_layout_qualifier: false,
    supports_precision_directive: false,
};

const GLSL_440: GlslTarget = GlslTarget {
    name: "GLSL 440",
    version_directive: "#version 440 core",
    precision_directive: None,
    varying_qualifier: "in",
    output_declaration: Some("layout(location = 0) out vec4 finalColor;"),
    fragment_output: "finalColor",
    texture_function: "texture",
    supports_desktop_interface_qualifiers: true,
    supports_es_varying_qualifier: false,
    supports_layout_qualifier: true,
    supports_precision_directive: false,
};

fn emit_source(
    tokens: &[Token<'_>],
    source: &ShaderSource,
    target: &GlslTarget,
    out: &mut String,
) -> Result<(), String> {
    let mut rewrite_idx = 0usize;
    let mut token_idx = 0usize;

    while token_idx < tokens.len() {
        if let Some(rewrite) = source.rewrites.get(rewrite_idx)
            && rewrite.name_idx == token_idx
        {
            emit_source_rewrite(tokens, rewrite, target, out);
            token_idx = rewrite.close_idx + 1;
            rewrite_idx += 1;
            continue;
        }

        out.push_str(tokens[token_idx].text);
        token_idx += 1;
    }
    Ok(())
}

fn emit_source_rewrite(
    tokens: &[Token<'_>],
    rewrite: &SourceRewrite,
    target: &GlslTarget,
    out: &mut String,
) {
    match rewrite.kind {
        IntrinsicKind::Texture => out.push_str(target.texture_function),
    }
    for token in &tokens[rewrite.name_idx + 1..=rewrite.close_idx] {
        out.push_str(token.text);
    }
}

fn source_len(tokens: &[Token<'_>]) -> usize {
    tokens.iter().map(|token| token.text.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::super::ir;
    use super::super::syntax::UsagiShaderModule;
    use super::*;

    const GOLDEN_SRC: &str = concat!(
        "#usagi shader 1\n\n",
        "uniform float u_time;\n",
        "vec4 usagi_main(vec2 uv, vec4 color) {\n",
        "    return usagi_texture(texture0, uv) * color * u_time;\n",
        "}\n",
    );

    fn emit_fragment(src: &str, profile: ShaderProfile) -> Result<String, String> {
        let module = UsagiShaderModule::parse(src)?;
        let ir = ir::lower(&module);
        emit(&ir, profile)
    }

    #[test]
    fn emitter_lowers_texture_intrinsic_without_macros() {
        let src = "#usagi shader 1\nvec4 usagi_main(vec2 uv, vec4 color) {\n    return usagi_texture(texture0, uv) * color;\n}\n";
        let out = emit_fragment(src, ShaderProfile::DesktopGlsl330).unwrap();

        assert!(out.contains("#version 330"));
        assert!(out.contains("return texture(texture0, uv) * color;"));
        assert!(!out.contains("#define usagi_texture"));
        assert!(out.contains("finalColor = usagi_main(fragTexCoord, fragColor);"));
    }

    #[test]
    fn emitter_uses_glsl_es_100_texture2d() {
        let src = "vec4 usagi_main(vec2 uv, vec4 color) {\n    return usagi_texture(texture0, uv) * color;\n}\n";
        let out = emit_fragment(src, ShaderProfile::WebGlslEs100).unwrap();

        assert!(out.contains("#version 100"));
        assert!(out.contains("precision mediump float;"));
        assert!(out.contains("return texture2D(texture0, uv) * color;"));
        assert!(out.contains("gl_FragColor = usagi_main(fragTexCoord, fragColor);"));
    }

    #[test]
    fn emitter_has_forward_glsl_440_profile() {
        let src = "vec4 usagi_main(vec2 uv, vec4 color) { return color; }\n";
        let out = emit_fragment(src, ShaderProfile::DesktopGlsl440).unwrap();

        assert!(out.contains("#version 440 core"));
        assert!(out.contains("layout(location = 0) out vec4 finalColor;"));
        assert!(out.contains("finalColor = usagi_main(fragTexCoord, fragColor);"));
    }

    #[test]
    fn emitter_golden_output_glsl_es_100() {
        let out = emit_fragment(GOLDEN_SRC, ShaderProfile::WebGlslEs100).unwrap();

        assert_eq!(
            out,
            concat!(
                "#version 100\n\n",
                "precision mediump float;\n\n",
                "varying vec2 fragTexCoord;\n",
                "varying vec4 fragColor;\n",
                "uniform sampler2D texture0;\n\n",
                "uniform float u_time;\n",
                "vec4 usagi_main(vec2 uv, vec4 color) {\n",
                "    return texture2D(texture0, uv) * color * u_time;\n",
                "}\n",
                "\n\n",
                "void main() {\n",
                "    gl_FragColor = usagi_main(fragTexCoord, fragColor);\n",
                "}\n",
            )
        );
    }

    #[test]
    fn emitter_golden_output_glsl_330() {
        let out = emit_fragment(GOLDEN_SRC, ShaderProfile::DesktopGlsl330).unwrap();

        assert_eq!(
            out,
            concat!(
                "#version 330\n\n",
                "in vec2 fragTexCoord;\n",
                "in vec4 fragColor;\n",
                "uniform sampler2D texture0;\n",
                "out vec4 finalColor;\n\n",
                "uniform float u_time;\n",
                "vec4 usagi_main(vec2 uv, vec4 color) {\n",
                "    return texture(texture0, uv) * color * u_time;\n",
                "}\n",
                "\n\n",
                "void main() {\n",
                "    finalColor = usagi_main(fragTexCoord, fragColor);\n",
                "}\n",
            )
        );
    }

    #[test]
    fn emitter_golden_output_glsl_440() {
        let out = emit_fragment(GOLDEN_SRC, ShaderProfile::DesktopGlsl440).unwrap();

        assert_eq!(
            out,
            concat!(
                "#version 440 core\n\n",
                "in vec2 fragTexCoord;\n",
                "in vec4 fragColor;\n",
                "uniform sampler2D texture0;\n",
                "layout(location = 0) out vec4 finalColor;\n\n",
                "uniform float u_time;\n",
                "vec4 usagi_main(vec2 uv, vec4 color) {\n",
                "    return texture(texture0, uv) * color * u_time;\n",
                "}\n",
                "\n\n",
                "void main() {\n",
                "    finalColor = usagi_main(fragTexCoord, fragColor);\n",
                "}\n",
            )
        );
    }
}
