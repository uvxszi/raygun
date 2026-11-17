//! Reading (M46.56): the book reader's word lookups (`dictionary`), EPUB books (`epub`) and where
//! imported books are kept (`books`).

pub use raygun_app_base::books;
pub mod dictionary;
pub mod epub;
