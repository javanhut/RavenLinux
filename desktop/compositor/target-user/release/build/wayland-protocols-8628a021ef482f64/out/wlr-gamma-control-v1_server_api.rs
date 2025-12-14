use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 1] =
    [NULLPTR as *const sys::common::wl_interface];
#[doc = "manager to create per-output gamma controls\n\nThis interface is a manager that allows creating per-output gamma\ncontrols."]
pub mod zwlr_gamma_control_manager_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Request {
        #[doc = "get a gamma control for an output\n\nCreate a gamma control that can be used to adjust gamma tables for the\nprovided output."]
        GetGammaControl {
            id: Main<super::zwlr_gamma_control_v1::ZwlrGammaControlV1>,
            output: super::wl_output::WlOutput,
        },
        #[doc = "destroy the manager\n\nAll objects created by the manager will still remain valid, until their\nappropriate destroy request has been called.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "get_gamma_control",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "destroy",
                since: 1,
                signature: &[],
                destructor: true,
            },
        ];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Request::Destroy => true,
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::GetGammaControl { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::GetGammaControl { .. } => 1,
                Request::Destroy => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwlr_gamma_control_v1::ZwlrGammaControlV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetGammaControl {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        output: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                1 => Ok(Request::Destroy),
                _ => Err(()),
            }
        }
        fn into_raw(self, sender_id: u32) -> Message {
            panic!("Request::into_raw can not be used Server-side.")
        }
        unsafe fn from_raw_c(
            obj: *mut ::std::os::raw::c_void,
            opcode: u32,
            args: *const wl_argument,
        ) -> Result<Request, ()> {
            match opcode {
                0 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::GetGammaControl {
                        id: {
                            let me =
                                Resource::<ZwlrGammaControlManagerV1>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zwlr_gamma_control_v1::ZwlrGammaControlV1>(
                                _args[0].n,
                            )
                            .unwrap()
                        },
                        output: Resource::<super::wl_output::WlOutput>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                    })
                }
                1 => Ok(Request::Destroy),
                _ => return Err(()),
            }
        }
        fn as_raw_c_in<F, T>(self, f: F) -> T
        where
            F: FnOnce(u32, &mut [wl_argument]) -> T,
        {
            panic!("Request::as_raw_c_in can not be used Server-side.")
        }
    }
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Event {}
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {}
        }
        fn opcode(&self) -> u16 {
            match *self {}
        }
        fn since(&self) -> u32 {
            match *self {}
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            panic!("Event::from_raw can not be used Server-side.")
        }
        fn into_raw(self, sender_id: u32) -> Message {
            match self {}
        }
        unsafe fn from_raw_c(
            obj: *mut ::std::os::raw::c_void,
            opcode: u32,
            args: *const wl_argument,
        ) -> Result<Event, ()> {
            panic!("Event::from_raw_c can not be used Server-side.")
        }
        fn as_raw_c_in<F, T>(self, f: F) -> T
        where
            F: FnOnce(u32, &mut [wl_argument]) -> T,
        {
            match self {}
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwlrGammaControlManagerV1(Resource<ZwlrGammaControlManagerV1>);
    impl AsRef<Resource<ZwlrGammaControlManagerV1>> for ZwlrGammaControlManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrGammaControlManagerV1>> for ZwlrGammaControlManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrGammaControlManagerV1(value)
        }
    }
    impl From<ZwlrGammaControlManagerV1> for Resource<ZwlrGammaControlManagerV1> {
        #[inline]
        fn from(value: ZwlrGammaControlManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrGammaControlManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrGammaControlManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_gamma_control_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_gamma_control_manager_v1_interface }
        }
    }
    impl ZwlrGammaControlManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_GAMMA_CONTROL_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    static mut zwlr_gamma_control_manager_v1_requests_get_gamma_control_types:
        [*const wl_interface; 2] = [
        unsafe {
            &super::zwlr_gamma_control_v1::zwlr_gamma_control_v1_interface as *const wl_interface
        },
        unsafe { &super::wl_output::wl_output_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_gamma_control_manager_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"get_gamma_control\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_gamma_control_manager_v1_requests_get_gamma_control_types as *const _
            },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_gamma_control_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_gamma_control_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwlr_gamma_control_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "adjust gamma tables for an output\n\nThis interface allows a client to adjust gamma tables for a particular\noutput.\n\nThe client will receive the gamma size, and will then be able to set gamma\ntables. At any time the compositor can send a failed event indicating that\nthis object is no longer valid.\n\nThere can only be at most one gamma control object per output, which\nhas exclusive access to this particular output. When the gamma control\nobject is destroyed, the gamma table is restored to its original value."]
