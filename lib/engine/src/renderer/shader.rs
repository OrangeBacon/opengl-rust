use std::{
    fmt::{self, Display, Formatter, Write},
    ops::{Deref, DerefMut},
};

use engine_proc_macro::context_globals;
use thiserror::Error;

// ================= //
// type declarations //
// ================= //

#[derive(Debug, Error)]
pub enum ShaderCreationError {
    #[error("Found multiple errors while compiling shader:\n{errors}")]
    ErrorList { errors: ErrorList },

    #[error(transparent)]
    Other { error: anyhow::Error },

    #[error("Wrong number of arguments passed to {func}: got {got}, expected: {expected}")]
    ArgumentCount {
        func: String,
        got: usize,
        expected: usize,
    },

    #[error("Wrong types passed to function {func}: {message}")]
    ArgumentType { func: String, message: String },

    #[error("Variable cannot have location applied: {name}")]
    VariableLocation { name: String },
}

#[derive(Debug)]
pub struct ErrorList(Vec<ShaderCreationError>);

impl fmt::Display for ErrorList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (idx, err) in self.0.iter().enumerate() {
            write!(f, "Error {}:\n", idx)?;
            write!(f, "{}\n\n", err)?;
        }

        Ok(())
    }
}

/// A complete shader program containing vertex, fragment, etc. shaders
#[derive(Debug)]
pub struct Program {
    functions: Vec<Function>,
    vertex: Option<VertexShader>,
    frag: Option<FragmentShader>,
    uniforms: Vec<Variable>,
    errors: Vec<ShaderCreationError>,
}

#[context_globals(program => uniforms)]
pub struct ProgramContext {
    program: Program,
}

/// A vertex shader's main function and input/output descriptions
#[derive(Debug)]
struct VertexShader {
    main: usize,
}

/// A fragment shader's input/output descriptions
#[derive(Debug)]
struct FragmentShader {
    main: usize,
}

/// A single function in a shader program, either a shader main function or
/// a utility function
#[derive(Debug)]
pub struct Function {
    blocks: Vec<Block>,
    vars: FunctionVars,
}

#[derive(Debug)]
struct FunctionVars {
    locals: Vec<Variable>,
    outputs: Vec<Variable>,
    inputs: Vec<Variable>,
}

#[context_globals(function.vars => inputs, outputs)]
pub struct FunctionContext<'a, 'b> {
    program: &'a mut ProgramContext,
    function: &'b mut Function,
}

/// An ssa basic block, contains no control flow, all jumps will be at the end
/// of the block, all entry will be at the start of the block
#[derive(Debug)]
pub struct Block {
    statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
/// An single expression's AST
pub enum Expression {
    /// Call a function, this is most expressions, including binary operators
    CallBuiltin {
        function: BuiltinFunction,
        arguments: Vec<Expression>,
    },

    /// Create a constant floating point value
    MakeFloat { value: f32 },

