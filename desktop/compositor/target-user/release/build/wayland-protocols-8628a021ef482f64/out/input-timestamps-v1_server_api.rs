use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 3] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "context object for high-resolution input timestamps\n\nA global interface used for requesting high-resolution timestamps\nfor input events."]
pub mod zwp_input_timestamps_manager_v1 {
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
        #[doc = "destroy the input timestamps manager object\n\nInforms the server that the client will no longer be using this\nprotocol object. Existing objects created by this object are not\naffected.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "subscribe to high-resolution keyboard timestamp events\n\nCreates a new input timestamps object that represents a subscription\nto high-resolution timestamp events for all wl_keyboard events that\ncarry a timestamp.\n\nIf the associated wl_keyboard object is invalidated, either through\nclient action (e.g. release) or server-side changes, the input\ntimestamps object becomes inert and the client should destroy it\nby calling zwp_input_timestamps_v1.destroy."]
        GetKeyboardTimestamps {
            id: Main<super::zwp_input_timestamps_v1::ZwpInputTimestampsV1>,
            keyboard: super::wl_keyboard::WlKeyboard,
        },
        #[doc = "subscribe to high-resolution pointer timestamp events\n\nCreates a new input timestamps object that represents a subscription\nto high-resolution timestamp events for all wl_pointer events that\ncarry a timestamp.\n\nIf the associated wl_pointer object is invalidated, either through\nclient action (e.g. release) or server-side changes, the input\ntimestamps object becomes inert and the client should destroy it\nby calling zwp_input_timestamps_v1.destroy."]
        GetPointerTimestamps {
            id: Main<super::zwp_input_timestamps_v1::ZwpInputTimestampsV1>,
            pointer: super::wl_pointer::WlPointer,
        },
        #[doc = "subscribe to high-resolution touch timestamp events\n\nCreates a new input timestamps object that represents a subscription\nto high-resolution timestamp events for all wl_touch events that\ncarry a timestamp.\n\nIf the associated wl_touch object becomes invalid, either through\nclient action (e.g. release) or server-side changes, the input\ntimestamps object becomes inert and the client should destroy it\nby calling zwp_input_timestamps_v1.destroy."]
        GetTouchTimestamps {
            id: Main<super::zwp_input_timestamps_v1::ZwpInputTimestampsV1>,
            touch: super::wl_touch::WlTouch,
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
                name: "get_keyboard_timestamps",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "get_pointer_timestamps",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "get_touch_timestamps",
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
                Request::GetKeyboardTimestamps { .. } => 1,
                Request::GetPointerTimestamps { .. } => 2,
                Request::GetTouchTimestamps { .. } => 3,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Destroy => 1,
                Request::GetKeyboardTimestamps { .. } => 1,
                Request::GetPointerTimestamps { .. } => 1,
                Request::GetTouchTimestamps { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                1 => Some(Object::from_interface::<
                    super::zwp_input_timestamps_v1::ZwpInputTimestampsV1,
                >(version, meta.child())),
                2 => Some(Object::from_interface::<
                    super::zwp_input_timestamps_v1::ZwpInputTimestampsV1,
                >(version, meta.child())),
                3 => Some(Object::from_interface::<
                    super::zwp_input_timestamps_v1::ZwpInputTimestampsV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => Ok(Request::Destroy),
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetKeyboardTimestamps {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        keyboard: {
                            if let Some(Argument::Object(val)) = args.next() {
                                map.get(val).ok_or(())?.into()
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetPointerTimestamps {
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
                3 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetTouchTimestamps {
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        touch: {
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
                    Ok(Request::GetKeyboardTimestamps {
                        id: {
                            let me =
                                Resource::<ZwpInputTimestampsManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_input_timestamps_v1 :: ZwpInputTimestampsV1 > (_args [0] . n) . unwrap ()
                        },
                        keyboard: Resource::<super::wl_keyboard::WlKeyboard>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                    })
                }
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::GetPointerTimestamps {
                        id: {
                            let me =
                                Resource::<ZwpInputTimestampsManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_input_timestamps_v1 :: ZwpInputTimestampsV1 > (_args [0] . n) . unwrap ()
                        },
                        pointer: Resource::<super::wl_pointer::WlPointer>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                    })
                }
                3 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::GetTouchTimestamps {
                        id: {
                            let me =
                                Resource::<ZwpInputTimestampsManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_input_timestamps_v1 :: ZwpInputTimestampsV1 > (_args [0] . n) . unwrap ()
                        },
                        touch: Resource::<super::wl_touch::WlTouch>::from_c_ptr(
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
    pub struct ZwpInputTimestampsManagerV1(Resource<ZwpInputTimestampsManagerV1>);
    impl AsRef<Resource<ZwpInputTimestampsManagerV1>> for ZwpInputTimestampsManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputTimestampsManagerV1>> for ZwpInputTimestampsManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputTimestampsManagerV1(value)
        }
    }
    impl From<ZwpInputTimestampsManagerV1> for Resource<ZwpInputTimestampsManagerV1> {
        #[inline]
        fn from(value: ZwpInputTimestampsManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputTimestampsManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputTimestampsManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_timestamps_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_timestamps_manager_v1_interface }
        }
    }
    impl ZwpInputTimestampsManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_KEYBOARD_TIMESTAMPS_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_POINTER_TIMESTAMPS_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_TOUCH_TIMESTAMPS_SINCE: u32 = 1u32;
    static mut zwp_input_timestamps_manager_v1_requests_get_keyboard_timestamps_types:
        [*const wl_interface; 2] = [
        unsafe {
            &super::zwp_input_timestamps_v1::zwp_input_timestamps_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_keyboard::wl_keyboard_interface as *const wl_interface },
    ];
    static mut zwp_input_timestamps_manager_v1_requests_get_pointer_timestamps_types:
        [*const wl_interface; 2] = [
        unsafe {
            &super::zwp_input_timestamps_v1::zwp_input_timestamps_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_pointer::wl_pointer_interface as *const wl_interface },
    ];
    static mut zwp_input_timestamps_manager_v1_requests_get_touch_timestamps_types:
        [*const wl_interface; 2] = [
        unsafe {
            &super::zwp_input_timestamps_v1::zwp_input_timestamps_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_touch::wl_touch_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_timestamps_manager_v1_requests: [wl_message; 4] = [
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"get_keyboard_timestamps\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_input_timestamps_manager_v1_requests_get_keyboard_timestamps_types as *const _
            },
        },
        wl_message {
            name: b"get_pointer_timestamps\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_input_timestamps_manager_v1_requests_get_pointer_timestamps_types as *const _
            },
        },
        wl_message {
            name: b"get_touch_timestamps\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwp_input_timestamps_manager_v1_requests_get_touch_timestamps_types as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_timestamps_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwp_input_timestamps_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 4,
        requests: unsafe { &zwp_input_timestamps_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "context object for input timestamps\n\nProvides high-resolution timestamp events for a set of subscribed input\nevents. The set of subscribed input events is determined by the\nzwp_input_timestamps_manager_v1 request used to create this object."]
pub mod zwp_input_timestamps_v1 {
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
        #[doc = "destroy the input timestamps object\n\nInforms the server that the client will no longer be using this\nprotocol object. After the server processes the request, no more\ntimestamp events will be emitted.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "high-resolution timestamp event\n\nThe timestamp event is associated with the first subsequent input event\ncarrying a timestamp which belongs to the set of input events this\nobject is subscribed to.\n\nThe timestamp provided by this event is a high-resolution version of\nthe timestamp argument of the associated input event. The provided\ntimestamp is in the same clock domain and is at least as accurate as\nthe associated input event timestamp.\n\nThe timestamp is expressed as tv_sec_hi, tv_sec_lo, tv_nsec triples,\neach component being an unsigned 32-bit value. Whole seconds are in\ntv_sec which is a 64-bit value combined from tv_sec_hi and tv_sec_lo,\nand the additional fractional part in tv_nsec as nanoseconds. Hence,\nfor valid timestamps tv_nsec must be in [0, 999999999]."]
        Timestamp {
            tv_sec_hi: u32,
            tv_sec_lo: u32,
            tv_nsec: u32,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[super::MessageDesc {
            name: "timestamp",
            since: 1,
            signature: &[
                super::ArgumentType::Uint,
                super::ArgumentType::Uint,
                super::ArgumentType::Uint,
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
                Event::Timestamp { .. } => 0,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Timestamp { .. } => 1,
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
                Event::Timestamp {
                    tv_sec_hi,
                    tv_sec_lo,
                    tv_nsec,
                } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Uint(tv_sec_hi),
                        Argument::Uint(tv_sec_lo),
                        Argument::Uint(tv_nsec),
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
                Event::Timestamp {
                    tv_sec_hi,
                    tv_sec_lo,
                    tv_nsec,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = tv_sec_hi;
                    _args_array[1].u = tv_sec_lo;
                    _args_array[2].u = tv_nsec;
                    f(0, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpInputTimestampsV1(Resource<ZwpInputTimestampsV1>);
    impl AsRef<Resource<ZwpInputTimestampsV1>> for ZwpInputTimestampsV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpInputTimestampsV1>> for ZwpInputTimestampsV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpInputTimestampsV1(value)
        }
    }
    impl From<ZwpInputTimestampsV1> for Resource<ZwpInputTimestampsV1> {
        #[inline]
        fn from(value: ZwpInputTimestampsV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpInputTimestampsV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpInputTimestampsV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_input_timestamps_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_input_timestamps_v1_interface }
        }
    }
    impl ZwpInputTimestampsV1 {
        #[doc = "high-resolution timestamp event\n\nThe timestamp event is associated with the first subsequent input event\ncarrying a timestamp which belongs to the set of input events this\nobject is subscribed to.\n\nThe timestamp provided by this event is a high-resolution version of\nthe timestamp argument of the associated input event. The provided\ntimestamp is in the same clock domain and is at least as accurate as\nthe associated input event timestamp.\n\nThe timestamp is expressed as tv_sec_hi, tv_sec_lo, tv_nsec triples,\neach component being an unsigned 32-bit value. Whole seconds are in\ntv_sec which is a 64-bit value combined from tv_sec_hi and tv_sec_lo,\nand the additional fractional part in tv_nsec as nanoseconds. Hence,\nfor valid timestamps tv_nsec must be in [0, 999999999]."]
        pub fn timestamp(&self, tv_sec_hi: u32, tv_sec_lo: u32, tv_nsec: u32) -> () {
            let msg = Event::Timestamp {
                tv_sec_hi: tv_sec_hi,
                tv_sec_lo: tv_sec_lo,
                tv_nsec: tv_nsec,
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_TIMESTAMP_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_timestamps_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_input_timestamps_v1_events: [wl_message; 1] = [wl_message {
        name: b"timestamp\0" as *const u8 as *const c_char,
        signature: b"uuu\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_input_timestamps_v1_interface: wl_interface = wl_interface {
        name: b"zwp_input_timestamps_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_input_timestamps_v1_requests as *const _ },
        event_count: 1,
        events: unsafe { &zwp_input_timestamps_v1_events as *const _ },
    };
}
