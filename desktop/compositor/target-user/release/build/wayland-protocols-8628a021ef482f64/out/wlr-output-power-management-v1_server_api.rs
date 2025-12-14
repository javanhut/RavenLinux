use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 1] =
    [NULLPTR as *const sys::common::wl_interface];
#[doc = "manager to create per-output power management\n\nThis interface is a manager that allows creating per-output power\nmanagement mode controls."]
pub mod zwlr_output_power_manager_v1 {
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
        #[doc = "get a power management for an output\n\nCreate an output power management mode control that can be used to\nadjust the power management mode for a given output."]
        GetOutputPower {
            id: Main<super::zwlr_output_power_v1::ZwlrOutputPowerV1>,
            output: super::wl_output::WlOutput,
        },
        #[doc = "destroy the manager\n\nAll objects created by the manager will still remain valid, until their\nappropriate destroy request has been called.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "get_output_power",
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
                Request::GetOutputPower { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::GetOutputPower { .. } => 1,
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
                    super::zwlr_output_power_v1::ZwlrOutputPowerV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetOutputPower {
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
                    Ok(Request::GetOutputPower {
                        id: {
                            let me =
                                Resource::<ZwlrOutputPowerManagerV1>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zwlr_output_power_v1::ZwlrOutputPowerV1>(
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
    pub struct ZwlrOutputPowerManagerV1(Resource<ZwlrOutputPowerManagerV1>);
    impl AsRef<Resource<ZwlrOutputPowerManagerV1>> for ZwlrOutputPowerManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrOutputPowerManagerV1>> for ZwlrOutputPowerManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrOutputPowerManagerV1(value)
        }
    }
    impl From<ZwlrOutputPowerManagerV1> for Resource<ZwlrOutputPowerManagerV1> {
        #[inline]
        fn from(value: ZwlrOutputPowerManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrOutputPowerManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrOutputPowerManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_output_power_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_output_power_manager_v1_interface }
        }
    }
    impl ZwlrOutputPowerManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_OUTPUT_POWER_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    static mut zwlr_output_power_manager_v1_requests_get_output_power_types: [*const wl_interface;
        2] = [
        unsafe {
            &super::zwlr_output_power_v1::zwlr_output_power_v1_interface as *const wl_interface
        },
        unsafe { &super::wl_output::wl_output_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_output_power_manager_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"get_output_power\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_output_power_manager_v1_requests_get_output_power_types as *const _
            },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_output_power_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_output_power_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwlr_output_power_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "adjust power management mode for an output\n\nThis object offers requests to set the power management mode of\nan output."]
pub mod zwlr_output_power_v1 {
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
    pub enum Mode {
        #[doc = "Output is turned off."]
        Off = 0,
        #[doc = "Output is turned on, no power saving"]
        On = 1,
    }
    impl Mode {
        pub fn from_raw(n: u32) -> Option<Mode> {
            match n {
                0 => Some(Mode::Off),
                1 => Some(Mode::On),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Error {
        #[doc = "nonexistent power save mode"]
        InvalidMode = 1,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                1 => Some(Error::InvalidMode),
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
        #[doc = "Set an outputs power save mode\n\nSet an output's power save mode to the given mode. The mode change\nis effective immediately. If the output does not support the given\nmode a failed event is sent."]
        SetMode { mode: Mode },
        #[doc = "destroy this power management\n\nDestroys the output power management mode control object.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "set_mode",
                since: 1,
                signature: &[super::ArgumentType::Uint],
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
                Request::SetMode { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::SetMode { .. } => 1,
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
                    Ok(Request::SetMode {
                        mode: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                Mode::from_raw(val).ok_or(())?
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
                    Ok(Request::SetMode {
                        mode: Mode::from_raw(_args[0].u).ok_or(())?,
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
    pub enum Event {
        #[doc = "Report a power management mode change\n\nReport the power management mode change of an output.\n\nThe mode event is sent after an output changed its power\nmanagement mode. The reason can be a client using set_mode or the\ncompositor deciding to change an output's mode.\nThis event is also sent immediately when the object is created\nso the client is informed about the current power management mode."]
        Mode { mode: Mode },
        #[doc = "object no longer valid\n\nThis event indicates that the output power management mode control\nis no longer valid. This can happen for a number of reasons,\nincluding:\n- The output doesn't support power management\n- Another client already has exclusive power management mode control\nfor this output\n- The output disappeared\n\nUpon receiving this event, the client should destroy this object."]
        Failed,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "mode",
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
                Event::Mode { .. } => 0,
                Event::Failed => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Mode { .. } => 1,
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
                Event::Mode { mode } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Uint(mode.to_raw()),],
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
                Event::Mode { mode } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = mode.to_raw();
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
    pub struct ZwlrOutputPowerV1(Resource<ZwlrOutputPowerV1>);
    impl AsRef<Resource<ZwlrOutputPowerV1>> for ZwlrOutputPowerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrOutputPowerV1>> for ZwlrOutputPowerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrOutputPowerV1(value)
        }
    }
    impl From<ZwlrOutputPowerV1> for Resource<ZwlrOutputPowerV1> {
        #[inline]
        fn from(value: ZwlrOutputPowerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrOutputPowerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrOutputPowerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_output_power_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_output_power_v1_interface }
        }
    }
    impl ZwlrOutputPowerV1 {
        #[doc = "Report a power management mode change\n\nReport the power management mode change of an output.\n\nThe mode event is sent after an output changed its power\nmanagement mode. The reason can be a client using set_mode or the\ncompositor deciding to change an output's mode.\nThis event is also sent immediately when the object is created\nso the client is informed about the current power management mode."]
        pub fn mode(&self, mode: Mode) -> () {
            let msg = Event::Mode { mode: mode };
            self.0.send(msg);
        }
        #[doc = "object no longer valid\n\nThis event indicates that the output power management mode control\nis no longer valid. This can happen for a number of reasons,\nincluding:\n- The output doesn't support power management\n- Another client already has exclusive power management mode control\nfor this output\n- The output disappeared\n\nUpon receiving this event, the client should destroy this object."]
        pub fn failed(&self) -> () {
            let msg = Event::Failed;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_MODE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_MODE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_FAILED_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_output_power_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"set_mode\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_output_power_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"mode\0" as *const u8 as *const c_char,
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
    pub static mut zwlr_output_power_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_output_power_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwlr_output_power_v1_requests as *const _ },
        event_count: 2,
        events: unsafe { &zwlr_output_power_v1_events as *const _ },
    };
}
