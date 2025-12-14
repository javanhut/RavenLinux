use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 5] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "virtual pointer\n\nThis protocol allows clients to emulate a physical pointer device. The\nrequests are mostly mirror opposites of those specified in wl_pointer."]
pub mod zwlr_virtual_pointer_v1 {
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
        #[doc = "client sent invalid axis enumeration value"]
        InvalidAxis = 0,
        #[doc = "client sent invalid axis source enumeration value"]
        InvalidAxisSource = 1,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::InvalidAxis),
                1 => Some(Error::InvalidAxisSource),
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
        #[doc = "pointer relative motion event\n\nThe pointer has moved by a relative amount to the previous request.\n\nValues are in the global compositor space."]
        Motion { time: u32, dx: f64, dy: f64 },
        #[doc = "pointer absolute motion event\n\nThe pointer has moved in an absolute coordinate frame.\n\nValue of x can range from 0 to x_extent, value of y can range from 0\nto y_extent."]
        MotionAbsolute {
            time: u32,
            x: u32,
            y: u32,
            x_extent: u32,
            y_extent: u32,
        },
        #[doc = "button event\n\nA button was pressed or released."]
        Button {
            time: u32,
            button: u32,
            state: super::wl_pointer::ButtonState,
        },
        #[doc = "axis event\n\nScroll and other axis requests."]
        Axis {
            time: u32,
            axis: super::wl_pointer::Axis,
            value: f64,
        },
        #[doc = "end of a pointer event sequence\n\nIndicates the set of events that logically belong together."]
        Frame,
        #[doc = "axis source event\n\nSource information for scroll and other axis."]
        AxisSource {
            axis_source: super::wl_pointer::AxisSource,
        },
        #[doc = "axis stop event\n\nStop notification for scroll and other axes."]
        AxisStop {
            time: u32,
            axis: super::wl_pointer::Axis,
        },
        #[doc = "axis click event\n\nDiscrete step information for scroll and other axes.\n\nThis event allows the client to extend data normally sent using the axis\nevent with discrete value."]
        AxisDiscrete {
            time: u32,
            axis: super::wl_pointer::Axis,
            value: f64,
            discrete: i32,
        },
        #[doc = "destroy the virtual pointer object\n\n\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "motion",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Fixed,
                    super::ArgumentType::Fixed,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "motion_absolute",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "button",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "axis",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Fixed,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "frame",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "axis_source",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "axis_stop",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "axis_discrete",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Uint,
                    super::ArgumentType::Fixed,
                    super::ArgumentType::Int,
                ],
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
                Request::Motion { .. } => 0,
                Request::MotionAbsolute { .. } => 1,
                Request::Button { .. } => 2,
                Request::Axis { .. } => 3,
                Request::Frame => 4,
                Request::AxisSource { .. } => 5,
                Request::AxisStop { .. } => 6,
                Request::AxisDiscrete { .. } => 7,
                Request::Destroy => 8,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::Motion { .. } => 1,
                Request::MotionAbsolute { .. } => 1,
                Request::Button { .. } => 1,
                Request::Axis { .. } => 1,
                Request::Frame => 1,
                Request::AxisSource { .. } => 1,
                Request::AxisStop { .. } => 1,
                Request::AxisDiscrete { .. } => 1,
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
                    Ok(Request::Motion {
                        time: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        dx: {
                            if let Some(Argument::Fixed(val)) = args.next() {
                                (val as f64) / 256.
                            } else {
                                return Err(());
                            }
                        },
                        dy: {
                            if let Some(Argument::Fixed(val)) = args.next() {
                                (val as f64) / 256.
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                1 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::MotionAbsolute {
                        time: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        x: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        y: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        x_extent: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        y_extent: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Button {
                        time: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        button: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        state: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                super::wl_pointer::ButtonState::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                3 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::Axis {
                        time: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        axis: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                super::wl_pointer::Axis::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        value: {
                            if let Some(Argument::Fixed(val)) = args.next() {
                                (val as f64) / 256.
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                4 => Ok(Request::Frame),
                5 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::AxisSource {
                        axis_source: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                super::wl_pointer::AxisSource::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                6 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::AxisStop {
                        time: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        axis: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                super::wl_pointer::Axis::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                7 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::AxisDiscrete {
                        time: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        axis: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                super::wl_pointer::Axis::from_raw(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        value: {
                            if let Some(Argument::Fixed(val)) = args.next() {
                                (val as f64) / 256.
                            } else {
                                return Err(());
                            }
                        },
                        discrete: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                8 => Ok(Request::Destroy),
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
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::Motion {
                        time: _args[0].u,
                        dx: (_args[1].f as f64) / 256.,
                        dy: (_args[2].f as f64) / 256.,
                    })
                }
                1 => {
                    let _args = ::std::slice::from_raw_parts(args, 5);
                    Ok(Request::MotionAbsolute {
                        time: _args[0].u,
                        x: _args[1].u,
                        y: _args[2].u,
                        x_extent: _args[3].u,
                        y_extent: _args[4].u,
                    })
                }
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::Button {
                        time: _args[0].u,
                        button: _args[1].u,
                        state: super::wl_pointer::ButtonState::from_raw(_args[2].u).ok_or(())?,
                    })
                }
                3 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::Axis {
                        time: _args[0].u,
                        axis: super::wl_pointer::Axis::from_raw(_args[1].u).ok_or(())?,
                        value: (_args[2].f as f64) / 256.,
                    })
                }
                4 => Ok(Request::Frame),
                5 => {
                    let _args = ::std::slice::from_raw_parts(args, 1);
                    Ok(Request::AxisSource {
                        axis_source: super::wl_pointer::AxisSource::from_raw(_args[0].u)
                            .ok_or(())?,
                    })
                }
                6 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::AxisStop {
                        time: _args[0].u,
                        axis: super::wl_pointer::Axis::from_raw(_args[1].u).ok_or(())?,
                    })
                }
                7 => {
                    let _args = ::std::slice::from_raw_parts(args, 4);
                    Ok(Request::AxisDiscrete {
                        time: _args[0].u,
                        axis: super::wl_pointer::Axis::from_raw(_args[1].u).ok_or(())?,
                        value: (_args[2].f as f64) / 256.,
                        discrete: _args[3].i,
                    })
                }
                8 => Ok(Request::Destroy),
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
    pub struct ZwlrVirtualPointerV1(Resource<ZwlrVirtualPointerV1>);
    impl AsRef<Resource<ZwlrVirtualPointerV1>> for ZwlrVirtualPointerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrVirtualPointerV1>> for ZwlrVirtualPointerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrVirtualPointerV1(value)
        }
    }
    impl From<ZwlrVirtualPointerV1> for Resource<ZwlrVirtualPointerV1> {
        #[inline]
        fn from(value: ZwlrVirtualPointerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrVirtualPointerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrVirtualPointerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_virtual_pointer_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_virtual_pointer_v1_interface }
        }
    }
    impl ZwlrVirtualPointerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_MOTION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_MOTION_ABSOLUTE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_BUTTON_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_AXIS_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_FRAME_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_AXIS_SOURCE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_AXIS_STOP_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_AXIS_DISCRETE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_virtual_pointer_v1_requests: [wl_message; 9] = [
        wl_message {
            name: b"motion\0" as *const u8 as *const c_char,
            signature: b"uff\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"motion_absolute\0" as *const u8 as *const c_char,
            signature: b"uuuuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"button\0" as *const u8 as *const c_char,
            signature: b"uuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"axis\0" as *const u8 as *const c_char,
            signature: b"uuf\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"frame\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"axis_source\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"axis_stop\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"axis_discrete\0" as *const u8 as *const c_char,
            signature: b"uufi\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_virtual_pointer_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_virtual_pointer_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 9,
        requests: unsafe { &zwlr_virtual_pointer_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "virtual pointer manager\n\nThis object allows clients to create individual virtual pointer objects."]
pub mod zwlr_virtual_pointer_manager_v1 {
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
        #[doc = "Create a new virtual pointer\n\nCreates a new virtual pointer. The optional seat is a suggestion to the\ncompositor."]
        CreateVirtualPointer {
            seat: Option<super::wl_seat::WlSeat>,
            id: Main<super::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1>,
        },
        #[doc = "destroy the virtual pointer manager\n\n\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
        #[doc = "Create a new virtual pointer\n\nCreates a new virtual pointer. The seat and the output arguments are\noptional. If the seat argument is set, the compositor should assign the\ninput device to the requested seat. If the output argument is set, the\ncompositor should map the input device to the requested output.\n\nOnly available since version 2 of the interface"]
        CreateVirtualPointerWithOutput {
            seat: Option<super::wl_seat::WlSeat>,
            output: Option<super::wl_output::WlOutput>,
            id: Main<super::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1>,
        },
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "create_virtual_pointer",
                since: 1,
                signature: &[super::ArgumentType::Object, super::ArgumentType::NewId],
                destructor: false,
            },
            super::MessageDesc {
                name: "destroy",
                since: 1,
                signature: &[],
                destructor: true,
            },
            super::MessageDesc {
                name: "create_virtual_pointer_with_output",
                since: 2,
                signature: &[
                    super::ArgumentType::Object,
                    super::ArgumentType::Object,
                    super::ArgumentType::NewId,
                ],
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
                Request::CreateVirtualPointer { .. } => 0,
                Request::Destroy => 1,
                Request::CreateVirtualPointerWithOutput { .. } => 2,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::CreateVirtualPointer { .. } => 1,
                Request::Destroy => 1,
                Request::CreateVirtualPointerWithOutput { .. } => 2,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<
                    super::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
                >(version, meta.child())),
                2 => Some(Object::from_interface::<
                    super::zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CreateVirtualPointer {
                        seat: {
                            if let Some(Argument::Object(val)) = args.next() {
                                if val == 0 {
                                    None
                                } else {
                                    Some(map.get(val).ok_or(())?.into())
                                }
                            } else {
                                return Err(());
                            }
                        },
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                    })
                }
                1 => Ok(Request::Destroy),
                2 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::CreateVirtualPointerWithOutput {
                        seat: {
                            if let Some(Argument::Object(val)) = args.next() {
                                if val == 0 {
                                    None
                                } else {
                                    Some(map.get(val).ok_or(())?.into())
                                }
                            } else {
                                return Err(());
                            }
                        },
                        output: {
                            if let Some(Argument::Object(val)) = args.next() {
                                if val == 0 {
                                    None
                                } else {
                                    Some(map.get(val).ok_or(())?.into())
                                }
                            } else {
                                return Err(());
                            }
                        },
                        id: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
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
                0 => {
                    let _args = ::std::slice::from_raw_parts(args, 2);
                    Ok(Request::CreateVirtualPointer {
                        seat: if _args[0].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_seat::WlSeat>::from_c_ptr(
                                    _args[0].o as *mut _,
                                )
                                .into(),
                            )
                        },
                        id: {
                            let me =
                                Resource::<ZwlrVirtualPointerManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwlr_virtual_pointer_v1 :: ZwlrVirtualPointerV1 > (_args [1] . n) . unwrap ()
                        },
                    })
                }
                1 => Ok(Request::Destroy),
                2 => {
                    let _args = ::std::slice::from_raw_parts(args, 3);
                    Ok(Request::CreateVirtualPointerWithOutput {
                        seat: if _args[0].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_seat::WlSeat>::from_c_ptr(
                                    _args[0].o as *mut _,
                                )
                                .into(),
                            )
                        },
                        output: if _args[1].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_output::WlOutput>::from_c_ptr(
                                    _args[1].o as *mut _,
                                )
                                .into(),
                            )
                        },
                        id: {
                            let me =
                                Resource::<ZwlrVirtualPointerManagerV1>::from_c_ptr(obj as *mut _);
                            me . make_child_for :: < super :: zwlr_virtual_pointer_v1 :: ZwlrVirtualPointerV1 > (_args [2] . n) . unwrap ()
                        },
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
    pub struct ZwlrVirtualPointerManagerV1(Resource<ZwlrVirtualPointerManagerV1>);
    impl AsRef<Resource<ZwlrVirtualPointerManagerV1>> for ZwlrVirtualPointerManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwlrVirtualPointerManagerV1>> for ZwlrVirtualPointerManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwlrVirtualPointerManagerV1(value)
        }
    }
    impl From<ZwlrVirtualPointerManagerV1> for Resource<ZwlrVirtualPointerManagerV1> {
        #[inline]
        fn from(value: ZwlrVirtualPointerManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwlrVirtualPointerManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwlrVirtualPointerManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwlr_virtual_pointer_manager_v1";
        const VERSION: u32 = 2;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwlr_virtual_pointer_manager_v1_interface }
        }
    }
    impl ZwlrVirtualPointerManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CREATE_VIRTUAL_POINTER_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_CREATE_VIRTUAL_POINTER_WITH_OUTPUT_SINCE: u32 = 2u32;
    static mut zwlr_virtual_pointer_manager_v1_requests_create_virtual_pointer_types:
        [*const wl_interface; 2] = [
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
        unsafe {
            &super::zwlr_virtual_pointer_v1::zwlr_virtual_pointer_v1_interface
                as *const wl_interface
        },
    ];
    static mut zwlr_virtual_pointer_manager_v1_requests_create_virtual_pointer_with_output_types:
        [*const wl_interface; 3] = [
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
        unsafe { &super::wl_output::wl_output_interface as *const wl_interface },
        unsafe {
            &super::zwlr_virtual_pointer_v1::zwlr_virtual_pointer_v1_interface
                as *const wl_interface
        },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwlr_virtual_pointer_manager_v1_requests: [wl_message; 3] = [
        wl_message {
            name: b"create_virtual_pointer\0" as *const u8 as *const c_char,
            signature: b"?on\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_virtual_pointer_manager_v1_requests_create_virtual_pointer_types as *const _
            },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"create_virtual_pointer_with_output\0" as *const u8 as *const c_char,
            signature: b"2?o?on\0" as *const u8 as *const c_char,
            types: unsafe {
                &zwlr_virtual_pointer_manager_v1_requests_create_virtual_pointer_with_output_types
                    as *const _
            },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwlr_virtual_pointer_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwlr_virtual_pointer_manager_v1\0" as *const u8 as *const c_char,
        version: 2,
        request_count: 3,
        requests: unsafe { &zwlr_virtual_pointer_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
