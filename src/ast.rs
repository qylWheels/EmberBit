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

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
    pub fields: Vec<(String, Type)>,
}

#[derive(Debug, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub params: Vec<(String, Type)>,
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
    Address(AddressExpression),
    AddressVariable(AddressVariableExpression),
    Array(ArrayExpression),
    Tuple(TupleExpression),
    Binary(BinaryExpression),
    ConstSlice(ConstSliceExpression),
    VarSlice(VarSliceExpression),
    Index(IndexExpression),
    Integer(IntegerLiteral),
    Identifier(Identifier),
}

#[derive(Debug, Clone)]
pub struct AddressExpression {
    pub expr: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct AddressVariableExpression {
    pub expr: Box<Expression>,
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
pub struct ConstSliceExpression {
    pub base: Box<Expression>,
    pub start: Box<Expression>,
    pub end: Box<Expression>,
}

#[derive(Debug, Clone)]
pub struct VarSliceExpression {
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
pub struct IntegerLiteral {
    pub value: i64,
}

#[derive(Debug, Clone)]
pub struct Identifier {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Trivial(TrivialType),
    Pointer(PointerType),
    VariablePointer(VariablePointerType),
    Array(ArrayType),
    ConstSlice(ConstSliceType),
    VarSlice(VarSliceType),
    Tuple(TupleType),
    Function(FunctionType),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrivialType {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PointerType {
    pub pointee_type: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VariablePointerType {
    pub pointee_type: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArrayType {
    pub size: i64,
    pub elem_type: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstSliceType {
    pub elem_type: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VarSliceType {
    pub elem_type: Box<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TupleType {
    pub elem_types: Vec<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionType {
    pub param_types: Vec<Type>,
    pub return_type: Box<Type>,
}