    /// Read a variable
    GetVariable { variable: VariableId },
}

/// A single operation in ssa form
#[derive(Debug, Clone)]
pub enum Statement {
    CallBuiltin {
        function: BuiltinFunction,
        arguments: Vec<VariableId>,
        result: Option<VariableId>,
    },
    MakeFloat {
        value: f32,
        variable: VariableId,
    },
    SetBuiltinVariable {
        variable: BuiltinVariable,
        value: VariableId,
    },
    GetBuiltinVariable {
        variable: BuiltinVariable,
        result: VariableId,
    },
}

/// The list of currently supported functions builtin to the shaders
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunction {
    Add,
    Div,
    Mul,
    Sub,
    Texture,
    MakeVec,
    SetGlobal,
}

/// Variables automagically provided by a shader without having to declare them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinVariable {
    VertexPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableId {
    id: usize,
    kind: VariableAllocationContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VariableAllocationContext {
    Local,
    Uniform,
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Vector(usize),

    /// matrix rows x columns
    Matrix(usize, usize),
    Floating,
    Sampler2D,
    Unknown,
}

impl Type {
    #![allow(non_upper_case_globals)]

    pub const Mat2: Type = Type::Matrix(2, 2);
    pub const Mat3: Type = Type::Matrix(3, 3);
    pub const Mat4: Type = Type::Matrix(4, 4);
    pub const Vec4: Type = Type::Vector(4);
    pub const Vec3: Type = Type::Vector(3);
    pub const Vec2: Type = Type::Vector(2);
}

// =============== //
// implementations //
// =============== //

impl Program {
    pub fn new(constructor: impl FnOnce(&mut ProgramContext)) -> Self {
        let program = Program {
            vertex: None,
            frag: None,
            uniforms: vec![],
            functions: vec![],
            errors: vec![],
        };
        let mut ctx = ProgramContext::new(program);

        constructor(&mut ctx);

        ctx.program
    }

    pub fn ok(&mut self) -> Result<(), ShaderCreationError> {
        if !self.errors.is_empty() {
            Err(ShaderCreationError::ErrorList {
                errors: ErrorList(std::mem::take(&mut self.errors)),
            })
        } else {
            Ok(())
        }
    }

    /// Get a list of all functions included in a shader
    pub fn functions(&self) -> &[Function] {
        &self.functions
    }

    /// Get the vertex shader main function
    pub fn vertex_main(&self) -> Option<&Function> {
        if let Some(vert) = &self.vertex {
            Some(&self.functions[vert.main])
        } else {
            None
        }
    }

    /// Get the fragment shader main function
    pub fn frag_main(&self) -> Option<&Function> {
        if let Some(frag) = &self.frag {
            Some(&self.functions[frag.main])
        } else {
            None
        }
    }

    /// Get the vertex shader main function
    pub fn vertex_mut(&mut self) -> Option<&mut Function> {
        if let Some(vert) = &self.vertex {
            Some(&mut self.functions[vert.main])
        } else {
            None
        }
    }

    /// Get the fragment shader main function
    pub fn frag_mut(&mut self) -> Option<&mut Function> {
        if let Some(frag) = &self.frag {
            Some(&mut self.functions[frag.main])
        } else {
            None
        }
    }

    /// Get a list of all the uniform variables in the program
    pub fn uniforms(&self) -> &[Variable] {
        &self.uniforms
    }

    pub fn uniforms_mut(&mut self) -> &mut [Variable] {
        &mut self.uniforms
    }

    pub fn get_variable<'a>(&'a self, func: &'a Function, variable: VariableId) -> &'a Variable {
        match variable.kind {
            VariableAllocationContext::Local => &func.vars.locals[variable.id],
            VariableAllocationContext::Uniform => &self.uniforms[variable.id],
            VariableAllocationContext::Input => &func.vars.inputs[variable.id],
            VariableAllocationContext::Output => &func.vars.outputs[variable.id],
        }
    }
}

impl ProgramContext {
    fn new(program: Program) -> Self {
        ProgramContext { program }
    }

    pub fn function(&mut self, constructor: impl FnOnce(&mut FunctionContext)) {
        let function = Function::new(self, constructor);
        self.program.functions.push(function);
    }

    pub fn vertex(&mut self, constructor: impl FnOnce(&mut FunctionContext)) {
        let shader = VertexShader::new(self, constructor);
        self.program.vertex = Some(shader);
    }

    pub fn frag(&mut self, constructor: impl FnOnce(&mut FunctionContext)) {
        let shader = FragmentShader::new(self, constructor);
        self.program.frag = Some(shader);
    }

    pub fn emit_error(&mut self, err: anyhow::Error) {
        self.program
            .errors
            .push(ShaderCreationError::Other { error: err });
    }

    pub fn error(&mut self, err: &str) {
        self.program.errors.push(ShaderCreationError::Other {
            error: anyhow::anyhow!(err.to_string()),
        })
    }

    fn creation_error(&mut self, err: ShaderCreationError) {
        self.program.errors.push(err);
    }

    fn check_arg_count(&mut self, fn_name: &str, args: &[VariableId], max: usize) -> Option<()> {
        if args.len() != max {
            self.creation_error(ShaderCreationError::ArgumentCount {
                func: fn_name.to_string(),
                got: args.len(),
                expected: max,
            });
            return None;
        }

        Some(())
    }
}

impl VertexShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut FunctionContext)) -> Self {
        let func = Function::new(prog, constructor);
        let main = prog.program.functions.len();
        prog.program.functions.push(func);

        VertexShader { main }
    }
}

