pub mod migrations;
pub mod repository;

pub fn register_sqlite_vec_extension() {
    type SqliteVecInit = unsafe extern "C" fn(
        *mut rusqlite::ffi::sqlite3,
        *mut *mut std::os::raw::c_char,
        *const rusqlite::ffi::sqlite3_api_routines,
    ) -> i32;

    let init: SqliteVecInit =
        unsafe { std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ()) };
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(init));
    }
}
