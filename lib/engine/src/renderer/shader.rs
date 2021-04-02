use std::ops::{Deref, DerefMut, Range};

use engine_proc_macro::context_globals;

// ================= //
// type declarations //
// ================= //

/// A complete shader program containing vertex, fragment, etc. shaders
#[derive(Debug)]
pub struct Program {
    vertex: Option<VertexShader>,
    frag: Option<FragmentShader>,
    uniforms: Vec<GlobalVariable>,
}

#[context_globals(program => uniforms)]
pub struct ProgramContext {
    program: Program,
}

/// A vertex shader's main function and input/output descriptions
#[derive(Debug)]
struct VertexShader {
    main: Function,
    inputs: Vec<GlobalVariable>,
    outputs: Vec<GlobalVariable>,
}

#[context_globals(shader => inputs, outputs)]
pub struct VertexContext<'a, 'b> {
    program: &'a mut ProgramContext,
    shader: &'b mut VertexShader,
}

/// A fragment shader's main function and input/output descriptions
#[derive(Debug)]
struct FragmentShader {
    main: Function,
    outputs: Vec<GlobalVariable>,
    inputs: Vec<GlobalVariable>,
}

#[context_globals(shader => inputs, outputs)]
pub struct FragmentContext<'a, 'b> {
    program: &'a mut ProgramContext,
    shader: &'b mut FragmentShader,
}

/// A single function in a shader program, either a shader main function or
/// a utility function
#[derive(Debug)]
struct Function {
    blocks: Vec<Block>,
    variables: Vec<LocalVariable>,
}

/// An ssa basic block, contains no control flow, all jumps will be at the end
/// of the block, all entry will be at the start of the block
#[derive(Debug)]
struct Block {
    statements: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Expression {
    CallBuiltin {
        function: BuiltinFunction,
        arguments: Vec<Expression>,
    },
    MakeFloat {
        value: f32,
    },
    GetVariable {
        variable: VariableId,
    },
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinFunction {
    Add,
    BitAnd,
    BitOr,
    BitXor,
    Div,
    Mul,
    Rem,
    Sub,
    Texture,
    MakeVec4,
    Output,
    AccessField,
    GetBuiltin,
    SetBuiltin,
}

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
        };
        let mut ctx = ProgramContext::new(program);

        constructor(&mut ctx);

        ctx.program
    }
}

impl ProgramContext {
    fn new(program: Program) -> Self {
        ProgramContext { program }
    }

    pub fn vertex(&mut self, constructor: impl FnOnce(&mut VertexContext)) {
        let shader = VertexShader::new(self, constructor);
        self.program.vertex = Some(shader);
    }

    pub fn frag(&mut self, constructor: impl FnOnce(&mut FragmentContext)) {
        let shader = FragmentShader::new(self, constructor);
        self.program.frag = Some(shader);
    }
}

impl VertexShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut VertexContext)) -> Self {
        let mut shader = VertexShader {
            outputs: vec![],
            inputs: vec![],
            main: Function {
                blocks: vec![],
                variables: vec![],
            },
        };

        let mut ctx = VertexContext {
            program: prog,
            shader: &mut shader,
        };

        constructor(&mut ctx);

        shader
    }
}

impl FragmentShader {
    fn new(prog: &mut ProgramContext, constructor: impl FnOnce(&mut FragmentContext)) -> Self {
        let mut shader = FragmentShader {
            inputs: vec![],
            outputs: vec![],
            main: Function {
                blocks: vec![],
                variables: vec![],
            },
        };

        let mut ctx = FragmentContext {
            program: prog,
            shader: &mut shader,
        };

        constructor(&mut ctx);

        shader
    }
}

impl<'a, 'b> Deref for VertexContext<'a, 'b> {
    type Target = ProgramContext;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

impl<'a, 'b> DerefMut for VertexContext<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.program
    }
}

impl<'a, 'b> Deref for FragmentContext<'a, 'b> {
    type Target = ProgramContext;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

impl<'a, 'b> DerefMut for FragmentContext<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.program
    }
}

macro_rules! ExpressionOpSelf {
    ($op:ident, $func:ident) => {
        impl ::std::ops::$op for Expression {
            type Output = Self;

            fn $func(self, rhs: Self) -> Self {
                Expression::CallBuiltin {
                    function: BuiltinFunction::$op,
                    arguments: vec![self, rhs],
                }
            }
        }
    };
}

macro_rules! ExpressionOpF32 {
    ($op:ident, $func:ident) => {
        impl ::std::ops::$op<f32> for Expression {
            type Output = Self;

            fn $func(self, rhs: f32) -> Self {
                Expression::CallBuiltin {
                    function: BuiltinFunction::$op,
                    arguments: vec![self, Expression::MakeFloat { value: rhs }],
                }
            }
        }
    };
}

macro_rules! ExpressionOps {
    ($op:ident, $func:ident) => {
        ExpressionOpSelf!($op, $func);
        ExpressionOpF32!($op, $func);
    };

    ($($op:ident, $func:ident;)+) => {
        $(ExpressionOps! {$op, $func})+
    };
}

ExpressionOps! {
    Add, add;
    BitAnd, bitand;
    BitOr, bitor;
    BitXor, bitxor;
    Div, div;
    Mul, mul;
    Rem, rem;
    Sub, sub;
}
