#![allow(unused_macros)]
pub mod rpc;
pub mod util;

pub type MainErr = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type MainResult<T> = std::result::Result<T, MainErr>;

#[macro_export]
macro_rules! other_err {
    ($($arg:tt)*) => ({
        Into::<knowls::MainErr>::into(std::io::Error::other(format!($($arg)*)))
    });
}
#[macro_export]
macro_rules! trace_panics {
    () => {
        std::panic::set_hook(Box::new(|v| {
            let payload = v.payload();
            let str = payload.downcast_ref::<String>().cloned().unwrap_or(
                payload
                    .downcast_ref::<&'static str>()
                    .map(|str| str.to_string())
                    .unwrap_or("Any".to_string()),
            );

            tracing::error!("thread panicked at: {:#?}\npayload: {str:#?}", v.location(),)
        }));
    };
}
