use super::shader::{
    Block, BuiltinFunction, Function, Program, Statement, Type, Variable,
    VariableAllocationContext, VariableId,
};
use thiserror::Error;

#[derive(Debug, Error)]
enum GlslError {
    #[error("Cannot create shader: locations specified in shader overlap")]
    OverlappingLocations,

    #[error("Unable to represent the type {ty} in glsl")]
    UnreprsentableType { ty: Type },

    #[error("Trying to create shader with missing location, this should not be possible")]
    MissingLocation,
}

impl Program {
    pub(super) fn to_glsl(mut self) -> Result<(), anyhow::Error> {
        self.glsl_verification()?;

        Self::set_variable_locations(self.uniforms_mut())?;

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

    fn calc_variable_locations(vars: &[Variable]) -> Result<Vec<usize>, GlslError> {
        let mut locations = vec![0; vars.len()];
        let mut is_location_used = vec![];

        for (idx, variable) in vars.iter().enumerate() {
            if let Some(loc) = variable.start_location {
                let size = variable.ty.location_count();

                if is_location_used.len() < loc + size {
                    is_location_used.resize(loc + size, false);
                }

                for i in loc..(loc + size) {
                    if is_location_used[i] {
                        return Err(GlslError::OverlappingLocations);
                    } else {
                        is_location_used[i] = true;
                    }
                }

                locations[idx] = loc;
            }
        }

        for (idx, variable) in vars.iter().enumerate() {
            if variable.start_location == None {
                let size = variable.ty.location_count();
                let pattern = vec![false; size];
                if let Some(loc) = slice_find(&is_location_used, &pattern) {
                    for i in loc..(loc + size) {
                        is_location_used[i] = true;
                    }

                    locations[idx] = loc;
                } else {
                    locations[idx] = is_location_used.len();
                    is_location_used.resize(is_location_used.len() + size, true);
                }
            }
        }

        Ok(locations)
    }

    fn set_variable_locations(vars: &mut [Variable]) -> Result<(), GlslError> {
        let locs = Self::calc_variable_locations(vars)?;

        for (var, loc) in vars.iter_mut().zip(locs) {
            var.start_location = Some(loc);
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
        global_output_loc(
            &mut source,
            "in",
            shader.inputs().into_iter(),
            &Self::calc_variable_locations(shader.inputs())?,
        )?;
        global_output_loc(
            &mut source,
            "out",
            shader.outputs().into_iter(),
            &Self::calc_variable_locations(shader.outputs())?,
        )?;

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

    /// get the number of locations a type takes up
    /// the result is only valid if self.is_representable()
    fn location_count(&self) -> usize {
        // internaly a location is a vec4, scalars padded to 1 location,
        // mat4 takes 4 locations, etc

        match self {
            Type::Vector(_) => 1,
            Type::Matrix(_, n) => *n,
            Type::Floating => 1,
            Type::Sampler2D => 1,
            Type::Unknown => 0,
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

fn slice_find<T: PartialEq>(input: &[T], pattern: &[T]) -> Option<usize> {
    let pattern_len = pattern.len();
    let input_len = input.len();

    if pattern_len > input_len {
        return None;
    }

    if pattern_len == input_len {
        if input == pattern {
            return Some(0);
        } else {
            return None;
        }
    }

    for i in 0..(input_len - pattern_len) {
        if &input[i..(i + pattern_len)] == pattern {
            return Some(i);
        }
    }

    None
}

fn global_output<'a>(
    out: &mut String,
    kind: &str,
    vars: impl Iterator<Item = &'a Variable> + ExactSizeIterator,
) -> Result<(), GlslError> {
    global_output_loc(out, kind, vars, &[])
}

fn global_output_loc<'a>(
    out: &mut String,
    kind: &str,
    vars: impl Iterator<Item = &'a Variable> + ExactSizeIterator,
    locations: &[usize],
) -> Result<(), GlslError> {
    let var_len = vars.len();

    for (idx, var) in vars.enumerate() {
        let location = if locations.len() == var_len {
            locations[idx]
        } else {
            var.start_location.ok_or(GlslError::MissingLocation)?
        };

        out.push_str(&format!(
            "layout(location = {}) {} {} {};\n",
            location,
            kind,
            var.ty.to_glsl(),
            var.name,
        ));
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
