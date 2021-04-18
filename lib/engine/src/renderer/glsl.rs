use super::shader::{
    Block, BuiltinFunction, Function, Program, Statement, Type, Variable,
    VariableAllocationContext, VariableId,
};
use thiserror::Error;

#[derive(Debug, Error)]
enum GlslError {
    #[error("Unable to represent the type {ty} in glsl")]
    UnreprsentableType { ty: Type },
}

impl Program {
    pub(super) fn to_glsl(mut self) -> Result<(), anyhow::Error> {
        self.glsl_verification()?;

        if let Ok(vert) = self.vert_shader() {
            println!("{}", vert);
        }

        if let Ok(frag) = self.frag_shader() {
            println!("{}", frag);
        }

        Ok(())
    }

    fn glsl_verification(&self) -> Result<(), GlslError> {
        for uniform in self.uniforms() {
            if !uniform.ty.is_representable() {
                return Err(GlslError::UnreprsentableType { ty: uniform.ty });
            }
        }

        for func in self.functions() {
            for var in func.all_vars() {
                if !var.ty.is_representable() {
                    return Err(GlslError::UnreprsentableType { ty: var.ty });
                }
            }
        }

        Ok(())
    }

    fn vert_shader(&mut self) -> Result<String, GlslError> {
        let shader = if let Some(vert) = self.vertex_main() {
            vert
        } else {
            return Ok(String::new());
        };

        self.write_shader(shader)
    }

    fn frag_shader(&self) -> Result<String, GlslError> {
        let shader = if let Some(frag) = self.frag_main() {
            frag
        } else {
            return Ok(String::new());
        };

        self.write_shader(shader)
    }

    fn used_uniforms(&self, func: &Function) -> Vec<&Variable> {
        let mut uniforms = vec![];
        for block in func.blocks() {
            self.block_uniforms(&mut uniforms, block);
        }

        uniforms
    }

    fn block_uniforms<'a>(&'a self, uniforms: &mut Vec<&'a Variable>, block: &Block) {
        for statement in block.statements() {
            match statement {
                Statement::CallBuiltin { arguments, .. } => {
                    for arg in arguments {
                        if arg.allocation_kind() == VariableAllocationContext::Uniform {
                            uniforms.push(&self.uniforms()[arg.id()])
                        }
                    }
                }
                _ => (),
            }
        }
    }

    fn write_shader(&self, shader: &Function) -> Result<String, GlslError> {
        let mut source = String::new();

        global_output(
            &mut source,
            "uniform",
            self.used_uniforms(shader).into_iter(),
        )?;
        global_output(&mut source, "in", shader.inputs().into_iter())?;
        global_output(&mut source, "out", shader.outputs().into_iter())?;

        source.push_str("void main() {\n");
        write_func(&mut source, self, shader);
        source.push_str("}\n");

        Ok(source)
    }
}

impl Type {
    fn is_representable(&self) -> bool {
        match self {
            Type::Vector(n) => *n <= 4,
            Type::Matrix(m, n) => *m <= 4 && *n <= 4,
            Type::Floating => true,
            Type::Sampler2D => true,
            Type::Unknown => false,
        }
    }

    fn to_glsl(&self) -> String {
        match self {
            Type::Vector(n) => format!("vec{}", *n),
            Type::Matrix(rows, cols) => {
                if rows == cols {
                    format!("mat{}", rows)
                } else {
                    format!("mat{}x{}", rows, cols)
                }
            }
            Type::Floating => "float".to_string(),
            Type::Sampler2D => "sampler2D".to_string(),
            Type::Unknown => "".to_string(), // should not occur
        }
    }
}

fn global_output<'a>(
    out: &mut String,
    kind: &str,
    vars: impl Iterator<Item = &'a Variable>,
) -> Result<(), GlslError> {
    for var in vars {
        out.push_str(&format!("{} {} {};\n", kind, var.ty.to_glsl(), var.name));
    }

    Ok(())
}

fn write_func(shader: &mut String, prog: &Program, func: &Function) {
    for block in func.blocks() {
        for statement in block.statements() {
            match statement {
                Statement::CallBuiltin {
                    function,
                    arguments,
                    result,
                } => {
                    write_builtin_call(shader, prog, func, function, arguments, result);
                }
                Statement::MakeFloat { value, variable } => {
                    write_variable_new(shader, prog, func, *variable);
                    shader.push_str(" = ");
                    shader.push_str(&format!("{:.20}", value));
                    shader.push_str(";\n");
                }
                Statement::SetBuiltinVariable { variable, value } => {
                    shader.push_str("    ");
                    shader.push_str(&variable.to_string());
                    shader.push_str(" = ");
                    write_variable_get(shader, prog, func, *value);
                    shader.push_str(";\n");
                }
                Statement::GetBuiltinVariable { variable, result } => {
                    write_variable_new(shader, prog, func, *result);
                    shader.push_str(" = ");
                    shader.push_str(&variable.to_string());
                    shader.push_str(";\n");
                }
            }
        }
    }
}

fn write_variable_new(shader: &mut String, prog: &Program, func: &Function, variable: VariableId) {
    shader.push_str("    ");

    let variable_ref = prog.get_variable(func, variable);
    shader.push_str(&variable_ref.ty.to_glsl());
    shader.push_str(" ");
    write_variable_name(shader, variable_ref, variable.id());
}

fn write_variable_get(shader: &mut String, prog: &Program, func: &Function, variable: VariableId) {
    write_variable_name(shader, prog.get_variable(func, variable), variable.id());
}

fn write_variable_name(shader: &mut String, var: &Variable, id: usize) {
    if var.name.is_empty() {
        shader.push_str(&format!("var_{}", id));
    } else {
        shader.push_str(&var.name);
    }
}

fn write_builtin_call(
    shader: &mut String,
    prog: &Program,
    func: &Function,
    function: &BuiltinFunction,
    arguments: &[VariableId],
    result: &Option<VariableId>,
) {
    if let Some(result) = result {
        write_variable_new(shader, prog, func, *result);
        shader.push_str(" = ");
    } else {
        shader.push_str("    ");
    }

    if let Some(op) = match function {
        BuiltinFunction::Add => Some("+"),
        BuiltinFunction::Div => Some("/"),
        BuiltinFunction::Mul => Some("*"),
        BuiltinFunction::Sub => Some("-"),
        _ => None,
    } {
        write_variable_get(shader, prog, func, arguments[0]);
        shader.push_str(&format!(" {} ", op));
        write_variable_get(shader, prog, func, arguments[1]);
        shader.push_str(";\n");
        return;
    }

    match function {
        BuiltinFunction::Texture => {
            shader.push_str("texture(");
            write_variable_get(shader, prog, func, arguments[0]);
            shader.push_str(", ");
            write_variable_get(shader, prog, func, arguments[1]);
            shader.push_str(");\n");
        }
        BuiltinFunction::SetGlobal => {
            write_variable_get(shader, prog, func, arguments[0]);
            shader.push_str(" = ");
            write_variable_get(shader, prog, func, arguments[1]);
            shader.push_str(";\n");
        }
        BuiltinFunction::MakeVec => {
            if let Some(result) = result {
                let result = prog.get_variable(func, *result);

                shader.push_str(&format!("{}(", result.ty));
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        shader.push_str(", ");
                    }
                    write_variable_get(shader, prog, func, *arg);
                }
                shader.push_str(");\n");
            }
        }
        _ => (),
    }
}
