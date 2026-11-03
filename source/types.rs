// Common project-level type aliases
// Shared<T> is a shorthand for Rc<RefCell<T>> used pervasively in the project
pub type Shared<T> = std::rc::Rc<std::cell::RefCell<T>>;
