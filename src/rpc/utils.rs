#[macro_export]
macro_rules! log_and_error {
    ($params:expr, $err:expr) => {{
        error!("channel request params {:?} => error: {:?}", $params, $err);
        Err(ErrorObjectOwned::owned(
            CALL_EXECUTION_FAILED_CODE,
            $err,
            Some($params),
        ))
    }};
}

#[macro_export]
macro_rules! handle_actor_call {
    ($actor:expr, $message:expr, $params:expr) => {
        match call!($actor, $message) {
            Ok(result) => match result {
                Ok(res) => Ok(res),
                Err(e) => log_and_error!($params, e),
            },
            Err(e) => log_and_error!($params, e.to_string()),
        }
    };
}

#[macro_export]
macro_rules! handle_actor_cast {
    ($actor:expr, $message:expr, $params:expr) => {
        match $actor.cast($message) {
            Ok(_) => Ok(()),
            Err(err) => log_and_error!($params, format!("{}", err)),
        }
    };
}
