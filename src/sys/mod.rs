pub mod fs;
pub mod math;
pub mod mem;

/// Readonly
pub mod ro {

    pub type Str = std::rc::Rc<str>;
    pub type AStr = std::sync::Arc<str>;

    pub type Array<T> = std::rc::Rc<[T]>;
    pub type AArray<T> = std::sync::Arc<[T]>;
}
