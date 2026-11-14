// Copyright (c) 2026 GPT-6.1 Sol
// Licensed under the MIT License. See LICENSE file in the project root for full license information.

//! Supply names for virtual list items through Microsoft Active Accessibility.
use super::STATE;
use std::{
    ffi::c_void,
    ptr::null_mut,
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::{
    Win32::{
        Foundation::{E_NOINTERFACE, E_OUTOFMEMORY, E_POINTER, HWND, S_OK, SysAllocStringLen},
        System::{
            Com::*,
            Variant::{VARIANT, VT_BSTR},
        },
        UI::{
            Accessibility::{ANNO_CONTAINER, CLSID_AccPropServices, PROPID_ACC_NAME},
            WindowsAndMessaging::OBJID_CLIENT,
        },
    },
    core::{BOOL, GUID, HRESULT, IID_IUnknown, IUnknown_Vtbl},
};

const IID_SERVICES: GUID = GUID::from_u128(0x6e26e776_04f0_495d_80e4_3330352e3169);
const IID_SERVER: GUID = GUID::from_u128(0x76c0dbbb_15e0_4e7b_b61b_20eeea2001e0);
const CLIENT: u32 = OBJID_CLIENT as u32;

// COM layout from oleacc.h. The unused slots above these methods retain their ABI positions.
#[repr(C)]
struct Services {
    base: IUnknown_Vtbl,
    unused_annotations: [usize; 5],
    set_server: unsafe extern "system" fn(
        *mut c_void,
        HWND,
        u32,
        u32,
        *const GUID,
        i32,
        *mut Server,
        i32,
    ) -> HRESULT,
    clear: unsafe extern "system" fn(*mut c_void, HWND, u32, u32, *const GUID, i32) -> HRESULT,
    unused_identity: usize,
    decode: unsafe extern "system" fn(
        *mut c_void,
        *const u8,
        u32,
        *mut HWND,
        *mut u32,
        *mut u32,
    ) -> HRESULT,
}

#[repr(C)]
struct ServerMethods {
    base: IUnknown_Vtbl,
    get: unsafe extern "system" fn(
        *mut Server,
        *const u8,
        u32,
        GUID,
        *mut VARIANT,
        *mut BOOL,
    ) -> HRESULT,
}

#[repr(C)]
struct Server {
    methods: &'static ServerMethods,
    references: AtomicU32,
    services: *mut c_void,
    list: HWND,
}

static METHODS: ServerMethods = ServerMethods {
    base: IUnknown_Vtbl {
        QueryInterface: query,
        AddRef: add_ref,
        Release: release,
    },
    get: get_name,
};

unsafe fn methods(services: *mut c_void) -> &'static Services {
    unsafe { &**services.cast::<*const Services>() }
}

fn same_guid(a: &GUID, b: &GUID) -> bool {
    (a.data1, a.data2, a.data3, a.data4) == (b.data1, b.data2, b.data3, b.data4)
}

unsafe extern "system" fn query(
    this: *mut c_void,
    iid: *const GUID,
    result: *mut *mut c_void,
) -> HRESULT {
    if result.is_null() || iid.is_null() {
        return E_POINTER;
    }
    unsafe {
        *result = null_mut();
        if !same_guid(&*iid, &IID_IUnknown) && !same_guid(&*iid, &IID_SERVER) {
            return E_NOINTERFACE;
        }
        add_ref(this);
        *result = this;
        S_OK
    }
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    unsafe {
        (*this.cast::<Server>())
            .references
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    unsafe {
        let server = this.cast::<Server>();
        let remaining = (*server).references.fetch_sub(1, Ordering::AcqRel) - 1;
        if remaining == 0 {
            let server = Box::from_raw(server);
            (methods(server.services).base.Release)(server.services);
        }
        remaining
    }
}

unsafe extern "system" fn get_name(
    server: *mut Server,
    identity: *const u8,
    len: u32,
    property: GUID,
    value: *mut VARIANT,
    has_value: *mut BOOL,
) -> HRESULT {
    if value.is_null() || has_value.is_null() {
        return E_POINTER;
    }
    unsafe {
        *value = VARIANT::default();
        *has_value = 0;
        if !same_guid(&property, &PROPID_ACC_NAME) {
            return S_OK;
        }
        let server = &*server;
        let mut hwnd = null_mut();
        let (mut object, mut child) = (0, 0);
        if (methods(server.services).decode)(
            server.services,
            identity,
            len,
            &mut hwnd,
            &mut object,
            &mut child,
        ) < 0
            || hwnd != server.list
            || object != CLIENT
            || child == 0
        {
            return S_OK;
        }
        let name = STATE
            .try_with(|cell| {
                let state = cell.try_borrow().ok()?;
                let state = state.as_ref()?;
                let &entry = state.hits.get(child as usize - 1)?;
                let entry = state.entries.get(entry)?;
                Some(super::panel::result_label(entry))
            })
            .ok()
            .flatten();
        if let Some(name) = name {
            let string = SysAllocStringLen(name.as_ptr(), (name.len() - 1) as u32);
            if string.is_null() {
                return E_OUTOFMEMORY;
            }
            (*value).Anonymous.Anonymous.vt = VT_BSTR;
            (*value).Anonymous.Anonymous.Anonymous.bstrVal = string;
            *has_value = 1;
        }
        S_OK
    }
}

pub(super) struct ResultNames {
    services: *mut c_void,
    list: HWND,
}

impl ResultNames {
    pub(super) fn register(list: HWND) -> Option<Self> {
        if list.is_null() {
            return None;
        }
        unsafe {
            if CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED as u32) < 0 {
                return None;
            }
            let mut services = null_mut();
            if CoCreateInstance(
                &CLSID_AccPropServices,
                null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_SERVICES,
                &mut services,
            ) < 0
            {
                CoUninitialize();
                return None;
            }
            let registration = Self { services, list };
            (methods(services).base.AddRef)(services);
            let server = Box::into_raw(Box::new(Server {
                methods: &METHODS,
                references: AtomicU32::new(1),
                services,
                list,
            }));
            let result = (methods(services).set_server)(
                services,
                list,
                CLIENT,
                0,
                &PROPID_ACC_NAME,
                1,
                server,
                ANNO_CONTAINER,
            );
            release(server.cast());
            (result >= 0).then_some(registration)
        }
    }
}

impl Drop for ResultNames {
    fn drop(&mut self) {
        unsafe {
            let service = methods(self.services);
            (service.clear)(self.services, self.list, CLIENT, 0, &PROPID_ACC_NAME, 1);
            (service.base.Release)(self.services);
            CoUninitialize();
        }
    }
}