pub mod zwlr_gamma_control_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Error {
        #[doc = "invalid gamma tables"]
        InvalidGamma = 1,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                1 => Some(Error::InvalidGamma),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Request {
        #[doc = "set the gamma table\n\nSet the gamma table. The file descriptor can be memory-mapped to provide\nthe raw gamma table, which contains successive gamma ramps for the red,\ngreen and blue channels. Each gamma ramp is an array of 16-byte unsigned\nintegers which has the same length as the gamma size.\n\nThe file descriptor data must have the same length as three times the\ngamma size."]
        SetGamma { fd: ::std::os::unix::io::RawFd },
        #[doc = "destroy this control\n\nDestroys the gamma control object. If the object is still valid, this\nrestores the original gamma tables.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "set_gamma",
                since: 1,
                signature: &[super::ArgumentType::Fd],
                destructor: false,
            },
            super::MessageDesc {
                name: "destroy",
                since: 1,
                signature: &[],
                destructor: true,
            },
        ];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Request::Destroy => true,
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::SetGamma { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::SetGamma { .. } => 1,
                Request::Destroy => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::SetGamma {
                        fd: {
                            if let Some(Argument::Fd(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                1 => Ok(Request::Destroy),
                _ => Err(()),
            }
        }
        fn into_raw(self, sender_id: u32) -> Message {
            panic!("Request::into_raw can not be used Server-side.")
        }
        unsafe fn from_raw_c(
            obj: *mut ::std::os::raw::c_void,
            opcode: u32,
            args: *const wl_argument,
        ) -> Result<Request, ()> {
            match opcode {
                0 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::SetGamma { fd: _args[0].h })
                }
                1 => Ok(Request::Destroy),
                _ => return Err(()),
            }
        }
        fn as_raw_c_in<F, T>(self, f: F) -> T
        where
            F: FnOnce(u32, &mut [wl_argument]) -> T,
        {
            panic!("Request::as_raw_c_in can not be used Server-side.")
        }
    }
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum Event {
        #[doc = "size of gamma ramps\n\nAdvertise the size of each gamma ramp.\n\nThis event is sent immediately when the gamma control object is created."]
        GammaSize { size: u32 },
        #[doc = "object no longer valid\n\nThis event indicates that the gamma control is no longer valid. This\ncan happen for a number of reasons, including:\n- The output doesn't support gamma tables\n- Setting the gamma tables failed\n- Another client already has exclusive gamma control for this output\n- The compositor has transferred gamma control to another client\n\nUpon receiving this event, the client should destroy this object."]
        Failed,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "gamma_size",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "failed",
                since: 1,
                signature: &[],
                destructor: false,
            },
        ];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Event::GammaSize { .. } => 0,
                Event::Failed => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::GammaSize { .. } => 1,
                Event::Failed => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            panic!("Event::from_raw can not be used Server-side.")
        }
        fn into_raw(self, sender_id: u32) -> Message {
            match self {
                Event::GammaSize { size } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Uint(size),],
                },
                Event::Failed => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![],
                },
            }
        }
        unsafe fn from_raw_c(
            obj: *mut ::std::os::raw::c_void,
            opcode: u32,
            args: *const wl_argument,
        ) -> Result<Event, ()> {
            panic!("Event::from_raw_c can not be used Server-side.")
        }
        fn as_raw_c_in<F, T>(self, f: F) -> T
        where
            F: FnOnce(u32, &mut [wl_argument]) -> T,
        {
            match self {
                Event::GammaSize { size } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = size;
                    f(0, &mut _args_array)
                }
                Event::Failed => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwlrGammaControlV1(Resource<ZwlrGammaControlV1>);
    impl AsRef<Resource<ZwlrGammaControlV1>> for ZwlrGammaControlV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrGammaControlV1>> for ZwlrGammaControlV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrGammaControlV1(value)
        }
    }
    impl From<ZwlrGammaControlV1> for Resource<ZwlrGammaControlV1> {
        #[inline]
        fn from(value: ZwlrGammaControlV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrGammaControlV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrGammaControlV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_gamma_control_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_gamma_control_v1_interface }
        }
    }
    impl ZwlrGammaControlV1 {
        #[doc = "size of gamma ramps\n\nAdvertise the size of each gamma ramp.\n\nThis event is sent immediately when the gamma control object is created."]
        pub fn gamma_size(&self, size: u32) -> () {
            let msg = Event::GammaSize { size: size };
            self.0.send(msg);
        }
        #[doc = "object no longer valid\n\nThis event indicates that the gamma control is no longer valid. This\ncan happen for a number of reasons, including:\n- The output doesn't support gamma tables\n- Setting the gamma tables failed\n- Another client already has exclusive gamma control for this output\n- The compositor has transferred gamma control to another client\n\nUpon receiving this event, the client should destroy this object."]
        pub fn failed(&self) -> () {
            let msg = Event::Failed;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_GAMMA_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_GAMMA_SIZE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_FAILED_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_gamma_control_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"set_gamma\0" as *const u8 as *const c_char,
            signature: b"h\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_gamma_control_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"gamma_size\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"failed\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_gamma_control_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_gamma_control_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwlr_gamma_control_v1_requests as *const _ },
        event_count: 2,
        events: unsafe { &zwlr_gamma_control_v1_events as *const _ },
    };
}