impl FragmentShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut FunctionContext)) -> Self {
        let func = Function::new(prog, constructor);
        let main = prog.program.functions.len();
        prog.program.functions.push(func);

        FragmentShader { main }
    }
}

impl Function {
    /// create a function in a shader
    fn new(program: &mut ProgramContext, constructor: impl FnOnce(&mut FunctionContext)) -> Self {
        let mut func = Function::new_empty();
        let mut ctx = FunctionContext {
            program,
            function: &mut func,
        };

        constructor(&mut ctx);

        func.type_check(program);

        func
    }

    /// create a function with no code inside
    fn new_empty() -> Self {
        Function {
            blocks: vec![Block { statements: vec![] }],
            vars: FunctionVars {
                locals: vec![],
                inputs: vec![],
                outputs: vec![],
            },
        }
    }

    /// creates an immutable local variable that is unused
    fn local_variable(&mut self, name: &str, ty: Type) -> VariableId {
        let id = self.vars.locals.len();

        let name = if name.is_empty() {
            String::new()
        } else {
            name.to_string()
        };

        self.vars.locals.push(Variable { name, ty });

        VariableId {
            id,
            kind: VariableAllocationContext::Local,
        }
    }

    fn set_var_name(&mut self, program: &mut Program, name: &str, var: VariableId) {
        let name = name.to_string();

        match var.kind {
            VariableAllocationContext::Local => {
                self.vars.locals[var.id].name = name;
            }
            VariableAllocationContext::Uniform => {
                program.uniforms[var.id].name = name;
            }
            VariableAllocationContext::Input => {
                self.vars.inputs[var.id].name = name;
            }
            VariableAllocationContext::Output => {
                self.vars.outputs[var.id].name = name;
            }
        }
    }

    fn expr_to_variable(&mut self, program: &mut Program, expr: &Expression) -> VariableId {
        match expr {
            &Expression::GetVariable { variable } => variable,
            &Expression::MakeFloat { value } => {
                let variable = self.local_variable("", Type::Floating);
                self.set_var_name(
                    program,
                    &format!("f32_{}", self.vars.locals.len() - 1),
                    variable,
                );

                self.blocks[0]
                    .statements
                    .push(Statement::MakeFloat { value, variable });

                variable
            }
            &Expression::CallBuiltin {
                ref arguments,
                function,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|expr| self.expr_to_variable(program, expr))
                    .collect();

                let result = self.local_variable("", Type::Unknown);

                self.blocks[0].statements.push(Statement::CallBuiltin {
                    function,
                    result: Some(result),
                    arguments,
                });

                result
            }
        }
    }
}

impl Function {
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    pub fn all_vars(&self) -> impl Iterator<Item = &Variable> {
        self.vars
            .inputs
            .iter()
            .chain(self.vars.outputs.iter())
            .chain(self.vars.locals.iter())
    }

    pub fn inputs(&self) -> &[Variable] {
        &self.vars.inputs
    }

    pub fn outputs(&self) -> &[Variable] {
        &self.vars.outputs
    }

    pub fn locals(&self) -> &[Variable] {
        &self.vars.locals
    }

    pub fn inputs_mut(&mut self) -> &mut [Variable] {
        &mut self.vars.inputs
    }

    pub fn outputs_mut(&mut self) -> &mut [Variable] {
        &mut self.vars.outputs
    }

    pub fn locals_mut(&mut self) -> &mut [Variable] {
        &mut self.vars.locals
    }
}

impl<'a, 'b> FunctionContext<'a, 'b> {
    pub fn set_builtin(&mut self, builtin: BuiltinVariable, value: Expression) {
        let value = self
            .function
            .expr_to_variable(&mut self.program.program, &value);

        self.function.blocks[0]
            .statements
            .push(Statement::SetBuiltinVariable {
                variable: builtin,
                value,
            })
    }

    pub fn set_output(&mut self, target: Expression, value: Expression) {
        let target = self
            .function
            .expr_to_variable(&mut self.program.program, &target);

        let value = self
            .function
            .expr_to_variable(&mut self.program.program, &value);

        self.function.blocks[0]
            .statements
            .push(Statement::CallBuiltin {
                function: BuiltinFunction::SetGlobal,
                arguments: vec![target, value],
                result: None,
            })
    }
}

