use super::shader::{
    Block, Function, Program, Statement, Type, Variable, VariableAllocationContext,
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

        if let Some(frag) = self.frag_shader() {
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
        let vert = if let Some(vert) = self.vertex_main() {
            vert
        } else {
            return Ok(String::new());
        };

        let mut shader = String::new();

        global_output(&mut shader, "uniform", self.used_uniforms(vert).into_iter())?;
        global_output_loc(
            &mut shader,
            "in",
            vert.inputs().into_iter(),
            &Self::calc_variable_locations(vert.inputs())?,
        )?;
        global_output_loc(
            &mut shader,
            "out",
            vert.outputs().into_iter(),
            &Self::calc_variable_locations(vert.outputs())?,
        )?;

        Ok(shader)
    }

    fn frag_shader(&self) -> Option<String> {
        None
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
