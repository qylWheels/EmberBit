//! Abstract Syntax Tree (AST) definitions for EmberBit.

#[derive(Debug, Clone)]
pub struct Module {
    pub top_definitions: Vec<TopDefinition>,
}

#[derive(Debug, Clone)]
pub enum TopDefinition {
    Import(ImportDefinition),
    Struct(StructDefinition),
    Function(FunctionDefinition),
}

#[derive(Debug, Clone)]
pub struct ImportDefinition {
    pub parts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
    pub fields: Vec<(Visibility, String, Type)>,
    pub methods: Vec<(Visibility, FunctionDefinition)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mutability {
    Const,
    Var,
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub params: Vec<(Mutability, String, Type)>,
    pub return_type: Type,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    ConstDef(ConstDefStatement),
    VarDef(VarDefStatement),
    Assignment(AssignmentStatement),
    Return(ReturnStatement),
    Expression(ExpressionStatement),
}

#[derive(Debug, Clone)]
pub struct ConstDefStatement {
    pub name: String,
    pub ty: Type,
    pub expr: Expression,
}

#[derive(Debug, Clone)]
pub struct VarDefStatement {
    pub name: String,
    pub ty: Type,
    pub expr: Expression,
}

#[derive(Debug, Clone)]
pub struct AssignmentStatement {
    pub lhs: Expression,
    pub rhs: Expression,
}

#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub expr: Expression,
}

#[derive(Debug, Clone)]
pub struct ExpressionStatement {
    pub expr: Expression,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Array(ArrayExpression),
    Tuple(TupleExpression),
    Binary(BinaryExpression),
    Slice(SliceExpression),
    Index(IndexExpression),
    Call(CallExpression),
    Integer(IntegerLiteral),
    Identifier(Identifier),
    Struct(StructExpression),
    FieldAccess(FieldAccessExpression),
}

#[derive(Debug, Clone)]
pub struct ArrayExpression {
    pub elements: Vec<Expression>,
}

#[derive(Debug, Clone)]
pub struct TupleExpression {
    pub elements: Vec<Expression>,
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone)]
pub struct BinaryExpression {
    pub lhs: Box<Expression>,
    pub op: BinaryOp,
    pub rhs: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct SliceExpression {
    pub base: Box<Expression>,
    pub start: Box<Expression>,
    pub end: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct IndexExpression {
    pub base: Box<Expression>,
    pub index: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct CallExpression {
    pub callee: Box<Expression>,
    pub args: Vec<Expression>,
}

#[derive(Debug, Clone)]
pub struct IntegerLiteral {
    pub value: i64,
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct StructExpression {
    pub name: String,
    pub fields: Vec<(String, Expression)>,
}

#[derive(Debug, Clone)]
pub struct FieldAccessExpression {
    pub base: Box<Expression>,
    pub field: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Trivial(TrivialType),
    Array(ArrayType),
    Slice(SliceType),
    Tuple(TupleType),
    Function(FunctionType),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrivialType {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayType {
    pub size: i64,
    pub elem_type: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SliceType {
    pub elem_type: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TupleType {
    pub elem_types: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionType {
    pub params: Vec<(Mutability, Type)>,
    pub return_type: Box<Type>,
}
