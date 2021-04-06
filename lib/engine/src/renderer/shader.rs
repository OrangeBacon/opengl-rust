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
    OtherError { error: anyhow::Error },

    #[error("Cannot set name of global from local context")]
    GlobalNameSet,
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
    uniforms: Vec<GlobalVariable>,
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
    inputs: Vec<GlobalVariable>,
    outputs: Vec<GlobalVariable>,
}

#[context_globals(shader => inputs, outputs)]
pub struct VertexContext<'a, 'b, 'c, 'd> {
    function: &'a mut FunctionContext<'b, 'c>,
    shader: &'d mut VertexShader,
}

/// A fragment shader's input/output descriptions
#[derive(Debug)]
struct FragmentShader {
    main: usize,
    outputs: Vec<GlobalVariable>,
    inputs: Vec<GlobalVariable>,
}

#[context_globals(shader => inputs, outputs)]
pub struct FragmentContext<'a, 'b, 'c, 'd> {
    function: &'a mut FunctionContext<'b, 'c>,
    shader: &'d mut FragmentShader,
}

/// A single function in a shader program, either a shader main function or
/// a utility function
#[derive(Debug)]
struct Function {
    blocks: Vec<Block>,
    variables: Vec<LocalVariable>,
}

pub struct FunctionContext<'a, 'b> {
    program: &'a mut ProgramContext,
    function: &'b mut Function,
}

/// An ssa basic block, contains no control flow, all jumps will be at the end
/// of the block, all entry will be at the start of the block
#[derive(Debug)]
struct Block {
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
enum Statement {
    CallBuiltin {
        function: BuiltinFunction,
        arguments: Vec<VariableId>,
        result: Option<VariableId>,
    },
    MakeFloat {
        value: f32,
        variable: VariableId,
    },
    MakeBuiltinVariable {
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
    Rem,
    Sub,
    Texture,
    MakeVec,
    GetBuiltin,
    SetBuiltin,
    SetGlobal,
    Output,
}

/// Variables automagically provided by a shader without having to declare them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinVariable {
    VertexPosition,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VariableId {
    Local(usize),
    Global(usize, GlobalAllocationContext),
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlobalAllocationContext {
    ProgramUniform,
    VertexInput,
    VertexOutput,
    FragmentInput,
    FragmentOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalVariable {
    name: String,
    ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GlobalVariable {
    name: String,
    ty: Type,
    start_location: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Vector(usize),
    Matrix(usize, usize),
    Floating,
    Sampler2D,
    Unknown,
}

impl Type {
    #![allow(non_upper_case_globals)]

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
}

impl ProgramContext {
    fn new(program: Program) -> Self {
        ProgramContext { program }
    }

    pub fn function(&mut self, constructor: impl FnOnce(&mut FunctionContext)) {
        let function = Function::new(self, constructor);
        self.program.functions.push(function);
    }

    pub fn vertex(&mut self, constructor: impl FnOnce(&mut VertexContext)) {
        let shader = VertexShader::new(self, constructor);
        self.program.vertex = Some(shader);
    }

    pub fn frag(&mut self, constructor: impl FnOnce(&mut FragmentContext)) {
        let shader = FragmentShader::new(self, constructor);
        self.program.frag = Some(shader);
    }

    pub fn emit_error(&mut self, err: anyhow::Error) {
        self.program
            .errors
            .push(ShaderCreationError::OtherError { error: err });
    }

    pub fn error(&mut self, err: &str) {
        self.program.errors.push(ShaderCreationError::OtherError {
            error: anyhow::anyhow!(err.to_string()),
        })
    }
}

impl VertexShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut VertexContext)) -> Self {
        let mut shader = VertexShader {
            outputs: vec![],
            inputs: vec![],
            main: 0,
        };

        let mut func = Function::new_empty();
        let mut fn_ctx = FunctionContext {
            function: &mut func,
            program: prog,
        };

        let mut ctx = VertexContext {
            function: &mut fn_ctx,
            shader: &mut shader,
        };

        constructor(&mut ctx);

        let fn_id = prog.program.functions.len();
        shader.main = fn_id;

        prog.program.functions.push(func);

        shader
    }
}

impl<'a, 'b, 'c, 'd> Deref for VertexContext<'a, 'b, 'c, 'd> {
    type Target = FunctionContext<'b, 'c>;

    fn deref(&self) -> &Self::Target {
        self.function
    }
}

impl<'a, 'b, 'c, 'd> DerefMut for VertexContext<'a, 'b, 'c, 'd> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.function
    }
}

impl FragmentShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut FragmentContext)) -> Self {
        let mut shader = FragmentShader {
            outputs: vec![],
            inputs: vec![],
            main: 0,
        };

        let mut func = Function::new_empty();
        let mut fn_ctx = FunctionContext {
            function: &mut func,
            program: prog,
        };

        let mut ctx = FragmentContext {
            function: &mut fn_ctx,
            shader: &mut shader,
        };

        constructor(&mut ctx);

        let fn_id = prog.program.functions.len();
        shader.main = fn_id;

        prog.program.functions.push(func);

        shader
    }
}

impl<'a, 'b, 'c, 'd> Deref for FragmentContext<'a, 'b, 'c, 'd> {
    type Target = FunctionContext<'b, 'c>;

    fn deref(&self) -> &Self::Target {
        self.function
    }
}

