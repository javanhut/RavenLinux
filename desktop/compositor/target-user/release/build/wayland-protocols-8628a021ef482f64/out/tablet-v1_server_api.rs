use std::os::raw::{c_char, c_void};
const NULLPTR: *const c_void = 0 as *const c_void;
static mut types_null: [*const sys::common::wl_interface; 3] = [
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
    NULLPTR as *const sys::common::wl_interface,
];
#[doc = "controller object for graphic tablet devices\n\nAn object that provides access to the graphics tablets available on this\nsystem. All tablets are associated with a seat, to get access to the\nactual tablets, use wp_tablet_manager.get_tablet_seat."]
pub mod zwp_tablet_manager_v1 {
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
        #[doc = "get the tablet seat\n\nGet the wp_tablet_seat object for the given seat. This object\nprovides access to all graphics tablets in this seat."]
        GetTabletSeat {
            tablet_seat: Main<super::zwp_tablet_seat_v1::ZwpTabletSeatV1>,
            seat: super::wl_seat::WlSeat,
        },
        #[doc = "release the memory for the tablet manager object\n\nDestroy the wp_tablet_manager object. Objects created from this\nobject are unaffected and should be destroyed separately.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "get_tablet_seat",
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
                Request::GetTabletSeat { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::GetTabletSeat { .. } => 1,
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
                    super::zwp_tablet_seat_v1::ZwpTabletSeatV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            match msg.opcode {
                0 => {
                    let mut args = msg.args.into_iter();
                    Ok(Request::GetTabletSeat {
                        tablet_seat: {
                            if let Some(Argument::NewId(val)) = args.next() {
                                map.get_new(val).ok_or(())?
                            } else {
                                return Err(());
                            }
                        },
                        seat: {
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
                    Ok(Request::GetTabletSeat {
                        tablet_seat: {
                            let me = Resource::<ZwpTabletManagerV1>::from_c_ptr(obj as *mut _);
                            me.make_child_for::<super::zwp_tablet_seat_v1::ZwpTabletSeatV1>(
                                _args[0].n,
                            )
                            .unwrap()
                        },
                        seat: Resource::<super::wl_seat::WlSeat>::from_c_ptr(_args[1].o as *mut _)
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
    pub struct ZwpTabletManagerV1(Resource<ZwpTabletManagerV1>);
    impl AsRef<Resource<ZwpTabletManagerV1>> for ZwpTabletManagerV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpTabletManagerV1>> for ZwpTabletManagerV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpTabletManagerV1(value)
        }
    }
    impl From<ZwpTabletManagerV1> for Resource<ZwpTabletManagerV1> {
        #[inline]
        fn from(value: ZwpTabletManagerV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpTabletManagerV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpTabletManagerV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_tablet_manager_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_tablet_manager_v1_interface }
        }
    }
    impl ZwpTabletManagerV1 {}
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_GET_TABLET_SEAT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    static mut zwp_tablet_manager_v1_requests_get_tablet_seat_types: [*const wl_interface; 2] = [
        unsafe { &super::zwp_tablet_seat_v1::zwp_tablet_seat_v1_interface as *const wl_interface },
        unsafe { &super::wl_seat::wl_seat_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_tablet_manager_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"get_tablet_seat\0" as *const u8 as *const c_char,
            signature: b"no\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_tablet_manager_v1_requests_get_tablet_seat_types as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_tablet_manager_v1_interface: wl_interface = wl_interface {
        name: b"zwp_tablet_manager_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwp_tablet_manager_v1_requests as *const _ },
        event_count: 0,
        events: NULLPTR as *const wl_message,
    };
}
#[doc = "controller object for graphic tablet devices of a seat\n\nAn object that provides access to the graphics tablets available on this\nseat. After binding to this interface, the compositor sends a set of\nwp_tablet_seat.tablet_added and wp_tablet_seat.tool_added events."]
pub mod zwp_tablet_seat_v1 {
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
        #[doc = "release the memory for the tablet seat object\n\nDestroy the wp_tablet_seat object. Objects created from this\nobject are unaffected and should be destroyed separately.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "new device notification\n\nThis event is sent whenever a new tablet becomes available on this\nseat. This event only provides the object id of the tablet, any\nstatic information about the tablet (device name, vid/pid, etc.) is\nsent through the wp_tablet interface."]
        TabletAdded {
            id: Resource<super::zwp_tablet_v1::ZwpTabletV1>,
        },
        #[doc = "a new tool has been used with a tablet\n\nThis event is sent whenever a tool that has not previously been used\nwith a tablet comes into use. This event only provides the object id\nof the tool; any static information about the tool (capabilities,\ntype, etc.) is sent through the wp_tablet_tool interface."]
        ToolAdded {
            id: Resource<super::zwp_tablet_tool_v1::ZwpTabletToolV1>,
        },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "tablet_added",
                since: 1,
                signature: &[super::ArgumentType::NewId],
                destructor: false,
            },
            super::MessageDesc {
                name: "tool_added",
                since: 1,
                signature: &[super::ArgumentType::NewId],
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
                Event::TabletAdded { .. } => 0,
                Event::ToolAdded { .. } => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::TabletAdded { .. } => 1,
                Event::ToolAdded { .. } => 1,
            }
        }
        fn child<Meta: ObjectMetadata>(
            opcode: u16,
            version: u32,
            meta: &Meta,
        ) -> Option<Object<Meta>> {
            match opcode {
                0 => Some(Object::from_interface::<super::zwp_tablet_v1::ZwpTabletV1>(
                    version,
                    meta.child(),
                )),
                1 => Some(Object::from_interface::<
                    super::zwp_tablet_tool_v1::ZwpTabletToolV1,
                >(version, meta.child())),
                _ => None,
            }
        }
        fn from_raw(msg: Message, map: &mut Self::Map) -> Result<Self, ()> {
            panic!("Event::from_raw can not be used Server-side.")
        }
        fn into_raw(self, sender_id: u32) -> Message {
            match self {
                Event::TabletAdded { id } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::NewId(id.id()),],
                },
                Event::ToolAdded { id } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![Argument::NewId(id.id()),],
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
                Event::TabletAdded { id } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = id.c_ptr() as *mut _;
                    f(0, &mut _args_array)
                }
                Event::ToolAdded { id } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].o = id.c_ptr() as *mut _;
                    f(1, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpTabletSeatV1(Resource<ZwpTabletSeatV1>);
    impl AsRef<Resource<ZwpTabletSeatV1>> for ZwpTabletSeatV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpTabletSeatV1>> for ZwpTabletSeatV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpTabletSeatV1(value)
        }
    }
    impl From<ZwpTabletSeatV1> for Resource<ZwpTabletSeatV1> {
        #[inline]
        fn from(value: ZwpTabletSeatV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpTabletSeatV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpTabletSeatV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_tablet_seat_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_tablet_seat_v1_interface }
        }
    }
    impl ZwpTabletSeatV1 {
        #[doc = "new device notification\n\nThis event is sent whenever a new tablet becomes available on this\nseat. This event only provides the object id of the tablet, any\nstatic information about the tablet (device name, vid/pid, etc.) is\nsent through the wp_tablet interface."]
        pub fn tablet_added(&self, id: &super::zwp_tablet_v1::ZwpTabletV1) -> () {
            let msg = Event::TabletAdded {
                id: id.as_ref().clone(),
            };
            self.0.send(msg);
        }
        #[doc = "a new tool has been used with a tablet\n\nThis event is sent whenever a tool that has not previously been used\nwith a tablet comes into use. This event only provides the object id\nof the tool; any static information about the tool (capabilities,\ntype, etc.) is sent through the wp_tablet_tool interface."]
        pub fn tool_added(&self, id: &super::zwp_tablet_tool_v1::ZwpTabletToolV1) -> () {
            let msg = Event::ToolAdded {
                id: id.as_ref().clone(),
            };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_TABLET_ADDED_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_TOOL_ADDED_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_tablet_seat_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    static mut zwp_tablet_seat_v1_events_tablet_added_types: [*const wl_interface; 1] =
        [unsafe { &super::zwp_tablet_v1::zwp_tablet_v1_interface as *const wl_interface }];
    static mut zwp_tablet_seat_v1_events_tool_added_types: [*const wl_interface; 1] = [unsafe {
        &super::zwp_tablet_tool_v1::zwp_tablet_tool_v1_interface as *const wl_interface
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_tablet_seat_v1_events: [wl_message; 2] = [
        wl_message {
            name: b"tablet_added\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_tablet_seat_v1_events_tablet_added_types as *const _ },
        },
        wl_message {
            name: b"tool_added\0" as *const u8 as *const c_char,
            signature: b"n\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_tablet_seat_v1_events_tool_added_types as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_tablet_seat_v1_interface: wl_interface = wl_interface {
        name: b"zwp_tablet_seat_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_tablet_seat_v1_requests as *const _ },
        event_count: 2,
        events: unsafe { &zwp_tablet_seat_v1_events as *const _ },
    };
}
#[doc = "a physical tablet tool\n\nAn object that represents a physical tool that has been, or is\ncurrently in use with a tablet in this seat. Each wp_tablet_tool\nobject stays valid until the client destroys it; the compositor\nreuses the wp_tablet_tool object to indicate that the object's\nrespective physical tool has come into proximity of a tablet again.\n\nA wp_tablet_tool object's relation to a physical tool depends on the\ntablet's ability to report serial numbers. If the tablet supports\nthis capability, then the object represents a specific physical tool\nand can be identified even when used on multiple tablets.\n\nA tablet tool has a number of static characteristics, e.g. tool type,\nhardware_serial and capabilities. These capabilities are sent in an\nevent sequence after the wp_tablet_seat.tool_added event before any\nactual events from this tool. This initial event sequence is\nterminated by a wp_tablet_tool.done event.\n\nTablet tool events are grouped by wp_tablet_tool.frame events.\nAny events received before a wp_tablet_tool.frame event should be\nconsidered part of the same hardware state change."]
pub mod zwp_tablet_tool_v1 {
    use super::sys::common::{wl_argument, wl_array, wl_interface, wl_message};
    use super::sys::server::*;
    use super::{
        smallvec, types_null, AnonymousObject, Argument, ArgumentType, Interface, Main, Message,
        MessageDesc, MessageGroup, Object, ObjectMetadata, Resource, NULLPTR,
    };
    use std::os::raw::c_char;
    #[doc = "a physical tool type\n\nDescribes the physical type of a tool. The physical type of a tool\ngenerally defines its base usage.\n\nThe mouse tool represents a mouse-shaped tool that is not a relative\ndevice but bound to the tablet's surface, providing absolute\ncoordinates.\n\nThe lens tool is a mouse-shaped tool with an attached lens to\nprovide precision focus."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Type {
        #[doc = "Pen"]
        Pen = 320,
        #[doc = "Eraser"]
        Eraser = 321,
        #[doc = "Brush"]
        Brush = 322,
        #[doc = "Pencil"]
        Pencil = 323,
        #[doc = "Airbrush"]
        Airbrush = 324,
        #[doc = "Finger"]
        Finger = 325,
        #[doc = "Mouse"]
        Mouse = 326,
        #[doc = "Lens"]
        Lens = 327,
    }
    impl Type {
        pub fn from_raw(n: u32) -> Option<Type> {
            match n {
                320 => Some(Type::Pen),
                321 => Some(Type::Eraser),
                322 => Some(Type::Brush),
                323 => Some(Type::Pencil),
                324 => Some(Type::Airbrush),
                325 => Some(Type::Finger),
                326 => Some(Type::Mouse),
                327 => Some(Type::Lens),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[doc = "capability flags for a tool\n\nDescribes extra capabilities on a tablet.\n\nAny tool must provide x and y values, extra axes are\ndevice-specific."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum Capability {
        #[doc = "Tilt axes"]
        Tilt = 1,
        #[doc = "Pressure axis"]
        Pressure = 2,
        #[doc = "Distance axis"]
        Distance = 3,
        #[doc = "Z-rotation axis"]
        Rotation = 4,
        #[doc = "Slider axis"]
        Slider = 5,
        #[doc = "Wheel axis"]
        Wheel = 6,
    }
    impl Capability {
        pub fn from_raw(n: u32) -> Option<Capability> {
            match n {
                1 => Some(Capability::Tilt),
                2 => Some(Capability::Pressure),
                3 => Some(Capability::Distance),
                4 => Some(Capability::Rotation),
                5 => Some(Capability::Slider),
                6 => Some(Capability::Wheel),
                _ => Option::None,
            }
        }
        pub fn to_raw(&self) -> u32 {
            *self as u32
        }
    }
    #[doc = "physical button state\n\nDescribes the physical state of a button that produced the button event."]
    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    #[non_exhaustive]
    pub enum ButtonState {
        #[doc = "button is not pressed"]
        Released = 0,
        #[doc = "button is pressed"]
        Pressed = 1,
    }
    impl ButtonState {
        pub fn from_raw(n: u32) -> Option<ButtonState> {
            match n {
                0 => Some(ButtonState::Released),
                1 => Some(ButtonState::Pressed),
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
        #[doc = "given wl_surface has another role"]
        Role = 0,
    }
    impl Error {
        pub fn from_raw(n: u32) -> Option<Error> {
            match n {
                0 => Some(Error::Role),
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
        #[doc = "set the tablet tool's surface\n\nSets the surface of the cursor used for this tool on the given\ntablet. This request only takes effect if the tool is in proximity\nof one of the requesting client's surfaces or the surface parameter\nis the current pointer surface. If there was a previous surface set\nwith this request it is replaced. If surface is NULL, the cursor\nimage is hidden.\n\nThe parameters hotspot_x and hotspot_y define the position of the\npointer surface relative to the pointer location. Its top-left corner\nis always at (x, y) - (hotspot_x, hotspot_y), where (x, y) are the\ncoordinates of the pointer location, in surface-local coordinates.\n\nOn surface.attach requests to the pointer surface, hotspot_x and\nhotspot_y are decremented by the x and y parameters passed to the\nrequest. Attach must be confirmed by wl_surface.commit as usual.\n\nThe hotspot can also be updated by passing the currently set pointer\nsurface to this request with new values for hotspot_x and hotspot_y.\n\nThe current and pending input regions of the wl_surface are cleared,\nand wl_surface.set_input_region is ignored until the wl_surface is no\nlonger used as the cursor. When the use as a cursor ends, the current\nand pending input regions become undefined, and the wl_surface is\nunmapped.\n\nThis request gives the surface the role of a cursor. The role\nassigned by this request is the same as assigned by\nwl_pointer.set_cursor meaning the same surface can be\nused both as a wl_pointer cursor and a wp_tablet cursor. If the\nsurface already has another role, it raises a protocol error.\nThe surface may be used on multiple tablets and across multiple\nseats."]
        SetCursor {
            serial: u32,
            surface: Option<super::wl_surface::WlSurface>,
            hotspot_x: i32,
            hotspot_y: i32,
        },
        #[doc = "destroy the tool object\n\nThis destroys the client's resource for this tool object.\n\nThis is a destructor, once received this object cannot be used any longer."]
        Destroy,
    }
    impl super::MessageGroup for Request {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "set_cursor",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Object,
                    super::ArgumentType::Int,
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
                Request::SetCursor { .. } => 0,
                Request::Destroy => 1,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Request::SetCursor { .. } => 1,
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
                    Ok(Request::SetCursor {
                        serial: {
                            if let Some(Argument::Uint(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        surface: {
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
                        hotspot_x: {
                            if let Some(Argument::Int(val)) = args.next() {
                                val
                            } else {
                                return Err(());
                            }
                        },
                        hotspot_y: {
                            if let Some(Argument::Int(val)) = args.next() {
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
                    let _args = ::std::slice::from_raw_parts(args, 4);
                    Ok(Request::SetCursor {
                        serial: _args[0].u,
                        surface: if _args[1].o.is_null() {
                            None
                        } else {
                            Some(
                                Resource::<super::wl_surface::WlSurface>::from_c_ptr(
                                    _args[1].o as *mut _,
                                )
                                .into(),
                            )
                        },
                        hotspot_x: _args[2].i,
                        hotspot_y: _args[3].i,
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
        #[doc = "tool type\n\nThe tool type is the high-level type of the tool and usually decides\nthe interaction expected from this tool.\n\nThis event is sent in the initial burst of events before the\nwp_tablet_tool.done event."]
        Type { tool_type: Type },
        #[doc = "unique hardware serial number of the tool\n\nIf the physical tool can be identified by a unique 64-bit serial\nnumber, this event notifies the client of this serial number.\n\nIf multiple tablets are available in the same seat and the tool is\nuniquely identifiable by the serial number, that tool may move\nbetween tablets.\n\nOtherwise, if the tool has no serial number and this event is\nmissing, the tool is tied to the tablet it first comes into\nproximity with. Even if the physical tool is used on multiple\ntablets, separate wp_tablet_tool objects will be created, one per\ntablet.\n\nThis event is sent in the initial burst of events before the\nwp_tablet_tool.done event."]
        HardwareSerial {
            hardware_serial_hi: u32,
            hardware_serial_lo: u32,
        },
        #[doc = "hardware id notification in Wacom's format\n\nThis event notifies the client of a hardware id available on this tool.\n\nThe hardware id is a device-specific 64-bit id that provides extra\ninformation about the tool in use, beyond the wl_tool.type\nenumeration. The format of the id is specific to tablets made by\nWacom Inc. For example, the hardware id of a Wacom Grip\nPen (a stylus) is 0x802.\n\nThis event is sent in the initial burst of events before the\nwp_tablet_tool.done event."]
        HardwareIdWacom {
            hardware_id_hi: u32,
            hardware_id_lo: u32,
        },
        #[doc = "tool capability notification\n\nThis event notifies the client of any capabilities of this tool,\nbeyond the main set of x/y axes and tip up/down detection.\n\nOne event is sent for each extra capability available on this tool.\n\nThis event is sent in the initial burst of events before the\nwp_tablet_tool.done event."]
        Capability { capability: Capability },
        #[doc = "tool description events sequence complete\n\nThis event signals the end of the initial burst of descriptive\nevents. A client may consider the static description of the tool to\nbe complete and finalize initialization of the tool."]
        Done,
        #[doc = "tool removed\n\nThis event is sent when the tool is removed from the system and will\nsend no further events. Should the physical tool come back into\nproximity later, a new wp_tablet_tool object will be created.\n\nIt is compositor-dependent when a tool is removed. A compositor may\nremove a tool on proximity out, tablet removal or any other reason.\nA compositor may also keep a tool alive until shutdown.\n\nIf the tool is currently in proximity, a proximity_out event will be\nsent before the removed event. See wp_tablet_tool.proximity_out for\nthe handling of any buttons logically down.\n\nWhen this event is received, the client must wp_tablet_tool.destroy\nthe object."]
        Removed,
        #[doc = "proximity in event\n\nNotification that this tool is focused on a certain surface.\n\nThis event can be received when the tool has moved from one surface to\nanother, or when the tool has come back into proximity above the\nsurface.\n\nIf any button is logically down when the tool comes into proximity,\nthe respective button event is sent after the proximity_in event but\nwithin the same frame as the proximity_in event."]
        ProximityIn {
            serial: u32,
            tablet: super::zwp_tablet_v1::ZwpTabletV1,
            surface: super::wl_surface::WlSurface,
        },
        #[doc = "proximity out event\n\nNotification that this tool has either left proximity, or is no\nlonger focused on a certain surface.\n\nWhen the tablet tool leaves proximity of the tablet, button release\nevents are sent for each button that was held down at the time of\nleaving proximity. These events are sent before the proximity_out\nevent but within the same wp_tablet.frame.\n\nIf the tool stays within proximity of the tablet, but the focus\nchanges from one surface to another, a button release event may not\nbe sent until the button is actually released or the tool leaves the\nproximity of the tablet."]
        ProximityOut,
        #[doc = "tablet tool is making contact\n\nSent whenever the tablet tool comes in contact with the surface of the\ntablet.\n\nIf the tool is already in contact with the tablet when entering the\ninput region, the client owning said region will receive a\nwp_tablet.proximity_in event, followed by a wp_tablet.down\nevent and a wp_tablet.frame event.\n\nNote that this event describes logical contact, not physical\ncontact. On some devices, a compositor may not consider a tool in\nlogical contact until a minimum physical pressure threshold is\nexceeded."]
        Down { serial: u32 },
        #[doc = "tablet tool is no longer making contact\n\nSent whenever the tablet tool stops making contact with the surface of\nthe tablet, or when the tablet tool moves out of the input region\nand the compositor grab (if any) is dismissed.\n\nIf the tablet tool moves out of the input region while in contact\nwith the surface of the tablet and the compositor does not have an\nongoing grab on the surface, the client owning said region will\nreceive a wp_tablet.up event, followed by a wp_tablet.proximity_out\nevent and a wp_tablet.frame event. If the compositor has an ongoing\ngrab on this device, this event sequence is sent whenever the grab\nis dismissed in the future.\n\nNote that this event describes logical contact, not physical\ncontact. On some devices, a compositor may not consider a tool out\nof logical contact until physical pressure falls below a specific\nthreshold."]
        Up,
        #[doc = "motion event\n\nSent whenever a tablet tool moves."]
        Motion { x: f64, y: f64 },
        #[doc = "pressure change event\n\nSent whenever the pressure axis on a tool changes. The value of this\nevent is normalized to a value between 0 and 65535.\n\nNote that pressure may be nonzero even when a tool is not in logical\ncontact. See the down and up events for more details."]
        Pressure { pressure: u32 },
        #[doc = "distance change event\n\nSent whenever the distance axis on a tool changes. The value of this\nevent is normalized to a value between 0 and 65535.\n\nNote that distance may be nonzero even when a tool is not in logical\ncontact. See the down and up events for more details."]
        Distance { distance: u32 },
        #[doc = "tilt change event\n\nSent whenever one or both of the tilt axes on a tool change. Each tilt\nvalue is in 0.01 of a degree, relative to the z-axis of the tablet.\nThe angle is positive when the top of a tool tilts along the\npositive x or y axis."]
        Tilt { tilt_x: i32, tilt_y: i32 },
        #[doc = "z-rotation change event\n\nSent whenever the z-rotation axis on the tool changes. The\nrotation value is in 0.01 of a degree clockwise from the tool's\nlogical neutral position."]
        Rotation { degrees: i32 },
        #[doc = "Slider position change event\n\nSent whenever the slider position on the tool changes. The\nvalue is normalized between -65535 and 65535, with 0 as the logical\nneutral position of the slider.\n\nThe slider is available on e.g. the Wacom Airbrush tool."]
        Slider { position: i32 },
        #[doc = "Wheel delta event\n\nSent whenever the wheel on the tool emits an event. This event\ncontains two values for the same axis change. The degrees value is\nin 0.01 of a degree in the same orientation as the\nwl_pointer.vertical_scroll axis. The clicks value is in discrete\nlogical clicks of the mouse wheel. This value may be zero if the\nmovement of the wheel was less than one logical click.\n\nClients should choose either value and avoid mixing degrees and\nclicks. The compositor may accumulate values smaller than a logical\nclick and emulate click events when a certain threshold is met.\nThus, wl_tablet_tool.wheel events with non-zero clicks values may\nhave different degrees values."]
        Wheel { degrees: i32, clicks: i32 },
        #[doc = "button event\n\nSent whenever a button on the tool is pressed or released.\n\nIf a button is held down when the tool moves in or out of proximity,\nbutton events are generated by the compositor. See\nwp_tablet_tool.proximity_in and wp_tablet_tool.proximity_out for\ndetails."]
        Button {
            serial: u32,
            button: u32,
            state: ButtonState,
        },
        #[doc = "frame event\n\nMarks the end of a series of axis and/or button updates from the\ntablet. The Wayland protocol requires axis updates to be sent\nsequentially, however all events within a frame should be considered\none hardware event."]
        Frame { time: u32 },
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "type",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "hardware_serial",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "hardware_id_wacom",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "capability",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "done",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "removed",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "proximity_in",
                since: 1,
                signature: &[
                    super::ArgumentType::Uint,
                    super::ArgumentType::Object,
                    super::ArgumentType::Object,
                ],
                destructor: false,
            },
            super::MessageDesc {
                name: "proximity_out",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "down",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "up",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "motion",
                since: 1,
                signature: &[super::ArgumentType::Fixed, super::ArgumentType::Fixed],
                destructor: false,
            },
            super::MessageDesc {
                name: "pressure",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "distance",
                since: 1,
                signature: &[super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "tilt",
                since: 1,
                signature: &[super::ArgumentType::Int, super::ArgumentType::Int],
                destructor: false,
            },
            super::MessageDesc {
                name: "rotation",
                since: 1,
                signature: &[super::ArgumentType::Int],
                destructor: false,
            },
            super::MessageDesc {
                name: "slider",
                since: 1,
                signature: &[super::ArgumentType::Int],
                destructor: false,
            },
            super::MessageDesc {
                name: "wheel",
                since: 1,
                signature: &[super::ArgumentType::Int, super::ArgumentType::Int],
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
                name: "frame",
                since: 1,
                signature: &[super::ArgumentType::Uint],
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
                Event::Type { .. } => 0,
                Event::HardwareSerial { .. } => 1,
                Event::HardwareIdWacom { .. } => 2,
                Event::Capability { .. } => 3,
                Event::Done => 4,
                Event::Removed => 5,
                Event::ProximityIn { .. } => 6,
                Event::ProximityOut => 7,
                Event::Down { .. } => 8,
                Event::Up => 9,
                Event::Motion { .. } => 10,
                Event::Pressure { .. } => 11,
                Event::Distance { .. } => 12,
                Event::Tilt { .. } => 13,
                Event::Rotation { .. } => 14,
                Event::Slider { .. } => 15,
                Event::Wheel { .. } => 16,
                Event::Button { .. } => 17,
                Event::Frame { .. } => 18,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Type { .. } => 1,
                Event::HardwareSerial { .. } => 1,
                Event::HardwareIdWacom { .. } => 1,
                Event::Capability { .. } => 1,
                Event::Done => 1,
                Event::Removed => 1,
                Event::ProximityIn { .. } => 1,
                Event::ProximityOut => 1,
                Event::Down { .. } => 1,
                Event::Up => 1,
                Event::Motion { .. } => 1,
                Event::Pressure { .. } => 1,
                Event::Distance { .. } => 1,
                Event::Tilt { .. } => 1,
                Event::Rotation { .. } => 1,
                Event::Slider { .. } => 1,
                Event::Wheel { .. } => 1,
                Event::Button { .. } => 1,
                Event::Frame { .. } => 1,
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
                Event::Type { tool_type } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Uint(tool_type.to_raw()),],
                },
                Event::HardwareSerial {
                    hardware_serial_hi,
                    hardware_serial_lo,
                } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![
                        Argument::Uint(hardware_serial_hi),
                        Argument::Uint(hardware_serial_lo),
                    ],
                },
                Event::HardwareIdWacom {
                    hardware_id_hi,
                    hardware_id_lo,
                } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![
                        Argument::Uint(hardware_id_hi),
                        Argument::Uint(hardware_id_lo),
                    ],
                },
                Event::Capability { capability } => Message {
                    sender_id: sender_id,
                    opcode: 3,
                    args: smallvec![Argument::Uint(capability.to_raw()),],
                },
                Event::Done => Message {
                    sender_id: sender_id,
                    opcode: 4,
                    args: smallvec![],
                },
                Event::Removed => Message {
                    sender_id: sender_id,
                    opcode: 5,
                    args: smallvec![],
                },
                Event::ProximityIn {
                    serial,
                    tablet,
                    surface,
                } => Message {
                    sender_id: sender_id,
                    opcode: 6,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Object(tablet.as_ref().id()),
                        Argument::Object(surface.as_ref().id()),
                    ],
                },
                Event::ProximityOut => Message {
                    sender_id: sender_id,
                    opcode: 7,
                    args: smallvec![],
                },
                Event::Down { serial } => Message {
                    sender_id: sender_id,
                    opcode: 8,
                    args: smallvec![Argument::Uint(serial),],
                },
                Event::Up => Message {
                    sender_id: sender_id,
                    opcode: 9,
                    args: smallvec![],
                },
                Event::Motion { x, y } => Message {
                    sender_id: sender_id,
                    opcode: 10,
                    args: smallvec![
                        Argument::Fixed((x * 256.) as i32),
                        Argument::Fixed((y * 256.) as i32),
                    ],
                },
                Event::Pressure { pressure } => Message {
                    sender_id: sender_id,
                    opcode: 11,
                    args: smallvec![Argument::Uint(pressure),],
                },
                Event::Distance { distance } => Message {
                    sender_id: sender_id,
                    opcode: 12,
                    args: smallvec![Argument::Uint(distance),],
                },
                Event::Tilt { tilt_x, tilt_y } => Message {
                    sender_id: sender_id,
                    opcode: 13,
                    args: smallvec![Argument::Int(tilt_x), Argument::Int(tilt_y),],
                },
                Event::Rotation { degrees } => Message {
                    sender_id: sender_id,
                    opcode: 14,
                    args: smallvec![Argument::Int(degrees),],
                },
                Event::Slider { position } => Message {
                    sender_id: sender_id,
                    opcode: 15,
                    args: smallvec![Argument::Int(position),],
                },
                Event::Wheel { degrees, clicks } => Message {
                    sender_id: sender_id,
                    opcode: 16,
                    args: smallvec![Argument::Int(degrees), Argument::Int(clicks),],
                },
                Event::Button {
                    serial,
                    button,
                    state,
                } => Message {
                    sender_id: sender_id,
                    opcode: 17,
                    args: smallvec![
                        Argument::Uint(serial),
                        Argument::Uint(button),
                        Argument::Uint(state.to_raw()),
                    ],
                },
                Event::Frame { time } => Message {
                    sender_id: sender_id,
                    opcode: 18,
                    args: smallvec![Argument::Uint(time),],
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
                Event::Type { tool_type } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = tool_type.to_raw();
                    f(0, &mut _args_array)
                }
                Event::HardwareSerial {
                    hardware_serial_hi,
                    hardware_serial_lo,
                } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = hardware_serial_hi;
                    _args_array[1].u = hardware_serial_lo;
                    f(1, &mut _args_array)
                }
                Event::HardwareIdWacom {
                    hardware_id_hi,
                    hardware_id_lo,
                } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = hardware_id_hi;
                    _args_array[1].u = hardware_id_lo;
                    f(2, &mut _args_array)
                }
                Event::Capability { capability } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = capability.to_raw();
                    f(3, &mut _args_array)
                }
                Event::Done => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(4, &mut _args_array)
                }
                Event::Removed => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(5, &mut _args_array)
                }
                Event::ProximityIn {
                    serial,
                    tablet,
                    surface,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].o = tablet.as_ref().c_ptr() as *mut _;
                    _args_array[2].o = surface.as_ref().c_ptr() as *mut _;
                    f(6, &mut _args_array)
                }
                Event::ProximityOut => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(7, &mut _args_array)
                }
                Event::Down { serial } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    f(8, &mut _args_array)
                }
                Event::Up => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(9, &mut _args_array)
                }
                Event::Motion { x, y } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].f = (x * 256.) as i32;
                    _args_array[1].f = (y * 256.) as i32;
                    f(10, &mut _args_array)
                }
                Event::Pressure { pressure } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = pressure;
                    f(11, &mut _args_array)
                }
                Event::Distance { distance } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = distance;
                    f(12, &mut _args_array)
                }
                Event::Tilt { tilt_x, tilt_y } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = tilt_x;
                    _args_array[1].i = tilt_y;
                    f(13, &mut _args_array)
                }
                Event::Rotation { degrees } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = degrees;
                    f(14, &mut _args_array)
                }
                Event::Slider { position } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = position;
                    f(15, &mut _args_array)
                }
                Event::Wheel { degrees, clicks } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].i = degrees;
                    _args_array[1].i = clicks;
                    f(16, &mut _args_array)
                }
                Event::Button {
                    serial,
                    button,
                    state,
                } => {
                    let mut _args_array: [wl_argument; 3] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = serial;
                    _args_array[1].u = button;
                    _args_array[2].u = state.to_raw();
                    f(17, &mut _args_array)
                }
                Event::Frame { time } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = time;
                    f(18, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpTabletToolV1(Resource<ZwpTabletToolV1>);
    impl AsRef<Resource<ZwpTabletToolV1>> for ZwpTabletToolV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpTabletToolV1>> for ZwpTabletToolV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpTabletToolV1(value)
        }
    }
    impl From<ZwpTabletToolV1> for Resource<ZwpTabletToolV1> {
        #[inline]
        fn from(value: ZwpTabletToolV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpTabletToolV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpTabletToolV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_tablet_tool_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_tablet_tool_v1_interface }
        }
    }
    impl ZwpTabletToolV1 {
        #[doc = "tool type\n\nThe tool type is the high-level type of the tool and usually decides\nthe interaction expected from this tool.\n\nThis event is sent in the initial burst of events before the\nwp_tablet_tool.done event."]
        pub fn _type(&self, tool_type: Type) -> () {
            let msg = Event::Type {
                tool_type: tool_type,
            };
            self.0.send(msg);
        }
        #[doc = "unique hardware serial number of the tool\n\nIf the physical tool can be identified by a unique 64-bit serial\nnumber, this event notifies the client of this serial number.\n\nIf multiple tablets are available in the same seat and the tool is\nuniquely identifiable by the serial number, that tool may move\nbetween tablets.\n\nOtherwise, if the tool has no serial number and this event is\nmissing, the tool is tied to the tablet it first comes into\nproximity with. Even if the physical tool is used on multiple\ntablets, separate wp_tablet_tool objects will be created, one per\ntablet.\n\nThis event is sent in the initial burst of events before the\nwp_tablet_tool.done event."]
        pub fn hardware_serial(&self, hardware_serial_hi: u32, hardware_serial_lo: u32) -> () {
            let msg = Event::HardwareSerial {
                hardware_serial_hi: hardware_serial_hi,
                hardware_serial_lo: hardware_serial_lo,
            };
            self.0.send(msg);
        }
        #[doc = "hardware id notification in Wacom's format\n\nThis event notifies the client of a hardware id available on this tool.\n\nThe hardware id is a device-specific 64-bit id that provides extra\ninformation about the tool in use, beyond the wl_tool.type\nenumeration. The format of the id is specific to tablets made by\nWacom Inc. For example, the hardware id of a Wacom Grip\nPen (a stylus) is 0x802.\n\nThis event is sent in the initial burst of events before the\nwp_tablet_tool.done event."]
        pub fn hardware_id_wacom(&self, hardware_id_hi: u32, hardware_id_lo: u32) -> () {
            let msg = Event::HardwareIdWacom {
                hardware_id_hi: hardware_id_hi,
                hardware_id_lo: hardware_id_lo,
            };
            self.0.send(msg);
        }
        #[doc = "tool capability notification\n\nThis event notifies the client of any capabilities of this tool,\nbeyond the main set of x/y axes and tip up/down detection.\n\nOne event is sent for each extra capability available on this tool.\n\nThis event is sent in the initial burst of events before the\nwp_tablet_tool.done event."]
        pub fn capability(&self, capability: Capability) -> () {
            let msg = Event::Capability {
                capability: capability,
            };
            self.0.send(msg);
        }
        #[doc = "tool description events sequence complete\n\nThis event signals the end of the initial burst of descriptive\nevents. A client may consider the static description of the tool to\nbe complete and finalize initialization of the tool."]
        pub fn done(&self) -> () {
            let msg = Event::Done;
            self.0.send(msg);
        }
        #[doc = "tool removed\n\nThis event is sent when the tool is removed from the system and will\nsend no further events. Should the physical tool come back into\nproximity later, a new wp_tablet_tool object will be created.\n\nIt is compositor-dependent when a tool is removed. A compositor may\nremove a tool on proximity out, tablet removal or any other reason.\nA compositor may also keep a tool alive until shutdown.\n\nIf the tool is currently in proximity, a proximity_out event will be\nsent before the removed event. See wp_tablet_tool.proximity_out for\nthe handling of any buttons logically down.\n\nWhen this event is received, the client must wp_tablet_tool.destroy\nthe object."]
        pub fn removed(&self) -> () {
            let msg = Event::Removed;
            self.0.send(msg);
        }
        #[doc = "proximity in event\n\nNotification that this tool is focused on a certain surface.\n\nThis event can be received when the tool has moved from one surface to\nanother, or when the tool has come back into proximity above the\nsurface.\n\nIf any button is logically down when the tool comes into proximity,\nthe respective button event is sent after the proximity_in event but\nwithin the same frame as the proximity_in event."]
        pub fn proximity_in(
            &self,
            serial: u32,
            tablet: &super::zwp_tablet_v1::ZwpTabletV1,
            surface: &super::wl_surface::WlSurface,
        ) -> () {
            let msg = Event::ProximityIn {
                serial: serial,
                tablet: tablet.clone(),
                surface: surface.clone(),
            };
            self.0.send(msg);
        }
        #[doc = "proximity out event\n\nNotification that this tool has either left proximity, or is no\nlonger focused on a certain surface.\n\nWhen the tablet tool leaves proximity of the tablet, button release\nevents are sent for each button that was held down at the time of\nleaving proximity. These events are sent before the proximity_out\nevent but within the same wp_tablet.frame.\n\nIf the tool stays within proximity of the tablet, but the focus\nchanges from one surface to another, a button release event may not\nbe sent until the button is actually released or the tool leaves the\nproximity of the tablet."]
        pub fn proximity_out(&self) -> () {
            let msg = Event::ProximityOut;
            self.0.send(msg);
        }
        #[doc = "tablet tool is making contact\n\nSent whenever the tablet tool comes in contact with the surface of the\ntablet.\n\nIf the tool is already in contact with the tablet when entering the\ninput region, the client owning said region will receive a\nwp_tablet.proximity_in event, followed by a wp_tablet.down\nevent and a wp_tablet.frame event.\n\nNote that this event describes logical contact, not physical\ncontact. On some devices, a compositor may not consider a tool in\nlogical contact until a minimum physical pressure threshold is\nexceeded."]
        pub fn down(&self, serial: u32) -> () {
            let msg = Event::Down { serial: serial };
            self.0.send(msg);
        }
        #[doc = "tablet tool is no longer making contact\n\nSent whenever the tablet tool stops making contact with the surface of\nthe tablet, or when the tablet tool moves out of the input region\nand the compositor grab (if any) is dismissed.\n\nIf the tablet tool moves out of the input region while in contact\nwith the surface of the tablet and the compositor does not have an\nongoing grab on the surface, the client owning said region will\nreceive a wp_tablet.up event, followed by a wp_tablet.proximity_out\nevent and a wp_tablet.frame event. If the compositor has an ongoing\ngrab on this device, this event sequence is sent whenever the grab\nis dismissed in the future.\n\nNote that this event describes logical contact, not physical\ncontact. On some devices, a compositor may not consider a tool out\nof logical contact until physical pressure falls below a specific\nthreshold."]
        pub fn up(&self) -> () {
            let msg = Event::Up;
            self.0.send(msg);
        }
        #[doc = "motion event\n\nSent whenever a tablet tool moves."]
        pub fn motion(&self, x: f64, y: f64) -> () {
            let msg = Event::Motion { x: x, y: y };
            self.0.send(msg);
        }
        #[doc = "pressure change event\n\nSent whenever the pressure axis on a tool changes. The value of this\nevent is normalized to a value between 0 and 65535.\n\nNote that pressure may be nonzero even when a tool is not in logical\ncontact. See the down and up events for more details."]
        pub fn pressure(&self, pressure: u32) -> () {
            let msg = Event::Pressure { pressure: pressure };
            self.0.send(msg);
        }
        #[doc = "distance change event\n\nSent whenever the distance axis on a tool changes. The value of this\nevent is normalized to a value between 0 and 65535.\n\nNote that distance may be nonzero even when a tool is not in logical\ncontact. See the down and up events for more details."]
        pub fn distance(&self, distance: u32) -> () {
            let msg = Event::Distance { distance: distance };
            self.0.send(msg);
        }
        #[doc = "tilt change event\n\nSent whenever one or both of the tilt axes on a tool change. Each tilt\nvalue is in 0.01 of a degree, relative to the z-axis of the tablet.\nThe angle is positive when the top of a tool tilts along the\npositive x or y axis."]
        pub fn tilt(&self, tilt_x: i32, tilt_y: i32) -> () {
            let msg = Event::Tilt {
                tilt_x: tilt_x,
                tilt_y: tilt_y,
            };
            self.0.send(msg);
        }
        #[doc = "z-rotation change event\n\nSent whenever the z-rotation axis on the tool changes. The\nrotation value is in 0.01 of a degree clockwise from the tool's\nlogical neutral position."]
        pub fn rotation(&self, degrees: i32) -> () {
            let msg = Event::Rotation { degrees: degrees };
            self.0.send(msg);
        }
        #[doc = "Slider position change event\n\nSent whenever the slider position on the tool changes. The\nvalue is normalized between -65535 and 65535, with 0 as the logical\nneutral position of the slider.\n\nThe slider is available on e.g. the Wacom Airbrush tool."]
        pub fn slider(&self, position: i32) -> () {
            let msg = Event::Slider { position: position };
            self.0.send(msg);
        }
        #[doc = "Wheel delta event\n\nSent whenever the wheel on the tool emits an event. This event\ncontains two values for the same axis change. The degrees value is\nin 0.01 of a degree in the same orientation as the\nwl_pointer.vertical_scroll axis. The clicks value is in discrete\nlogical clicks of the mouse wheel. This value may be zero if the\nmovement of the wheel was less than one logical click.\n\nClients should choose either value and avoid mixing degrees and\nclicks. The compositor may accumulate values smaller than a logical\nclick and emulate click events when a certain threshold is met.\nThus, wl_tablet_tool.wheel events with non-zero clicks values may\nhave different degrees values."]
        pub fn wheel(&self, degrees: i32, clicks: i32) -> () {
            let msg = Event::Wheel {
                degrees: degrees,
                clicks: clicks,
            };
            self.0.send(msg);
        }
        #[doc = "button event\n\nSent whenever a button on the tool is pressed or released.\n\nIf a button is held down when the tool moves in or out of proximity,\nbutton events are generated by the compositor. See\nwp_tablet_tool.proximity_in and wp_tablet_tool.proximity_out for\ndetails."]
        pub fn button(&self, serial: u32, button: u32, state: ButtonState) -> () {
            let msg = Event::Button {
                serial: serial,
                button: button,
                state: state,
            };
            self.0.send(msg);
        }
        #[doc = "frame event\n\nMarks the end of a series of axis and/or button updates from the\ntablet. The Wayland protocol requires axis updates to be sent\nsequentially, however all events within a frame should be considered\none hardware event."]
        pub fn frame(&self, time: u32) -> () {
            let msg = Event::Frame { time: time };
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_SET_CURSOR_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_TYPE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_HARDWARE_SERIAL_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_HARDWARE_ID_WACOM_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_CAPABILITY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DONE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_REMOVED_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PROXIMITY_IN_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PROXIMITY_OUT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DOWN_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_UP_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_MOTION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PRESSURE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DISTANCE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_TILT_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_ROTATION_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_SLIDER_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_WHEEL_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_BUTTON_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_FRAME_SINCE: u32 = 1u32;
    static mut zwp_tablet_tool_v1_requests_set_cursor_types: [*const wl_interface; 4] = [
        NULLPTR as *const wl_interface,
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
        NULLPTR as *const wl_interface,
        NULLPTR as *const wl_interface,
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_tablet_tool_v1_requests: [wl_message; 2] = [
        wl_message {
            name: b"set_cursor\0" as *const u8 as *const c_char,
            signature: b"u?oii\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_tablet_tool_v1_requests_set_cursor_types as *const _ },
        },
        wl_message {
            name: b"destroy\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    static mut zwp_tablet_tool_v1_events_proximity_in_types: [*const wl_interface; 3] = [
        NULLPTR as *const wl_interface,
        unsafe { &super::zwp_tablet_v1::zwp_tablet_v1_interface as *const wl_interface },
        unsafe { &super::wl_surface::wl_surface_interface as *const wl_interface },
    ];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_tablet_tool_v1_events: [wl_message; 19] = [
        wl_message {
            name: b"type\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"hardware_serial\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"hardware_id_wacom\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"capability\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"done\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"removed\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"proximity_in\0" as *const u8 as *const c_char,
            signature: b"uoo\0" as *const u8 as *const c_char,
            types: unsafe { &zwp_tablet_tool_v1_events_proximity_in_types as *const _ },
        },
        wl_message {
            name: b"proximity_out\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"down\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"up\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"motion\0" as *const u8 as *const c_char,
            signature: b"ff\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"pressure\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"distance\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"tilt\0" as *const u8 as *const c_char,
            signature: b"ii\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"rotation\0" as *const u8 as *const c_char,
            signature: b"i\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"slider\0" as *const u8 as *const c_char,
            signature: b"i\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"wheel\0" as *const u8 as *const c_char,
            signature: b"ii\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"button\0" as *const u8 as *const c_char,
            signature: b"uuu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"frame\0" as *const u8 as *const c_char,
            signature: b"u\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_tablet_tool_v1_interface: wl_interface = wl_interface {
        name: b"zwp_tablet_tool_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 2,
        requests: unsafe { &zwp_tablet_tool_v1_requests as *const _ },
        event_count: 19,
        events: unsafe { &zwp_tablet_tool_v1_events as *const _ },
    };
}
#[doc = "graphics tablet device\n\nThe wp_tablet interface represents one graphics tablet device. The\ntablet interface itself does not generate events; all events are\ngenerated by wp_tablet_tool objects when in proximity above a tablet.\n\nA tablet has a number of static characteristics, e.g. device name and\npid/vid. These capabilities are sent in an event sequence after the\nwp_tablet_seat.tablet_added event. This initial event sequence is\nterminated by a wp_tablet.done event."]
pub mod zwp_tablet_v1 {
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
        #[doc = "destroy the tablet object\n\nThis destroys the client's resource for this tablet object.\n\nThis is a destructor, once received this object cannot be used any longer."]
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
        #[doc = "tablet device name\n\nThis event is sent in the initial burst of events before the\nwp_tablet.done event."]
        Name { name: String },
        #[doc = "tablet device USB vendor/product id\n\nThis event is sent in the initial burst of events before the\nwp_tablet.done event."]
        Id { vid: u32, pid: u32 },
        #[doc = "path to the device\n\nA system-specific device path that indicates which device is behind\nthis wp_tablet. This information may be used to gather additional\ninformation about the device, e.g. through libwacom.\n\nA device may have more than one device path. If so, multiple\nwp_tablet.path events are sent. A device may be emulated and not\nhave a device path, and in that case this event will not be sent.\n\nThe format of the path is unspecified, it may be a device node, a\nsysfs path, or some other identifier. It is up to the client to\nidentify the string provided.\n\nThis event is sent in the initial burst of events before the\nwp_tablet.done event."]
        Path { path: String },
        #[doc = "tablet description events sequence complete\n\nThis event is sent immediately to signal the end of the initial\nburst of descriptive events. A client may consider the static\ndescription of the tablet to be complete and finalize initialization\nof the tablet."]
        Done,
        #[doc = "tablet removed event\n\nSent when the tablet has been removed from the system. When a tablet\nis removed, some tools may be removed.\n\nWhen this event is received, the client must wp_tablet.destroy\nthe object."]
        Removed,
    }
    impl super::MessageGroup for Event {
        const MESSAGES: &'static [super::MessageDesc] = &[
            super::MessageDesc {
                name: "name",
                since: 1,
                signature: &[super::ArgumentType::Str],
                destructor: false,
            },
            super::MessageDesc {
                name: "id",
                since: 1,
                signature: &[super::ArgumentType::Uint, super::ArgumentType::Uint],
                destructor: false,
            },
            super::MessageDesc {
                name: "path",
                since: 1,
                signature: &[super::ArgumentType::Str],
                destructor: false,
            },
            super::MessageDesc {
                name: "done",
                since: 1,
                signature: &[],
                destructor: false,
            },
            super::MessageDesc {
                name: "removed",
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
                Event::Name { .. } => 0,
                Event::Id { .. } => 1,
                Event::Path { .. } => 2,
                Event::Done => 3,
                Event::Removed => 4,
            }
        }
        fn since(&self) -> u32 {
            match *self {
                Event::Name { .. } => 1,
                Event::Id { .. } => 1,
                Event::Path { .. } => 1,
                Event::Done => 1,
                Event::Removed => 1,
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
                Event::Name { name } => Message {
                    sender_id: sender_id,
                    opcode: 0,
                    args: smallvec![Argument::Str(Box::new(unsafe {
                        ::std::ffi::CString::from_vec_unchecked(name.into())
                    })),],
                },
                Event::Id { vid, pid } => Message {
                    sender_id: sender_id,
                    opcode: 1,
                    args: smallvec![Argument::Uint(vid), Argument::Uint(pid),],
                },
                Event::Path { path } => Message {
                    sender_id: sender_id,
                    opcode: 2,
                    args: smallvec![Argument::Str(Box::new(unsafe {
                        ::std::ffi::CString::from_vec_unchecked(path.into())
                    })),],
                },
                Event::Done => Message {
                    sender_id: sender_id,
                    opcode: 3,
                    args: smallvec![],
                },
                Event::Removed => Message {
                    sender_id: sender_id,
                    opcode: 4,
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
                Event::Name { name } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    let _arg_0 = ::std::ffi::CString::new(name).unwrap();
                    _args_array[0].s = _arg_0.as_ptr();
                    f(0, &mut _args_array)
                }
                Event::Id { vid, pid } => {
                    let mut _args_array: [wl_argument; 2] = unsafe { ::std::mem::zeroed() };
                    _args_array[0].u = vid;
                    _args_array[1].u = pid;
                    f(1, &mut _args_array)
                }
                Event::Path { path } => {
                    let mut _args_array: [wl_argument; 1] = unsafe { ::std::mem::zeroed() };
                    let _arg_0 = ::std::ffi::CString::new(path).unwrap();
                    _args_array[0].s = _arg_0.as_ptr();
                    f(2, &mut _args_array)
                }
                Event::Done => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(3, &mut _args_array)
                }
                Event::Removed => {
                    let mut _args_array: [wl_argument; 0] = unsafe { ::std::mem::zeroed() };
                    f(4, &mut _args_array)
                }
            }
        }
    }
    #[derive(Clone, Eq, PartialEq)]
    pub struct ZwpTabletV1(Resource<ZwpTabletV1>);
    impl AsRef<Resource<ZwpTabletV1>> for ZwpTabletV1 {
        #[inline]
        fn as_ref(&self) -> &Resource<Self> {
            &self.0
        }
    }
    impl From<Resource<ZwpTabletV1>> for ZwpTabletV1 {
        #[inline]
        fn from(value: Resource<Self>) -> Self {
            ZwpTabletV1(value)
        }
    }
    impl From<ZwpTabletV1> for Resource<ZwpTabletV1> {
        #[inline]
        fn from(value: ZwpTabletV1) -> Self {
            value.0
        }
    }
    impl std::fmt::Debug for ZwpTabletV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self.0))
        }
    }
    impl Interface for ZwpTabletV1 {
        type Request = Request;
        type Event = Event;
        const NAME: &'static str = "zwp_tablet_v1";
        const VERSION: u32 = 1;
        fn c_interface() -> *const wl_interface {
            unsafe { &zwp_tablet_v1_interface }
        }
    }
    impl ZwpTabletV1 {
        #[doc = "tablet device name\n\nThis event is sent in the initial burst of events before the\nwp_tablet.done event."]
        pub fn name(&self, name: String) -> () {
            let msg = Event::Name { name: name };
            self.0.send(msg);
        }
        #[doc = "tablet device USB vendor/product id\n\nThis event is sent in the initial burst of events before the\nwp_tablet.done event."]
        pub fn id(&self, vid: u32, pid: u32) -> () {
            let msg = Event::Id { vid: vid, pid: pid };
            self.0.send(msg);
        }
        #[doc = "path to the device\n\nA system-specific device path that indicates which device is behind\nthis wp_tablet. This information may be used to gather additional\ninformation about the device, e.g. through libwacom.\n\nA device may have more than one device path. If so, multiple\nwp_tablet.path events are sent. A device may be emulated and not\nhave a device path, and in that case this event will not be sent.\n\nThe format of the path is unspecified, it may be a device node, a\nsysfs path, or some other identifier. It is up to the client to\nidentify the string provided.\n\nThis event is sent in the initial burst of events before the\nwp_tablet.done event."]
        pub fn path(&self, path: String) -> () {
            let msg = Event::Path { path: path };
            self.0.send(msg);
        }
        #[doc = "tablet description events sequence complete\n\nThis event is sent immediately to signal the end of the initial\nburst of descriptive events. A client may consider the static\ndescription of the tablet to be complete and finalize initialization\nof the tablet."]
        pub fn done(&self) -> () {
            let msg = Event::Done;
            self.0.send(msg);
        }
        #[doc = "tablet removed event\n\nSent when the tablet has been removed from the system. When a tablet\nis removed, some tools may be removed.\n\nWhen this event is received, the client must wp_tablet.destroy\nthe object."]
        pub fn removed(&self) -> () {
            let msg = Event::Removed;
            self.0.send(msg);
        }
    }
    #[doc = r" The minimal object version supporting this request"]
    pub const REQ_DESTROY_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_NAME_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_ID_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_PATH_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_DONE_SINCE: u32 = 1u32;
    #[doc = r" The minimal object version supporting this event"]
    pub const EVT_REMOVED_SINCE: u32 = 1u32;
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_tablet_v1_requests: [wl_message; 1] = [wl_message {
        name: b"destroy\0" as *const u8 as *const c_char,
        signature: b"\0" as *const u8 as *const c_char,
        types: unsafe { &types_null as *const _ },
    }];
    #[doc = r" C-representation of the messages of this interface, for interop"]
    pub static mut zwp_tablet_v1_events: [wl_message; 5] = [
        wl_message {
            name: b"name\0" as *const u8 as *const c_char,
            signature: b"s\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"id\0" as *const u8 as *const c_char,
            signature: b"uu\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"path\0" as *const u8 as *const c_char,
            signature: b"s\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"done\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
        wl_message {
            name: b"removed\0" as *const u8 as *const c_char,
            signature: b"\0" as *const u8 as *const c_char,
            types: unsafe { &types_null as *const _ },
        },
    ];
    #[doc = r" C representation of this interface, for interop"]
    pub static mut zwp_tablet_v1_interface: wl_interface = wl_interface {
        name: b"zwp_tablet_v1\0" as *const u8 as *const c_char,
        version: 1,
        request_count: 1,
        requests: unsafe { &zwp_tablet_v1_requests as *const _ },
        event_count: 5,
        events: unsafe { &zwp_tablet_v1_events as *const _ },
    };
}