impl<'a, 'b> Deref for FunctionContext<'a, 'b> {
    type Target = ProgramContext;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

impl<'a, 'b> DerefMut for FunctionContext<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.program
    }
}

impl Block {
    /// Get all the statements in a block
    pub fn statements(&self) -> &[Statement] {
        &self.statements
    }
}

impl From<f32> for Expression {
    fn from(value: f32) -> Self {
        Expression::MakeFloat { value }
    }
}

macro_rules! ExpressionOp {
    ($op:ident, $func:ident) => {
        impl ::std::ops::$op for Expression {
            type Output = Self;

            fn $func(self, rhs: Expression) -> Self {
                Expression::CallBuiltin {
                    function: BuiltinFunction::$op,
                    arguments: vec![self, rhs],
                }
            }
        }
    };
}

macro_rules! ExpressionOps {
    ($op:ident, $func:ident) => {
        ExpressionOp!($op, $func);
    };

    ($($op:ident, $func:ident;)+) => {
        $(ExpressionOps! {$op, $func})+
    };
}

ExpressionOps! {
    Add, add;
    Div, div;
    Mul, mul;
    Sub, sub;
}

impl Expression {
    pub fn texture(tex: Expression, uv: Expression) -> Expression {
        Expression::CallBuiltin {
            arguments: vec![tex, uv],
            function: BuiltinFunction::Texture,
        }
    }

    pub fn vec(components: &[Expression]) -> Expression {
        Expression::CallBuiltin {
            arguments: components.to_vec(),
            function: BuiltinFunction::MakeVec,
        }
    }
}

impl BuiltinVariable {
    fn get_type(&self) -> Type {
        match self {
            &BuiltinVariable::VertexPosition => Type::Vec4,
        }
    }
}

impl VariableId {
    pub fn allocation_kind(&self) -> VariableAllocationContext {
        self.kind
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

// ------------- //
// Type checking //
// ------------- //

impl Function {
    fn type_check(&mut self, prog: &mut ProgramContext) {
        if prog.program.errors.is_empty() {
            for block in &self.blocks {
                block.type_check(prog, &mut self.vars);
            }
        }
    }
}

fn get_variable<'a>(
    variable: VariableId,
    prog: &'a mut ProgramContext,
    vars: &'a mut FunctionVars,
) -> &'a mut Variable {
    match variable.kind {
        VariableAllocationContext::Local => &mut vars.locals[variable.id],
        VariableAllocationContext::Uniform => &mut prog.program.uniforms[variable.id],
        VariableAllocationContext::Input => &mut vars.inputs[variable.id],
        VariableAllocationContext::Output => &mut vars.outputs[variable.id],
    }
}

impl Block {
    fn type_check(&self, prog: &mut ProgramContext, vars: &mut FunctionVars) {
        for statement in &self.statements {
            match statement {
                Statement::CallBuiltin {
                    function,
                    arguments,
                    result,
                } => {
                    if let (Some(ty), Some(result)) =
                        (function.type_check(prog, vars, arguments), result)
                    {
                        get_variable(*result, prog, vars).ty = ty;
                    }
                }
                Statement::MakeFloat { variable, .. }
                    if get_variable(*variable, prog, vars).ty == Type::Unknown =>
                {
                    get_variable(*variable, prog, vars).ty = Type::Floating;
                }
                Statement::GetBuiltinVariable { variable, result }
                    if get_variable(*result, prog, vars).ty == Type::Unknown =>
                {
                    get_variable(*result, prog, vars).ty = variable.get_type();
                }
                _ => (),
            }
        }
    }
}

impl BuiltinFunction {
    fn type_check(
        &self,
        prog: &mut ProgramContext,
        vars: &mut FunctionVars,
        arguments: &[VariableId],
    ) -> Option<Type> {
        // don't try to calculate the type if not all the inputs have types
        if arguments
            .iter()
            .any(|&variable| get_variable(variable, prog, vars).ty == Type::Unknown)
        {
            return None;
        }

        match self {
            BuiltinFunction::Add => Self::type_check_binary(prog, vars, "add", arguments),
            BuiltinFunction::Div => Self::type_check_binary(prog, vars, "div", arguments),
            BuiltinFunction::Mul => Self::type_check_mul(prog, vars, arguments),
            BuiltinFunction::Sub => Self::type_check_binary(prog, vars, "sub", arguments),

            BuiltinFunction::Texture => Self::type_check_texture(prog, vars, arguments),
            BuiltinFunction::MakeVec => Self::type_check_make_vec(prog, vars, arguments),

            // These functions do not have an output variable
            BuiltinFunction::SetGlobal => {
                Self::type_check_setter("set_global", prog, vars, arguments);
                None
            }
        }
    }

