use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 6] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "get relative pointer objects\n\nA global interface used for getting the relative pointer object for a\ngiven pointer."]
pub mod zwp_relative_pointer_manager_v1 {
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
        #[doc = "destroy the relative pointer manager object\n\nUsed by the client to notify the server that it will no longer use this\nrelative pointer manager object.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "get a relative pointer object\n\nCreate a relative pointer interface given a wl_pointer object. See the\nwp_relative_pointer interface for more details."]
        GetRelativePointer {
            id: Main<super::zwp_relative_pointer_v1::ZwpRelativePointerV1>,
            pointer: super::wl_pointer::WlPointer,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "destroy",
                since: 1,
                signature: &[],
                destructor: true,
            },
            super::MessageDesc {
                name: "get_relative_pointer",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
                destructor: false,
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
                Request::Destroy => 0,
                Request::GetRelativePointer { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::GetRelativePointer { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                1 => Some(Object::from_interface::<
                    super::zwp_relative_pointer_v1::ZwpRelativePointerV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetRelativePointer {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        pointer: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
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
                0 => Ok(Request::Destroy),
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::GetRelativePointer {
                        id: {
                            let me =
                                Resource::<ZwpRelativePointerManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_relative_pointer_v1 :: ZwpRelativePointerV1 > (_args [0] . n) . unwrap ()
                        },
                        pointer: Resource::<super::wl_pointer::WlPointer>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                    })
                }
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
    pub struct ZwpRelativePointerManagerV1(Resource<ZwpRelativePointerManagerV1>);
    impl AsRef<Resource<ZwpRelativePointerManagerV1>> for ZwpRelativePointerManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpRelativePointerManagerV1>> for ZwpRelativePointerManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpRelativePointerManagerV1(value)
        }
    }
    impl From<ZwpRelativePointerManagerV1> for Resource<ZwpRelativePointerManagerV1> {
        #[inline]
        fn from(value: ZwpRelativePointerManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpRelativePointerManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpRelativePointerManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_relative_pointer_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_relative_pointer_manager_v1_interface }
        }
    }
    impl ZwpRelativePointerManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_RELATIVE_POINTER_SINCE: u32 = 1u32;
    static mut zwp_relative_pointer_manager_v1_requests_get_relative_pointer_types:
        [*const wl_interface; 2] = [
        unsafe {
            &super::zwp_relative_pointer_v1::zwp_relative_pointer_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_pointer::wl_pointer_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_relative_pointer_manager_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"get_relative_pointer\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_relative_pointer_manager_v1_requests_get_relative_pointer_types as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_relative_pointer_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwp_relative_pointer_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwp_relative_pointer_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "relative pointer object\n\nA wp_relative_pointer object is an extension to the wl_pointer interface\nused for emitting relative pointer events. It shares the same focus as\nwl_pointer objects of the same seat and will only emit events when it has\nfocus."]
pub mod zwp_relative_pointer_v1 {
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
        #[doc = "release the relative pointer object\n\n\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "destroy",
            since: 1,
            signature: &[],
            destructor: true,
        }];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Request::Destroy => true,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::Destroy => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
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
                0 => Ok(Request::Destroy),
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
                0 => Ok(Request::Destroy),
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
        #[doc = "relative pointer motion\n\nRelative x/y pointer motion from the pointer of the seat associated with\nthis object.\n\nA relative motion is in the same dimension as regular wl_pointer motion\nevents, except they do not represent an absolute position. For example,\nmoving a pointer from (x, y) to (x', y') would have the equivalent\nrelative motion (x' - x, y' - y). If a pointer motion caused the\nabsolute pointer position to be clipped by for example the edge of the\nmonitor, the relative motion is unaffected by the clipping and will\nrepresent the unclipped motion.\n\nThis event also contains non-accelerated motion deltas. The\nnon-accelerated delta is, when applicable, the regular pointer motion\ndelta as it was before having applied motion acceleration and other\ntransformations such as normalization.\n\nNote that the non-accelerated delta does not represent 'raw' events as\nthey were read from some device. Pointer motion acceleration is device-\nand configuration-specific and non-accelerated deltas and accelerated\ndeltas may have the same value on some devices.\n\nRelative motions are not coupled to wl_pointer.motion events, and can be\nsent in combination with such events, but also independently. There may\nalso be scenarios where wl_pointer.motion is sent, but there is no\nrelative motion. The order of an absolute and relative motion event\noriginating from the same physical motion is not guaranteed.\n\nIf the client needs button events or focus state, it can receive them\nfrom a wl_pointer object of the same seat that the wp_relative_pointer\nobject is associated with."]
        RelativeMotion {
            utime_hi: u32,
            utime_lo: u32,
            dx: f64,
            dy: f64,
            dx_unaccel: f64,
            dy_unaccel: f64,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "relative_motion",
            since: 1,
            signature: &[
                super::ArgumentType::Uint,
                super::ArgumentType::Uint,
                super::ArgumentType::Fixed,
                super::ArgumentType::Fixed,
                super::ArgumentType::Fixed,
                super::ArgumentType::Fixed,
            ],
            destructor: false,
        }];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Event::RelativeMotion { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::RelativeMotion { .. } => 1,
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
                Event::RelativeMotion {
                    utime_hi,
                    utime_lo,
                    dx,
                    dy,
                    dx_unaccel,
                    dy_unaccel,
                } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Uint(utime_hi),
                        Argument::Uint(utime_lo),
                        Argument::Fixed((dx * 256.) as i32),
                        Argument::Fixed((dy * 256.) as i32),
                        Argument::Fixed((dx_unaccel * 256.) as i32),
                        Argument::Fixed((dy_unaccel * 256.) as i32),
                    ],
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
                Event::RelativeMotion {
                    utime_hi,
                    utime_lo,
                    dx,
                    dy,
                    dx_unaccel,
                    dy_unaccel,
                } => {
                    let mut _args_array: [wl_argument; 6] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = utime_hi;
                    _args_array[1].u = utime_lo;
                    _args_array[2].f = (dx * 256.) as i32;
                    _args_array[3].f = (dy * 256.) as i32;
                    _args_array[4].f = (dx_unaccel * 256.) as i32;
                    _args_array[5].f = (dy_unaccel * 256.) as i32;
                    f(0, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpRelativePointerV1(Resource<ZwpRelativePointerV1>);
    impl AsRef<Resource<ZwpRelativePointerV1>> for ZwpRelativePointerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpRelativePointerV1>> for ZwpRelativePointerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpRelativePointerV1(value)
        }
    }
    impl From<ZwpRelativePointerV1> for Resource<ZwpRelativePointerV1> {
        #[inline]
        fn from(value: ZwpRelativePointerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpRelativePointerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpRelativePointerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_relative_pointer_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_relative_pointer_v1_interface }
        }
    }
    impl ZwpRelativePointerV1 {
        #[doc = "relative pointer motion\n\nRelative x/y pointer motion from the pointer of the seat associated with\nthis object.\n\nA relative motion is in the same dimension as regular wl_pointer motion\nevents, except they do not represent an absolute position. For example,\nmoving a pointer from (x, y) to (x', y') would have the equivalent\nrelative motion (x' - x, y' - y). If a pointer motion caused the\nabsolute pointer position to be clipped by for example the edge of the\nmonitor, the relative motion is unaffected by the clipping and will\nrepresent the unclipped motion.\n\nThis event also contains non-accelerated motion deltas. The\nnon-accelerated delta is, when applicable, the regular pointer motion\ndelta as it was before having applied motion acceleration and other\ntransformations such as normalization.\n\nNote that the non-accelerated delta does not represent 'raw' events as\nthey were read from some device. Pointer motion acceleration is device-\nand configuration-specific and non-accelerated deltas and accelerated\ndeltas may have the same value on some devices.\n\nRelative motions are not coupled to wl_pointer.motion events, and can be\nsent in combination with such events, but also independently. There may\nalso be scenarios where wl_pointer.motion is sent, but there is no\nrelative motion. The order of an absolute and relative motion event\noriginating from the same physical motion is not guaranteed.\n\nIf the client needs button events or focus state, it can receive them\nfrom a wl_pointer object of the same seat that the wp_relative_pointer\nobject is associated with."]
        pub fn relative_motion(
            &self,
            utime_hi: u32,
            utime_lo: u32,
            dx: f64,
            dy: f64,
            dx_unaccel: f64,
            dy_unaccel: f64,
        ) -> () {
            let msg = Event::RelativeMotion {
                utime_hi: utime_hi,
                utime_lo: utime_lo,
                dx: dx,
                dy: dy,
                dx_unaccel: dx_unaccel,
                dy_unaccel: dy_unaccel,
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_RELATIVE_MOTION_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_relative_pointer_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_relative_pointer_v1_events: [wl_message; 1] = [wl_message {
        name: b"relative_motion\0" as *const u8 as *const c_char,
        signature: b"uuffff\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_relative_pointer_v1_interface: wl_interface = wl_interface {
        name: b"zwp_relative_pointer_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_relative_pointer_v1_requests as *const _ },
        event_count: 1,
        events: unsafe { &zwp_relative_pointer_v1_events as *const _ },
    };
}
