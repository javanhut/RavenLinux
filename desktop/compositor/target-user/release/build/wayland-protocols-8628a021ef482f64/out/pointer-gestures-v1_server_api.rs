use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 5] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "touchpad gestures\n\nA global interface to provide semantic touchpad gestures for a given\npointer.\n\nTwo gestures are currently supported: swipe and pinch.\nAll gestures follow a three-stage cycle: begin, update, end and\nare identified by a unique id.\n\nWarning! The protocol described in this file is experimental and\nbackward incompatible changes may be made. Backward compatible changes\nmay be added together with the corresponding interface version bump.\nBackward incompatible changes are done by bumping the version number in\nthe protocol and interface names and resetting the interface version.\nOnce the protocol is to be declared stable, the 'z' prefix and the\nversion number in the protocol and interface names are removed and the\ninterface version number is reset."]
pub mod zwp_pointer_gestures_v1 {
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
        #[doc = "get swipe gesture\n\nCreate a swipe gesture object. See the\nwl_pointer_gesture_swipe interface for details."]
        GetSwipeGesture {
            id: Main<super::zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1>,
            pointer: super::wl_pointer::WlPointer,
        },
        #[doc = "get pinch gesture\n\nCreate a pinch gesture object. See the\nwl_pointer_gesture_pinch interface for details."]
        GetPinchGesture {
            id: Main<super::zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1>,
            pointer: super::wl_pointer::WlPointer,
        },
        #[doc = "destroy the pointer gesture object\n\nDestroy the pointer gesture object. Swipe and pinch objects created via this\ngesture object remain valid.\n\nThis is a destructor, once received this object cannot be used any longer.\nOnly available since version 2 of the interface"]
        Release,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "get_swipe_gesture",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "get_pinch_gesture",
                since: 1,
                signature: &[super::ArgumentType::NewId, super::ArgumentType::Object],
                destructor: false,
            },
            super::MessageDesc {
                name: "release",
                since: 2,
                signature: &[],
                destructor: true,
            },
        ];
        type Map = super::ResourceMap;
        fn is_destructor(&self) -> bool {
            match *self {
                Request::Release => true,
                _ => false,
            }
        }
        fn opcode(&self) -> u16 {
            match *self {
                Request::GetSwipeGesture { .. } => 0,
                Request::GetPinchGesture { .. } => 1,
                Request::Release => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::GetSwipeGesture { .. } => 1,
                Request::GetPinchGesture { .. } => 1,
                Request::Release => 2,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwp_pointer_gesture_swipe_v1::ZwpPointerGestureSwipeV1,
                >(version, meta.child())),
                1 => Some(Object::from_interface::<
                    super::zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetSwipeGesture {
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
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetPinchGesture {
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
                2 => Ok(Request::Release),
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
                    Ok(Request::GetSwipeGesture {
                        id: {
                            let me = Resource::<ZwpPointerGesturesV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_pointer_gesture_swipe_v1 :: ZwpPointerGestureSwipeV1 > (_args [0] . n) . unwrap ()
                        },
                        pointer: Resource::<super::wl_pointer::WlPointer>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                    })
                }
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::GetPinchGesture {
                        id: {
                            let me = Resource::<ZwpPointerGesturesV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwp_pointer_gesture_pinch_v1 :: ZwpPointerGesturePinchV1 > (_args [0] . n) . unwrap ()
                        },
                        pointer: Resource::<super::wl_pointer::WlPointer>::from_c_ptr(
                            _args[1].o as *mut _,
                        )
                        .into(),
                    })
                }
                2 => Ok(Request::Release),
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
    pub struct ZwpPointerGesturesV1(Resource<ZwpPointerGesturesV1>);
    impl AsRef<Resource<ZwpPointerGesturesV1>> for ZwpPointerGesturesV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpPointerGesturesV1>> for ZwpPointerGesturesV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpPointerGesturesV1(value)
        }
    }
    impl From<ZwpPointerGesturesV1> for Resource<ZwpPointerGesturesV1> {
        #[inline]
        fn from(value: ZwpPointerGesturesV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpPointerGesturesV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpPointerGesturesV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_pointer_gestures_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_pointer_gestures_v1_interface }
        }
    }
    impl ZwpPointerGesturesV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_SWIPE_GESTURE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_PINCH_GESTURE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_RELEASE_SINCE: u32 = 2u32;
    static mut zwp_pointer_gestures_v1_requests_get_swipe_gesture_types: [*const wl_interface; 2] = [
        unsafe {
            &super::zwp_pointer_gesture_swipe_v1::zwp_pointer_gesture_swipe_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_pointer::wl_pointer_interface as *const wl_interface },
    ];
    static mut zwp_pointer_gestures_v1_requests_get_pinch_gesture_types: [*const wl_interface; 2] = [
        unsafe {
            &super::zwp_pointer_gesture_pinch_v1::zwp_pointer_gesture_pinch_v1_interface
                as *const wl_interface
        },
        unsafe { &super::wl_pointer::wl_pointer_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_pointer_gestures_v1_requests: [wl_message; 3] = [
        wl_message {
            name: b"get_swipe_gesture\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_pointer_gestures_v1_requests_get_swipe_gesture_types as *const _ },
        },
        wl_message {
            name: b"get_pinch_gesture\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_pointer_gestures_v1_requests_get_pinch_gesture_types as *const _ },
        },
        wl_message {
            name: b"release\0" as *const u8 as *const c_char,
            signature: b"2\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_pointer_gestures_v1_interface: wl_interface = wl_interface {
        name: b"zwp_pointer_gestures_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 3,
        requests: unsafe { &zwp_pointer_gestures_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "a swipe gesture object\n\nA swipe gesture object notifies a client about a multi-finger swipe\ngesture detected on an indirect input device such as a touchpad.\nThe gesture is usually initiated by multiple fingers moving in the\nsame direction but once initiated the direction may change.\nThe precise conditions of when such a gesture is detected are\nimplementation-dependent.\n\nA gesture consists of three stages: begin, update (optional) and end.\nThere cannot be multiple simultaneous pinch or swipe gestures on a\nsame pointer/seat, how compositors prevent these situations is\nimplementation-dependent.\n\nA gesture may be cancelled by the compositor or the hardware.\nClients should not consider performing permanent or irreversible\nactions until the end of a gesture has been received."]
pub mod zwp_pointer_gesture_swipe_v1 {
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
        #[doc = "destroy the pointer swipe gesture object\n\n\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "multi-finger swipe begin\n\nThis event is sent when a multi-finger swipe gesture is detected\non the device."]
        Begin {
            serial: u32,
            time: u32,
            surface: super::wl_surface::WlSurface,
            fingers: u32,
        },
        #[doc = "multi-finger swipe motion\n\nThis event is sent when a multi-finger swipe gesture changes the\nposition of the logical center.\n\nThe dx and dy coordinates are relative coordinates of the logical\ncenter of the gesture compared to the previous event."]
        Update { time: u32, dx: f64, dy: f64 },
        #[doc = "multi-finger swipe end\n\nThis event is sent when a multi-finger swipe gesture ceases to\nbe valid. This may happen when one or more fingers are lifted or\nthe gesture is cancelled.\n\nWhen a gesture is cancelled, the client should undo state changes\ncaused by this gesture. What causes a gesture to be cancelled is\nimplementation-dependent."]
        End {
            serial: u32,
            time: u32,
            cancelled: i32,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "begin",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Object,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "update",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Fixed,
                    super::ArgumentType::Fixed,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "end",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Int,
                ],
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
                Event::Begin { .. } => 0,
                Event::Update { .. } => 1,
                Event::End { .. } => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Begin { .. } => 1,
                Event::Update { .. } => 1,
                Event::End { .. } => 1,
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
                Event::Begin {
                    serial,
                    time,
                    surface,
                    fingers,
                } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(time),
                        Argument::Object(surface.as_ref().id()),
                        Argument::Uint(fingers),
                    ],
                },
                Event::Update { time, dx, dy } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![
                        Argument::Uint(time),
                        Argument::Fixed((dx * 256.) as i32),
                        Argument::Fixed((dy * 256.) as i32),
                    ],
                },
                Event::End {
                    serial,
                    time,
                    cancelled,
                } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(time),
                        Argument::Int(cancelled),
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
                Event::Begin {
                    serial,
                    time,
                    surface,
                    fingers,
                } => {
                    let mut _args_array: [wl_argument; 4] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = time;
                    _args_array[2].o = surface.as_ref().c_ptr() as *mut _;
                    _args_array[3].u = fingers;
                    f(0, &mut _args_array)
                }
                Event::Update { time, dx, dy } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = time;
                    _args_array[1].f = (dx * 256.) as i32;
                    _args_array[2].f = (dy * 256.) as i32;
                    f(1, &mut _args_array)
                }
                Event::End {
                    serial,
                    time,
                    cancelled,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = time;
                    _args_array[2].i = cancelled;
                    f(2, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpPointerGestureSwipeV1(Resource<ZwpPointerGestureSwipeV1>);
    impl AsRef<Resource<ZwpPointerGestureSwipeV1>> for ZwpPointerGestureSwipeV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpPointerGestureSwipeV1>> for ZwpPointerGestureSwipeV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpPointerGestureSwipeV1(value)
        }
    }
    impl From<ZwpPointerGestureSwipeV1> for Resource<ZwpPointerGestureSwipeV1> {
        #[inline]
        fn from(value: ZwpPointerGestureSwipeV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpPointerGestureSwipeV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpPointerGestureSwipeV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_pointer_gesture_swipe_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_pointer_gesture_swipe_v1_interface }
        }
    }
    impl ZwpPointerGestureSwipeV1 {
        #[doc = "multi-finger swipe begin\n\nThis event is sent when a multi-finger swipe gesture is detected\non the device."]
        pub fn begin(
            &self,
            serial: u32,
            time: u32,
            surface: &super::wl_surface::WlSurface,
            fingers: u32,
        ) -> () {
            let msg = Event::Begin {
                serial: serial,
                time: time,
                surface: surface.clone(),
                fingers: fingers,
            };
            self.0.send(msg);
        }
        #[doc = "multi-finger swipe motion\n\nThis event is sent when a multi-finger swipe gesture changes the\nposition of the logical center.\n\nThe dx and dy coordinates are relative coordinates of the logical\ncenter of the gesture compared to the previous event."]
        pub fn update(&self, time: u32, dx: f64, dy: f64) -> () {
            let msg = Event::Update {
                time: time,
                dx: dx,
                dy: dy,
            };
            self.0.send(msg);
        }
        #[doc = "multi-finger swipe end\n\nThis event is sent when a multi-finger swipe gesture ceases to\nbe valid. This may happen when one or more fingers are lifted or\nthe gesture is cancelled.\n\nWhen a gesture is cancelled, the client should undo state changes\ncaused by this gesture. What causes a gesture to be cancelled is\nimplementation-dependent."]
        pub fn end(&self, serial: u32, time: u32, cancelled: i32) -> () {
            let msg = Event::End {
                serial: serial,
                time: time,
                cancelled: cancelled,
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_BEGIN_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_UPDATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_END_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_pointer_gesture_swipe_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    static mut zwp_pointer_gesture_swipe_v1_events_begin_types: [*const wl_interface; 4] = [
        NULLPTR as *const wl_interface,
        NULLPTR as *const wl_interface,
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
        NULLPTR as *const wl_interface,
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_pointer_gesture_swipe_v1_events: [wl_message; 3] = [
        wl_message {
            name: b"begin\0" as *const u8 as *const c_char,
            signature: b"uuou\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_pointer_gesture_swipe_v1_events_begin_types as *const _ },
        },
        wl_message {
            name: b"update\0" as *const u8 as *const c_char,
            signature: b"uff\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"end\0" as *const u8 as *const c_char,
            signature: b"uui\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_pointer_gesture_swipe_v1_interface: wl_interface = wl_interface {
        name: b"zwp_pointer_gesture_swipe_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 1,
        requests: unsafe { &zwp_pointer_gesture_swipe_v1_requests as *const _ },
        event_count: 3,
        events: unsafe { &zwp_pointer_gesture_swipe_v1_events as *const _ },
    };
}
#[doc = "a pinch gesture object\n\nA pinch gesture object notifies a client about a multi-finger pinch\ngesture detected on an indirect input device such as a touchpad.\nThe gesture is usually initiated by multiple fingers moving towards\neach other or away from each other, or by two or more fingers rotating\naround a logical center of gravity. The precise conditions of when\nsuch a gesture is detected are implementation-dependent.\n\nA gesture consists of three stages: begin, update (optional) and end.\nThere cannot be multiple simultaneous pinch or swipe gestures on a\nsame pointer/seat, how compositors prevent these situations is\nimplementation-dependent.\n\nA gesture may be cancelled by the compositor or the hardware.\nClients should not consider performing permanent or irreversible\nactions until the end of a gesture has been received."]
pub mod zwp_pointer_gesture_pinch_v1 {
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
        #[doc = "destroy the pinch gesture object\n\n\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "multi-finger pinch begin\n\nThis event is sent when a multi-finger pinch gesture is detected\non the device."]
        Begin {
            serial: u32,
            time: u32,
            surface: super::wl_surface::WlSurface,
            fingers: u32,
        },
        #[doc = "multi-finger pinch motion\n\nThis event is sent when a multi-finger pinch gesture changes the\nposition of the logical center, the rotation or the relative scale.\n\nThe dx and dy coordinates are relative coordinates in the\nsurface coordinate space of the logical center of the gesture.\n\nThe scale factor is an absolute scale compared to the\npointer_gesture_pinch.begin event, e.g. a scale of 2 means the fingers\nare now twice as far apart as on pointer_gesture_pinch.begin.\n\nThe rotation is the relative angle in degrees clockwise compared to the previous\npointer_gesture_pinch.begin or pointer_gesture_pinch.update event."]
        Update {
            time: u32,
            dx: f64,
            dy: f64,
            scale: f64,
            rotation: f64,
        },
        #[doc = "multi-finger pinch end\n\nThis event is sent when a multi-finger pinch gesture ceases to\nbe valid. This may happen when one or more fingers are lifted or\nthe gesture is cancelled.\n\nWhen a gesture is cancelled, the client should undo state changes\ncaused by this gesture. What causes a gesture to be cancelled is\nimplementation-dependent."]
        End {
            serial: u32,
            time: u32,
            cancelled: i32,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "begin",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Object,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "update",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Fixed,
                    super::ArgumentType::Fixed,
                    super::ArgumentType::Fixed,
                    super::ArgumentType::Fixed,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "end",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Int,
                ],
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
                Event::Begin { .. } => 0,
                Event::Update { .. } => 1,
                Event::End { .. } => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Begin { .. } => 1,
                Event::Update { .. } => 1,
                Event::End { .. } => 1,
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
                Event::Begin {
                    serial,
                    time,
                    surface,
                    fingers,
                } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(time),
                        Argument::Object(surface.as_ref().id()),
                        Argument::Uint(fingers),
                    ],
                },
                Event::Update {
                    time,
                    dx,
                    dy,
                    scale,
                    rotation,
                } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![
                        Argument::Uint(time),
                        Argument::Fixed((dx * 256.) as i32),
                        Argument::Fixed((dy * 256.) as i32),
                        Argument::Fixed((scale * 256.) as i32),
                        Argument::Fixed((rotation * 256.) as i32),
                    ],
                },
                Event::End {
                    serial,
                    time,
                    cancelled,
                } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(time),
                        Argument::Int(cancelled),
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
                Event::Begin {
                    serial,
                    time,
                    surface,
                    fingers,
                } => {
                    let mut _args_array: [wl_argument; 4] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = time;
                    _args_array[2].o = surface.as_ref().c_ptr() as *mut _;
                    _args_array[3].u = fingers;
                    f(0, &mut _args_array)
                }
                Event::Update {
                    time,
                    dx,
                    dy,
                    scale,
                    rotation,
                } => {
                    let mut _args_array: [wl_argument; 5] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = time;
                    _args_array[1].f = (dx * 256.) as i32;
                    _args_array[2].f = (dy * 256.) as i32;
                    _args_array[3].f = (scale * 256.) as i32;
                    _args_array[4].f = (rotation * 256.) as i32;
                    f(1, &mut _args_array)
                }
                Event::End {
                    serial,
                    time,
                    cancelled,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = time;
                    _args_array[2].i = cancelled;
                    f(2, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpPointerGesturePinchV1(Resource<ZwpPointerGesturePinchV1>);
    impl AsRef<Resource<ZwpPointerGesturePinchV1>> for ZwpPointerGesturePinchV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpPointerGesturePinchV1>> for ZwpPointerGesturePinchV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpPointerGesturePinchV1(value)
        }
    }
    impl From<ZwpPointerGesturePinchV1> for Resource<ZwpPointerGesturePinchV1> {
        #[inline]
        fn from(value: ZwpPointerGesturePinchV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpPointerGesturePinchV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpPointerGesturePinchV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_pointer_gesture_pinch_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_pointer_gesture_pinch_v1_interface }
        }
    }
    impl ZwpPointerGesturePinchV1 {
        #[doc = "multi-finger pinch begin\n\nThis event is sent when a multi-finger pinch gesture is detected\non the device."]
        pub fn begin(
            &self,
            serial: u32,
            time: u32,
            surface: &super::wl_surface::WlSurface,
            fingers: u32,
        ) -> () {
            let msg = Event::Begin {
                serial: serial,
                time: time,
                surface: surface.clone(),
                fingers: fingers,
            };
            self.0.send(msg);
        }
        #[doc = "multi-finger pinch motion\n\nThis event is sent when a multi-finger pinch gesture changes the\nposition of the logical center, the rotation or the relative scale.\n\nThe dx and dy coordinates are relative coordinates in the\nsurface coordinate space of the logical center of the gesture.\n\nThe scale factor is an absolute scale compared to the\npointer_gesture_pinch.begin event, e.g. a scale of 2 means the fingers\nare now twice as far apart as on pointer_gesture_pinch.begin.\n\nThe rotation is the relative angle in degrees clockwise compared to the previous\npointer_gesture_pinch.begin or pointer_gesture_pinch.update event."]
        pub fn update(&self, time: u32, dx: f64, dy: f64, scale: f64, rotation: f64) -> () {
            let msg = Event::Update {
                time: time,
                dx: dx,
                dy: dy,
                scale: scale,
                rotation: rotation,
            };
            self.0.send(msg);
        }
        #[doc = "multi-finger pinch end\n\nThis event is sent when a multi-finger pinch gesture ceases to\nbe valid. This may happen when one or more fingers are lifted or\nthe gesture is cancelled.\n\nWhen a gesture is cancelled, the client should undo state changes\ncaused by this gesture. What causes a gesture to be cancelled is\nimplementation-dependent."]
        pub fn end(&self, serial: u32, time: u32, cancelled: i32) -> () {
            let msg = Event::End {
                serial: serial,
                time: time,
                cancelled: cancelled,
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_BEGIN_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_UPDATE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_END_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_pointer_gesture_pinch_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    static mut zwp_pointer_gesture_pinch_v1_events_begin_types: [*const wl_interface; 4] = [
        NULLPTR as *const wl_interface,
        NULLPTR as *const wl_interface,
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
        NULLPTR as *const wl_interface,
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_pointer_gesture_pinch_v1_events: [wl_message; 3] = [
        wl_message {
            name: b"begin\0" as *const u8 as *const c_char,
            signature: b"uuou\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_pointer_gesture_pinch_v1_events_begin_types as *const _ },
        },
        wl_message {
            name: b"update\0" as *const u8 as *const c_char,
            signature: b"uffff\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"end\0" as *const u8 as *const c_char,
            signature: b"uui\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_pointer_gesture_pinch_v1_interface: wl_interface = wl_interface {
        name: b"zwp_pointer_gesture_pinch_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 1,
        requests: unsafe { &zwp_pointer_gesture_pinch_v1_requests as *const _ },
        event_count: 3,
        events: unsafe { &zwp_pointer_gesture_pinch_v1_events as *const _ },
    };
}
