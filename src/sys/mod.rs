pub mod fs;
pub mod math;
pub mod mem;

/// Readonly
pub mod ro {

    pub type OwnedStr = Box<str>;
    pub type Str = std::rc::Rc<str>;
    pub type AStr = std::sync::Arc<str>;

    pub type OwnedArr<T> = Box<[T]>;
    pub type Arr<T> = std::rc::Rc<[T]>;
    pub type AArr<T> = std::sync::Arc<[T]>;
}
