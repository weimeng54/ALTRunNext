use std::{cell::Cell, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::Input::Ime::{HIMC, ImmAssociateContext},
};

thread_local! {
    static SAVED_CONTEXT: Cell<Option<HIMC>> = const { Cell::new(None) };
}

pub fn set_direct(edit: HWND, direct: bool) {
    // This association belongs to our edit control. Its previous context
    // keeps its own open/conversion state while detached.
    SAVED_CONTEXT.with(|saved| {
        if direct {
            if saved.get().is_none() {
                let original = unsafe { ImmAssociateContext(edit, null_mut()) };
                saved.set(Some(original));
            }
        } else if let Some(original) = saved.take()
            && !original.is_null()
        {
            unsafe {
                ImmAssociateContext(edit, original);
            }
        }
    });
}
