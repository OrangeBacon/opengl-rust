use std::{
    fmt,
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
    function: &'b Function,
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
            blocks: vec![],
            variables: vec![],
        }
    }

    /// creates an immutable local variable and sets its value
    fn local_variable(&mut self, name: &str, ty: Type, value: Statement) -> VariableId {
        let id = self.variables.len();

        VariableId::Local(id)
    }
}

impl<'a, 'b> FunctionContext<'a, 'b> {
    pub fn set_global(&mut self, global: Expression, value: Expression) {}
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
