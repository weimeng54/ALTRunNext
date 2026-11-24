// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

use std::{cell::Cell, ptr::null_mut};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::Input::{
        Ime::{
            GCS_COMPSTR, HIMC, ImmAssociateContext, ImmGetCompositionStringW, ImmGetContext,
            ImmGetConversionStatus, ImmGetOpenStatus, ImmReleaseContext, ImmSetConversionStatus,
            ImmSetOpenStatus,
        },
        KeyboardAndMouse::{ActivateKeyboardLayout, GetKeyboardLayout, HKL},
    },
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

pub(crate) fn is_composing(edit: HWND) -> bool {
    unsafe {
        let context = ImmGetContext(edit);
        if context.is_null() {
            return false;
        }
        let composing = ImmGetCompositionStringW(context, GCS_COMPSTR, null_mut(), 0) > 0;
        ImmReleaseContext(edit, context);
        composing
    }
}

pub(crate) struct InputMode {
    layout: HKL,
    open: bool,
    conversion: Option<(u32, u32)>,
}

impl InputMode {
    pub(crate) fn capture(edit: HWND) -> Self {
        unsafe {
            let layout = GetKeyboardLayout(0);
            let context = ImmGetContext(edit);
            let (mut conversion, mut sentence) = (0, 0);
            let open = !context.is_null() && ImmGetOpenStatus(context) != 0;
            let conversion = (!context.is_null()
                && ImmGetConversionStatus(context, &mut conversion, &mut sentence) != 0)
                .then_some((conversion, sentence));
            if !context.is_null() {
                ImmReleaseContext(edit, context);
            }
            Self {
                layout,
                open,
                conversion,
            }
        }
    }

    pub(crate) fn start_in_english(edit: HWND) {
        unsafe {
            let context = ImmGetContext(edit);
            if !context.is_null() {
                // Leave the IME associated so its switching keys still work.
                if ImmGetOpenStatus(context) != 0 {
                    ImmSetOpenStatus(context, 0);
                }
                ImmReleaseContext(edit, context);
            }
        }
    }

    pub(crate) fn restore(self, edit: HWND) {
        unsafe {
            if !self.layout.is_null() && GetKeyboardLayout(0) != self.layout {
                ActivateKeyboardLayout(self.layout, 0);
            }
            let context = ImmGetContext(edit);
            if !context.is_null() {
                if (ImmGetOpenStatus(context) != 0) != self.open {
                    ImmSetOpenStatus(context, i32::from(self.open));
                }
                if let Some((conversion, sentence)) = self.conversion {
                    ImmSetConversionStatus(context, conversion, sentence);
                }
                ImmReleaseContext(edit, context);
            }
        }
    }
}
