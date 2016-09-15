#[macro_use]
extern crate neon;
extern crate fs2;

use neon::vm::{Call, JsResult, Module};
use neon::js::error::{JsError, Kind, throw};
use neon::js::{JsString, JsUndefined};
use neon::mem::Handle;

use std::fs;
use std::io::Error;
use fs2::*;

fn try_lock_exclusive(call: Call) -> JsResult<JsUndefined> {
    let scope = call.scope;
    let string: Handle<JsString> = try!(try!(call.arguments.require(scope, 0)).check::<JsString>());
    let path = &string.value()[..];

    let f = fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();

    println!("try locking {}", &path);

    match f.try_lock_exclusive() {
        Err(err) => {
            JsError::throw(Kind::Error, "Resource locked")
        },
        _ => {
            println!("locking {}", &path);
            f.lock_exclusive().unwrap();
            Ok(JsUndefined::new())
        }
    }
}


fn unlock(call: Call) -> JsResult<JsUndefined> {
    let scope = call.scope;
    let string: Handle<JsString> = try!(try!(call.arguments.require(scope, 0)).check::<JsString>());
    let path = &string.value()[..];

    let f = fs::OpenOptions::new().read(true).write(true).create(true).open(&path).unwrap();

    f.unlock().unwrap();
    Ok(JsUndefined::new())
}

register_module!(m, {
    try!(m.export("tryLockExclusive", try_lock_exclusive));
    try!(m.export("unlock", unlock));
    Ok(())
});