impl<'a, 'b, 'c, 'd> DerefMut for FragmentContext<'a, 'b, 'c, 'd> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.function
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

        func
    }

    /// create a function with no code inside
    fn new_empty() -> Self {
        Function {
            blocks: vec![Block { statements: vec![] }],
            variables: vec![],
        }
    }

    /// creates an immutable local variable that is unused
    fn local_variable(&mut self, name: &str, ty: Type) -> VariableId {
        let id = self.variables.len();

        let name = if name.is_empty() {
            String::new()
        } else {
            name.to_string()
        };

        self.variables.push(LocalVariable { name, ty });

        VariableId::Local(id)
    }

    fn set_var_name(&mut self, program: &mut Program, name: &str, var: VariableId) {
        match var {
            VariableId::Local(id) => {
                let name = name.to_string();
                self.variables[id].name = name;
            }
            VariableId::Global(_, _) => {
                program.errors.push(ShaderCreationError::GlobalNameSet);
            }
        }
    }

    fn expr_to_variable(&mut self, program: &mut Program, expr: &Expression) -> VariableId {
        match expr {
            &Expression::GetVariable { variable } => variable,
            &Expression::MakeFloat { value } => {
                let variable = self.local_variable("", Type::Floating);
                self.set_var_name(program, &format!("f32_{}", self.variables.len()), variable);

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

impl<'a, 'b> FunctionContext<'a, 'b> {
    pub fn set_global(&mut self, global: Expression, value: Expression) {
        let global = self
            .function
            .expr_to_variable(&mut self.program.program, &global);

        let value = self
            .function
            .expr_to_variable(&mut self.program.program, &value);

        self.function.blocks[0]
            .statements
            .push(Statement::CallBuiltin {
                function: BuiltinFunction::SetGlobal,
                arguments: vec![global, value],
                result: None,
            });
    }

    pub fn set_builtin(&mut self, builtin: BuiltinVariable, value: Expression) {
        let builtin_var = self
            .function
            .local_variable(&builtin.to_string(), builtin.get_type());

        self.function.blocks[0]
            .statements
            .push(Statement::MakeBuiltinVariable {
                variable: builtin,
                result: builtin_var,
            });

        let value = self
            .function
            .expr_to_variable(&mut self.program.program, &value);

        self.function.blocks[0]
            .statements
            .push(Statement::CallBuiltin {
                function: BuiltinFunction::SetBuiltin,
                arguments: vec![builtin_var, value],
                result: None,
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
                function: BuiltinFunction::Output,
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
    Rem, rem;
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

// ----------------------- //
// Display Implementations //
// ----------------------- //

fn print_function_header(
    f: &mut Formatter,
    inp: &[GlobalVariable],
    out: &[GlobalVariable],
) -> fmt::Result {
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

impl Display for Program {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        writeln!(f, "Program {{")?;

        for uniform in &self.uniforms {
            writeln!(f, "    {};", uniform.to_string("uniform ")?)?;
        }

        if let Some(vertex) = &self.vertex {
            write!(f, "\n    vertex main")?;
            print_function_header(f, &vertex.inputs, &vertex.outputs)?;
            self.functions[vertex.main].fmt(f, self)?;
        }

        if let Some(frag) = &self.frag {
            write!(f, "\n    fragment main")?;
            print_function_header(f, &frag.inputs, &frag.outputs)?;
            self.functions[frag.main].fmt(f, self)?;
        }

        writeln!(f, "}}")?;

        Ok(())
    }
}

impl GlobalVariable {
    fn to_string(&self, kind: &str) -> Result<String, fmt::Error> {
        let mut s = String::new();

        if let Some(loc) = self.start_location {
            write!(s, "layout(location = {}) ", loc)?;
        };

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

impl Function {
    fn fmt(&self, f: &mut Formatter, prog: &Program) -> fmt::Result {
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

                write!(f, "{:?} ", function)?;

                for (idx, arg) in arguments.iter().enumerate() {
                    if idx > 0 {
                        write!(f, ", ")?;
                    }

                    arg.fmt(f, prog, func)?;
                }
            }
            &Statement::MakeBuiltinVariable { variable, result } => {
                result.fmt(f, prog, func)?;
                write!(f, " = &{};", variable)?;
            }
            &Statement::MakeFloat { value, variable } => {
                variable.fmt(f, prog, func)?;
                write!(f, " = {};", value)?;
            }
        }

        Ok(())
    }
}

impl VariableId {
    fn fmt(&self, f: &mut Formatter, prog: &Program, func: &Function) -> fmt::Result {
        match self {
            &VariableId::Local(idx) => {
                let var = &func.variables[idx];
                if var.name.is_empty() {
                    write!(f, "%{}: {}", idx, var.ty)?;
                } else {
                    let has_whitespace = var.name.chars().any(char::is_whitespace);

                    if has_whitespace {
                        write!(f, "%\"{}\": {}", var.name, var.ty)?;
                    } else {
                        write!(f, "%{}: {}", var.name, var.ty)?;
                    }
                }
            }
            &VariableId::Global(idx, alloc) => match alloc {
                GlobalAllocationContext::ProgramUniform => {
                    let var = &prog.uniforms[idx];
                    write!(f, "${}: {}", var.name, var.ty)?;
                }
                GlobalAllocationContext::VertexInput => {}
                GlobalAllocationContext::VertexOutput => {}
                GlobalAllocationContext::FragmentInput => {}
                GlobalAllocationContext::FragmentOutput => {}
            },
        }

        Ok(())
    }
}