    fn type_check_binary(
        prog: &mut ProgramContext,
        vars: &mut FunctionVars,
        fn_name: &str,
        arguments: &[VariableId],
    ) -> Option<Type> {
        prog.check_arg_count(fn_name, arguments, 2)?;

        let arg1_shape = get_variable(arguments[0], prog, vars).ty;
        let arg1_shape = arg1_shape.get_shape(fn_name, prog)?;
        let arg2_shape = get_variable(arguments[1], prog, vars).ty;
        let arg2_shape = arg2_shape.get_shape(fn_name, prog)?;

        if arg1_shape == arg2_shape {
            return Some(get_variable(arguments[0], prog, vars).ty);
        }

        match (arg1_shape, arg2_shape) {
            ((1, 1), (n, m)) | ((n, m), (1, 1)) => {
                return Some(Type::from_shape(n, m));
            }
            _ => {
                let arg1 = get_variable(arguments[0], prog, vars).ty;
                let arg2 = get_variable(arguments[1], prog, vars).ty;
                prog.creation_error(ShaderCreationError::ArgumentType {
                    func: fn_name.to_string(),
                    message: format!("Unable to {} values of type {} and {}", fn_name, arg1, arg2),
                });
            }
        }

        None
    }

    fn type_check_mul(
        prog: &mut ProgramContext,
        vars: &mut FunctionVars,
        arguments: &[VariableId],
    ) -> Option<Type> {
        prog.check_arg_count("mul", arguments, 2)?;

        let arg1_shape = get_variable(arguments[0], prog, vars).ty;
        let arg1_shape = arg1_shape.get_shape("mul", prog)?;
        let arg2_shape = get_variable(arguments[1], prog, vars).ty;
        let arg2_shape = arg2_shape.get_shape("mul", prog)?;

        match (arg1_shape, arg2_shape) {
            // f32 * f32
            ((1, 1), (1, 1)) => return Some(Type::Floating),

            // f32 * mat or mat * f32
            ((1, 1), (m, n)) | ((m, n), (1, 1)) => {
                return Some(Type::from_shape(m, n));
            }

            // vec * vec elementwise multiplication
            ((1, n), (1, m)) if n == m => {
                return Some(Type::from_shape(1, n));
            }

            // mat * mat
            _ => (),
        }

        // get shape treats vectors as row vectors, if on the right hand side of
        // a matrix multiplication they should be a column vector
        let arg2_shape = if let (1, n) = arg2_shape {
            (n, 1)
        } else {
            arg2_shape
        };

        if arg1_shape.1 == arg2_shape.0 {
            return Some(Type::from_shape(arg1_shape.0, arg2_shape.1));
        }

        let arg1 = get_variable(arguments[0], prog, vars).ty;
        let arg2 = get_variable(arguments[1], prog, vars).ty;
        prog.creation_error(ShaderCreationError::ArgumentType {
            func: "mul".to_string(),
            message: format!("Unable to mul values of type {} and {}", arg1, arg2),
        });

        None
    }

    fn type_check_texture(
        prog: &mut ProgramContext,
        vars: &mut FunctionVars,
        arguments: &[VariableId],
    ) -> Option<Type> {
        prog.check_arg_count("texture", arguments, 2)?;

        let arg1 = get_variable(arguments[0], prog, vars).ty;
        let arg2 = get_variable(arguments[1], prog, vars).ty;

        if arg1 != Type::Sampler2D && arg2 != Type::Vec2 {
            prog.creation_error(ShaderCreationError::ArgumentType {
                func: "texture".to_string(),
                message: format!(
                    "The texture function currently only supports (Sampler2D, vec2), got {}, {}",
                    arg1, arg2
                ),
            });

            None
        } else {
            Some(Type::Vec4)
        }
    }

