//! Shader compilation helpers. LiquidGlass does not own the GL context here —
//! the host (an example sandbox today, a platform adapter eventually) creates
//! it and hands us a current `glow::Context` per the FROZEN context-ownership
//! rule.

use glow::HasContext;

/// Compiles and links a vertex/fragment pair. The fragment sources are the
/// same `glass.frag` / `glass_mask.frag` / `blur.frag` files the Macroquad
/// renderer uses — only the vertex stage differs, because it is pipeline
/// plumbing, not material behavior.
pub unsafe fn compile_program(
    gl: &glow::Context,
    vertex_src: &str,
    fragment_src: &str,
) -> Result<glow::NativeProgram, String> {
    unsafe {
        let vertex = compile_stage(gl, glow::VERTEX_SHADER, vertex_src)?;
        let fragment = compile_stage(gl, glow::FRAGMENT_SHADER, fragment_src)?;

        let program = gl.create_program().map_err(|e| format!("create_program: {e}"))?;
        gl.attach_shader(program, vertex);
        gl.attach_shader(program, fragment);
        gl.link_program(program);

        gl.delete_shader(vertex);
        gl.delete_shader(fragment);

        if !gl.get_program_link_status(program) {
            let log = gl.get_program_info_log(program);
            gl.delete_program(program);
            return Err(format!("program link failed: {log}"));
        }
        Ok(program)
    }
}

unsafe fn compile_stage(
    gl: &glow::Context,
    stage: u32,
    src: &str,
) -> Result<glow::NativeShader, String> {
    unsafe {
        let shader = gl.create_shader(stage).map_err(|e| format!("create_shader: {e}"))?;
        gl.shader_source(shader, src);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            let log = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(format!("shader compile failed: {log}"));
        }
        Ok(shader)
    }
}
