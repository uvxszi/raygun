//! The syntax tree of the Kotlin the parser reads: declarations, statements and expressions.

/// A function type with a receiver (`HttpUrl.Builder.() -> Unit`): its first argument is the
/// receiver.
pub const RECEIVER_FUNCTION: &str = "(.)";

/// A type as written; arguments and nullability are kept, the rest (variance, annotations) isn't.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Type {
    /// Dotted name (`Filter.Select`), `()` for a function type, or [`RECEIVER_FUNCTION`].
    pub name: String,
    pub args: Vec<Type>,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Text(String),
    Code(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Expr,
    /// `*array`.
    pub spread: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lambda {
    /// Parameter names (`_` for unused ones); a destructured one is its names in parentheses.
    pub params: Vec<LambdaParam>,
    /// No `->`: one parameter, `it`.
    pub implicit: bool,
    pub body: Block,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LambdaParam {
    Name(String),
    Destructured(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhenBranch {
    /// Empty for `else`.
    pub conditions: Vec<WhenCondition>,
    /// A guard: `is JsonObject if "i" in pages ->`.
    pub guard: Option<Expr>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WhenCondition {
    Expr(Expr),
    Is { ty: Type, negated: bool },
    In { range: Expr, negated: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Catch {
    pub name: String,
    pub ty: Type,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(Vec<StrPart>),
    Char(char),
    Bool(bool),
    Null,
    Name(String),
    This(Option<String>),
    Super,
    Member {
        target: Box<Expr>,
        name: String,
        safe: bool,
    },
    Call {
        callee: Box<Expr>,
        type_args: Vec<Type>,
        args: Vec<Arg>,
        lambda: Option<Box<Lambda>>,
    },
    Index {
        target: Box<Expr>,
        index: Vec<Expr>,
    },
    NotNull(Box<Expr>),
    Unary {
        op: &'static str,
        expr: Box<Expr>,
    },
    /// `x++` and friends, as expressions.
    Postfix {
        op: &'static str,
        expr: Box<Expr>,
    },
    /// Arithmetic, comparison, logic, `?:`, ranges, `in`, and infix calls (`to`, `until`).
    Binary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Is {
        expr: Box<Expr>,
        ty: Type,
        negated: bool,
    },
    As {
        expr: Box<Expr>,
        ty: Type,
        safe: bool,
    },
    If {
        condition: Box<Expr>,
        then: Block,
        otherwise: Option<Block>,
    },
    When {
        subject: Option<Box<Expr>>,
        /// `when (val x = …)`.
        binding: Option<String>,
        branches: Vec<WhenBranch>,
    },
    Try {
        body: Block,
        catches: Vec<Catch>,
        finally: Option<Block>,
    },
    Lambda(Box<Lambda>),
    /// `::name`, `Type::name`, `expr::name`, `Type::class`.
    Reference {
        target: Option<Box<Expr>>,
        name: String,
    },
    Throw(Box<Expr>),
    Return {
        label: Option<String>,
        value: Option<Box<Expr>>,
    },
    Break(Option<String>),
    Continue(Option<String>),
    /// `object : Supertype { … }`.
    Object(Box<ClassDecl>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `val x = …`, `var (a, b) = …`, `val x: T` (no value).
    Val {
        names: Vec<String>,
        destructured: bool,
        mutable: bool,
        value: Option<Expr>,
        /// `by lazy { … }`.
        delegate: Option<Expr>,
    },
    Assign {
        target: Expr,
        op: &'static str,
        value: Expr,
    },
    Expr(Expr),
    For {
        label: Option<String>,
        vars: LambdaParam,
        iterable: Expr,
        body: Block,
    },
    While {
        label: Option<String>,
        condition: Expr,
        body: Block,
        /// `do { } while (…)`.
        after: bool,
    },
    /// A local function or class.
    Decl(Box<Member>),
}

pub type Block = Vec<Stmt>;

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
    pub vararg: bool,
    /// A constructor's `val`/`var` parameter, which is a property too.
    pub property: bool,
    /// Its annotations' names; `SerialName=<json name>` for `@SerialName("…")`.
    pub annotations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Expr(Expr),
    Block(Block),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunDecl {
    pub name: String,
    pub modifiers: Vec<String>,
    pub annotations: Vec<String>,
    pub receiver: Option<Type>,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub body: Option<Body>,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropDecl {
    pub name: String,
    pub modifiers: Vec<String>,
    pub mutable: bool,
    pub receiver: Option<Type>,
    pub ty: Option<Type>,
    pub value: Option<Expr>,
    pub getter: Option<Body>,
    pub delegate: Option<Expr>,
    pub line: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Interface,
    Object,
    Enum,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuperRef {
    pub ty: Type,
    /// Constructor arguments; `None` for an interface.
    pub args: Option<Vec<Arg>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumEntry {
    pub name: String,
    pub args: Vec<Arg>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDecl {
    pub name: String,
    /// Its type parameters' names (`class ApiResponse<T>`).
    pub type_params: Vec<String>,
    pub kind: ClassKind,
    pub modifiers: Vec<String>,
    pub annotations: Vec<String>,
    pub params: Vec<Param>,
    pub supers: Vec<SuperRef>,
    pub entries: Vec<EnumEntry>,
    pub members: Vec<Member>,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Member {
    Fun(FunDecl),
    Prop(PropDecl),
    Class(ClassDecl),
    Init(Block),
    /// `typealias Name = Type`.
    TypeAlias(String, Type),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct File {
    pub package: String,
    /// Imports: (name as used, full name).
    pub imports: Vec<(String, String)>,
    pub members: Vec<Member>,
}

impl Member {
    pub fn name(&self) -> &str {
        match self {
            Self::Fun(fun) => &fun.name,
            Self::Prop(prop) => &prop.name,
            Self::Class(class) => &class.name,
            Self::Init(_) => "init",
            Self::TypeAlias(name, _) => name,
        }
    }

    pub fn modifiers(&self) -> &[String] {
        match self {
            Self::Fun(fun) => &fun.modifiers,
            Self::Prop(prop) => &prop.modifiers,
            Self::Class(class) => &class.modifiers,
            Self::Init(_) | Self::TypeAlias(..) => &[],
        }
    }

    pub fn is_override(&self) -> bool {
        self.modifiers()
            .iter()
            .any(|modifier| modifier == "override")
    }
}