    fn type_check_make_vec(
        prog: &mut ProgramContext,
        vars: &mut FunctionVars,
        arguments: &[VariableId],
    ) -> Option<Type> {
        let shapes = arguments
            .iter()
            .map(|&arg| {
                let shape = get_variable(arg, prog, vars).ty;
                shape.get_shape("make_vec", prog)
            })
            .collect::<Option<Vec<_>>>()?;

        let mut size = 0;
        let mut has_error = false;

        for arg in shapes {
            if arg.0 != 1 {
                has_error = true;
                prog.creation_error(ShaderCreationError::ArgumentType {
                    func: "make_vec".to_string(),
                    message: format!(
                        "Cannot make vector from variable of type {}",
                        Type::from_shape(arg.0, arg.1)
                    ),
                });
            }

            size += arg.1;
        }

        if has_error {
            None
        } else {
            Some(Type::Vector(size))
        }
    }

    fn type_check_setter(
        fn_name: &str,
        prog: &mut ProgramContext,
        vars: &mut FunctionVars,
        arguments: &[VariableId],
    ) {
        if prog.check_arg_count(fn_name, arguments, 2) == None {
            return;
        }

        let arg1 = get_variable(arguments[0], prog, vars).ty;
        let arg2 = get_variable(arguments[1], prog, vars).ty;

        if arg1 != arg2 {
            prog.creation_error(ShaderCreationError::ArgumentType {
                func: fn_name.to_string(),
                message: format!(
                    "Trying to set variable of type {} to value of type {}",
                    arg1, arg2
                ),
            })
        }
    }
}

impl Type {
    /// returns (rows, cols)
    fn get_shape(&self, fn_name: &str, prog: &mut ProgramContext) -> Option<(usize, usize)> {
        match self {
            Type::Vector(cols) => Some((1, *cols)),
            Type::Matrix(rows, cols) => Some((*rows, *cols)),
            Type::Floating => Some((1, 1)),
            ty @ (Type::Sampler2D | Type::Unknown) => {
                prog.creation_error(ShaderCreationError::ArgumentType {
                    func: fn_name.to_string(),
                    message: format!("Expected numeric type such as matrix or scalar, got {}", ty),
                });
                None
            }
        }
    }

    fn from_shape(rows: usize, cols: usize) -> Type {
        match (rows, cols) {
            (1, 1) => Type::Floating,
            (1, cols) => Type::Vector(cols),
            (rows, 1) => Type::Vector(rows),
            (rows, cols) => Type::Matrix(rows, cols),
        }
    }
}

// ----------------------- //
// Display Implementations //
// ----------------------- //

impl Display for Program {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "Program {{")?;

        for uniform in &self.uniforms {
            writeln!(f, "    {};", uniform.to_string("uniform ")?)?;
        }

        if let Some(vertex) = &self.vertex {
            write!(f, "\n    vertex main")?;
            self.functions[vertex.main].fmt(f, self)?;
        }

        if let Some(frag) = &self.frag {
            write!(f, "\n    fragment main")?;
            self.functions[frag.main].fmt(f, self)?;
        }

        writeln!(f, "}}")?;

        Ok(())
    }
}

impl Variable {
    fn to_string(&self, kind: &str) -> Result<String, fmt::Error> {
        let mut s = String::new();

        write!(s, "{}${}: {}", kind, self.name, self.ty)?;

        Ok(s)
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            Type::Floating => write!(f, "f32"),
            Type::Sampler2D => write!(f, "sampler2D"),
            Type::Unknown => write!(f, "null_type"),

            Type::Vector(n) => write!(f, "vec{}", n),
            Type::Matrix(n, m) => {
                if n == m {
                    write!(f, "mat{}", n)
                } else {
                    write!(f, "mat{}x{}", n, m)
                }
            }
        }
    }
}

impl Display for BuiltinVariable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &BuiltinVariable::VertexPosition => write!(f, "gl_Position"),
        }
    }
}

