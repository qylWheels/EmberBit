#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mutability {
    Const,
    Var,
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
    pub return_type_mutability: Mutability,
    pub return_type: Box<Type>,
}
