use std::error::Error;

pub type StorageError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum DbError<E> {
    Abort(E),
    Storage(StorageError),
}

pub type DbResult<T, E> = Result<T, DbError<E>>;

#[doc(hidden)]
#[derive(Debug)]
pub enum Infallible {}

pub type InfallibleDbResult<T> = DbResult<T, Infallible>;

impl<E> From<DbError<Infallible>> for DbError<E>
where
    E: Error,
{
    fn from(err: DbError<Infallible>) -> Self {
        match err {
            DbError::Abort(_) => unsafe { std::hint::unreachable_unchecked() },
            DbError::Storage(err) => Self::Storage(err),
        }
    }
}