fn print_function_header(f: &mut Formatter, inp: &[Variable], out: &[Variable]) -> fmt::Result {
    write!(f, "(")?;
    if inp.is_empty() {
        write!(f, ") ")?;
    } else {
        writeln!(f, "")?;
        for input in inp {
            writeln!(f, "        {},", input.to_string("")?)?;
        }
        write!(f, "    ) ")?;
    }

    if out.len() == 1 {
        write!(f, "-> {} ", out[0].to_string("")?)?;
    } else if out.len() > 1 {
        writeln!(f, "-> (")?;
        for output in out {
            writeln!(f, "        {},", output.to_string("")?)?;
        }
        write!(f, "    ) ")?;
    }

    Ok(())
}

impl Function {
    fn fmt(&self, f: &mut Formatter, prog: &Program) -> fmt::Result {
        print_function_header(f, &self.vars.inputs, &self.vars.outputs)?;

        writeln!(f, "{{")?;

        for (id, block) in self.blocks.iter().enumerate() {
            writeln!(f, "        @{}:", id)?;
            block.fmt(f, prog, self)?;
        }

        writeln!(f, "    }}")?;

        Ok(())
    }
}

impl Block {
    fn fmt(&self, f: &mut Formatter, prog: &Program, func: &Function) -> fmt::Result {
        for statement in &self.statements {
            write!(f, "          ")?;
            statement.fmt(f, prog, func)?;
            writeln!(f, "")?;
        }

        Ok(())
    }
}

fn fn_display(
    f: &mut Formatter,
    func: &Function,
    prog: &Program,
    builtin: BuiltinFunction,
    arguments: &[VariableId],
) -> fmt::Result {
    let operator = match builtin {
        BuiltinFunction::Add => Some("+"),
        BuiltinFunction::Div => Some("/"),
        BuiltinFunction::Mul => Some("*"),
        BuiltinFunction::Sub => Some("-"),
        _ => None,
    };

    if arguments.len() == 2 {
        if let Some(op) = operator {
            arguments[0].fmt(f, prog, func)?;
            write!(f, " {} ", op)?;
            arguments[1].fmt(f, prog, func)?;

            return Ok(());
        }
    }

    write!(f, "{:?} ", builtin)?;

    for (idx, arg) in arguments.iter().enumerate() {
        if idx > 0 {
            write!(f, ", ")?;
        }

        arg.fmt(f, prog, func)?;
    }

    Ok(())
}

impl Statement {
    fn fmt(&self, f: &mut Formatter, prog: &Program, func: &Function) -> fmt::Result {
        match self {
            &Statement::CallBuiltin {
                function,
                ref arguments,
                result,
            } => {
                if let Some(res) = result {
                    res.fmt(f, prog, func)?;
                    write!(f, " = ")?;
                }

                fn_display(f, func, prog, function, &arguments)?;
            }
            &Statement::MakeFloat { value, variable } => {
                variable.fmt(f, prog, func)?;
                write!(f, " = {};", value)?;
            }
            &Statement::GetBuiltinVariable { variable, result } => {
                result.fmt(f, prog, func)?;
                write!(f, " = {};", variable)?;
            }
            &Self::SetBuiltinVariable { value, variable } => {
                write!(f, "{} = ", variable)?;
                value.fmt(f, prog, func)?;
            }
        }

        Ok(())
    }
}

fn var_display(f: &mut Formatter, prefix: &str, id: usize, var: &Variable) -> fmt::Result {
    if var.name.is_empty() {
        write!(f, "{}{}: {}", prefix, id, var.ty)?;
    } else {
        let has_whitespace = var.name.chars().any(char::is_whitespace);

        if has_whitespace {
            write!(f, "{}\"{}\": {}", prefix, var.name, var.ty)?;
        } else {
            write!(f, "{}{}: {}", prefix, var.name, var.ty)?;
        }
    }

    Ok(())
}

impl VariableId {
    fn fmt(&self, f: &mut Formatter, prog: &Program, func: &Function) -> fmt::Result {
        match self.kind {
            VariableAllocationContext::Local => {
                var_display(f, "%", self.id, &func.vars.locals[self.id])?;
            }
            VariableAllocationContext::Uniform => {
                var_display(f, "$", self.id, &prog.uniforms[self.id])?;
            }
            VariableAllocationContext::Input => {
                var_display(f, "$", self.id, &func.vars.inputs[self.id])?;
            }
            VariableAllocationContext::Output => {
                var_display(f, "$", self.id, &func.vars.outputs[self.id])?;
            }
        }

        Ok(())
    }
}
