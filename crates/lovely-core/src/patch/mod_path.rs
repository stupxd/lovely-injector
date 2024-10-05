use std::{
    ffi::{c_void, CString},
    fs,
    path::PathBuf,
    ptr, slice,
};

use crate::sys::{self, lua_identity_closure, LuaState};
//use crate::sys::{self, LuaState};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModPathPatch {
    pub before: String,
    pub name: String,
    
    // Is set to mod's root folder path.
    #[serde(skip)]
    pub path: String,
}

impl ModPathPatch {
    /// Apply a mod path patch by adding mod path into the template,
    /// and loading it as module instantly
    ///
    /// # Safety
    /// Unsafe due to internal unchecked usages of raw lua state.
    pub unsafe fn apply<F: Fn(*mut LuaState, *const u8, isize, *const u8, *const u8) -> u32>(
        &self,
        file_name: &str,
        state: *mut LuaState,
        lual_loadbufferx: &F,
    ) -> bool {
        // Stop if we're not at the correct insertion point.
        if self.before != file_name {
            return false;
        }

        let mut code = include_str!("../../mod_path.lua").to_string();
        code = code.replace("lovely_template:mod_path", &self.path);
        log::info!("Creating module `{}` for mod path: \"{}\"\nwith code: {}", &self.name, &self.path, &code);
        
        // This doesnt work for some reason
        //sys::load_module(state, &self.name, &code, lual_loadbufferx);
        
        
        let buf_cstr = CString::new(code.as_str()).unwrap();
        let buf_len = buf_cstr.as_bytes().len();

        let name = format!("=[lovely {} \"{}\"]", &self.name, &self.name);
        let name_cstr = CString::new(name).unwrap();

        // Push the global package.preload table onto the top of the stack, saving its index.
        let stack_top = sys::lua_gettop(state);
        sys::lua_getfield(state, sys::LUA_GLOBALSINDEX, b"package\0".as_ptr() as _);
        sys::lua_getfield(state, -1, b"preload\0".as_ptr() as _);

        // This is the index of the `package.preload` table.
        let field_index = sys::lua_gettop(state);

        // Load the buffer and execute it via lua_pcall, pushing the result to the top of the stack.
        let return_code = lual_loadbufferx(
            state,
            buf_cstr.into_raw() as _,
            buf_len as _,
            name_cstr.into_raw() as _,
            ptr::null(),
        );

        // Returns code 3
        if return_code != 0 {
            log::error!("Failed to load module {}, code:{}", self.name, return_code);
            sys::lua_settop(state, stack_top);
            return false;
        }

        // Evaluate the results of the buffer now
        let return_code = sys::lua_pcall(state, 0, 1, 0);
        if return_code != 0 {
            log::error!("Evaluation of module failed for {}, code:{}", self.name, return_code);
            sys::lua_settop(state, stack_top);
            return false;
        }
        // Wrap this in the identity closure function
        sys::lua_pushcclosure(state, lua_identity_closure as *const c_void, 1);

        // Insert results onto the package.preload global table.
        let module_cstr = CString::new(self.name.clone()).unwrap();
        sys::lua_setfield(state, field_index, module_cstr.into_raw() as _);
        // Always ensure that the lua stack is in good order
        sys::lua_settop(state, stack_top);
        
        true
    }
}
